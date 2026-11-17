//! 绑定解析 DB 实现（M1b 生产路径，替换 M1a env 旁路）。
//!
//! 5 级 fallback：`(user,module,cap,role) → (user,module,cap,default) → (user,*,cap,role)
//! → (user,*,cap,default) → 未配置`。module 为 None（M1 无模块）时直接从 `*` 级起。
//! 哨兵 `*` 表全模块默认。命中后读 provider 密文 → [`MasterKey`] 解密 → [`ProviderConn`]（key 为 SecretString）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_api::{Capability, KernelError, ModuleId, ProviderConn, Role, UserId};
use secrecy::SecretString;
use serde_json::json;

use crate::crypto::MasterKey;
use crate::repo;
use crate::seam::{BindingResolver, ResolvedBinding};
use kernel_api::Store;

/// DB 绑定解析器（Store + 主密钥注入）。
pub struct DbBindingResolver {
    store: Arc<Store>,
    master: Arc<MasterKey>,
}

impl DbBindingResolver {
    /// 新建解析器。
    pub fn new(store: Arc<Store>, master: Arc<MasterKey>) -> Self {
        Self { store, master }
    }
}

fn cap_str(c: Capability) -> &'static str {
    match c {
        Capability::Llm => "llm",
        Capability::Tts => "tts",
        Capability::Stt => "stt",
    }
}

fn role_str(r: Role) -> &'static str {
    match r {
        Role::Default => "default",
        Role::Summarizer => "summarizer",
        Role::Extractor => "extractor",
        Role::Assessor => "assessor",
    }
}

/// 按优先级生成候选 (module_id, role)：模块覆盖 → 模块默认 → 全模块角色 → 全模块默认。
fn candidates(module: Option<&ModuleId>, role: Role) -> Vec<(String, &'static str)> {
    let role_s = role_str(role);
    let mut v = Vec::new();
    if let Some(m) = module {
        v.push((m.0.clone(), role_s));
        if role != Role::Default {
            v.push((m.0.clone(), "default"));
        }
    }
    v.push(("*".to_string(), role_s));
    if role != Role::Default {
        v.push(("*".to_string(), "default"));
    }
    v
}

#[async_trait]
impl BindingResolver for DbBindingResolver {
    async fn resolve(
        &self,
        user: UserId,
        module: Option<&ModuleId>,
        capability: Capability,
        role: Role,
    ) -> Result<ResolvedBinding, KernelError> {
        let cap = cap_str(capability);
        for (module_id, role_key) in candidates(module, role) {
            let binding =
                repo::find_binding(&self.store, user.0, &module_id, cap, role_key).await?;
            let Some(b) = binding else { continue };
            let secret = repo::get_provider_secret(&self.store, b.provider_id)
                .await?
                .ok_or_else(|| {
                    KernelError::Storage(format!("绑定引用的 provider 不存在：{}", b.provider_id))
                })?;
            let key_bytes = self.master.decrypt(&secret.key_enc)?;
            let key = String::from_utf8(key_bytes)
                .map_err(|_| KernelError::Internal("provider key 非 UTF-8".to_string()))?;
            let conn = ProviderConn {
                base_url: secret.base_url,
                api_key: SecretString::from(key),
                extra: secret.extra_json.unwrap_or_else(|| json!({})),
            };
            return Ok(ResolvedBinding {
                conn,
                model: b.model,
                params: b.params_json,
            });
        }
        Err(KernelError::Unconfigured(format!(
            "未配置绑定：user={} cap={cap} role={}",
            user.0,
            role_str(role)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_order_no_module() {
        let c = candidates(None, Role::Default);
        assert_eq!(c, vec![("*".to_string(), "default")]);
        let c = candidates(None, Role::Summarizer);
        assert_eq!(
            c,
            vec![
                ("*".to_string(), "summarizer"),
                ("*".to_string(), "default")
            ]
        );
    }

    #[test]
    fn candidates_order_with_module() {
        let m = ModuleId("teach-gojuon".to_string());
        let c = candidates(Some(&m), Role::Assessor);
        assert_eq!(
            c,
            vec![
                ("teach-gojuon".to_string(), "assessor"),
                ("teach-gojuon".to_string(), "default"),
                ("*".to_string(), "assessor"),
                ("*".to_string(), "default"),
            ]
        );
        // role=default 时去重（模块默认与角色同）。
        let c = candidates(Some(&m), Role::Default);
        assert_eq!(
            c,
            vec![
                ("teach-gojuon".to_string(), "default"),
                ("*".to_string(), "default")
            ]
        );
    }
}
