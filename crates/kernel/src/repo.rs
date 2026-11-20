//! providers / bindings 仓储 CRUD（kernel 内部，供 REST 与绑定解析）。
//!
//! 用户 API key 经 [`MasterKey`] 加密存 providers.key_enc BYTEA，**绝不以明文返回**（Provider 域类型不含 key）。
//! 所有查询按 user_id  scoped（多用户隔离）。双兼容 SQL 子集：uuid 主键应用生成、时间戳应用侧、JSON 整取。
//! bindings.module_id 哨兵 `*` 表全模块默认；UNIQUE(user_id,module_id,capability,role) 两后端约束。

use chrono::{DateTime, Utc};
use kernel_api::{
    execute_both, fetch_all_both, fetch_optional_both, KernelError, ProviderConn, Store,
};
use secrecy::SecretString;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::crypto::MasterKey;

/// provider 域类型（不含 key）：既是 DB 行也是 REST 序列化体。
#[derive(Debug, Clone, sqlx::FromRow, Serialize, utoipa::ToSchema)]
pub struct Provider {
    pub id: Uuid,
    pub user_id: Uuid,
    pub adapter_id: String,
    pub label: String,
    pub base_url: String,
    pub extra_json: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// binding 域类型：capability/role 为 lowercase 字符串（llm/tts/stt；default/summarizer/extractor/assessor）。
#[derive(Debug, Clone, sqlx::FromRow, Serialize, utoipa::ToSchema)]
pub struct Binding {
    pub id: Uuid,
    pub user_id: Uuid,
    pub module_id: String,
    pub capability: String,
    pub role: String,
    pub provider_id: Uuid,
    pub model: String,
    pub params_json: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 含密文的 provider 行（仅绑定解析内部用，绝不外泄）。
#[derive(sqlx::FromRow)]
pub(crate) struct ProviderSecret {
    pub base_url: String,
    pub key_enc: Vec<u8>,
    pub extra_json: Option<Value>,
}

const PROVIDER_COLS: &str =
    "id, user_id, adapter_id, label, base_url, extra_json, created_at, updated_at";
const BINDING_COLS: &str =
    "id, user_id, module_id, capability, role, provider_id, model, params_json, created_at, updated_at";

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

/// 确保用户行存在（缺失则建，is_admin=true）。
///
/// **测试夹具专用（审计 O6）**：会创建管理员用户，生产用户创建一律走 auth-local register
/// （首用户事务锁/邀请码/密码策略）；新代码不得在请求路径调用。
#[doc(hidden)]
pub async fn ensure_user(store: &Store, id: Uuid) -> Result<(), KernelError> {
    let exists: Option<(Uuid,)> =
        fetch_optional_both!(store, "SELECT id FROM users WHERE id = $1", (Uuid,), id)
            .map_err(storage_err)?;
    if exists.is_none() {
        let now = Utc::now();
        execute_both!(
            store,
            "INSERT INTO users (id, email, display_name, is_admin, status, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
            id,
            Option::<String>::None,
            "dev",
            true,
            "active",
            now,
            now
        )
        .map_err(storage_err)?;
    }
    Ok(())
}

/// 新建 provider（key 加密存储）。
#[allow(clippy::too_many_arguments)]
pub async fn create_provider(
    store: &Store,
    master: &MasterKey,
    user_id: Uuid,
    adapter_id: &str,
    label: &str,
    base_url: &str,
    key: &str,
    extra: Value,
) -> Result<Provider, KernelError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let key_enc = master.encrypt(key.as_bytes())?;
    execute_both!(
        store,
        "INSERT INTO providers (id, user_id, adapter_id, label, base_url, key_enc, extra_json, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        id, user_id, adapter_id, label, base_url, &key_enc, &extra, now, now
    )
    .map_err(storage_err)?;
    Ok(Provider {
        id,
        user_id,
        adapter_id: adapter_id.to_string(),
        label: label.to_string(),
        base_url: base_url.to_string(),
        extra_json: Some(extra),
        created_at: now,
        updated_at: now,
    })
}

/// 列出用户 providers（按创建序）。
pub async fn list_providers(store: &Store, user_id: Uuid) -> Result<Vec<Provider>, KernelError> {
    let sql =
        format!("SELECT {PROVIDER_COLS} FROM providers WHERE user_id = $1 ORDER BY created_at");
    fetch_all_both!(store, &sql, Provider, user_id).map_err(storage_err)
}

/// 取用户单个 provider（不存在或非本用户 → None）。
pub async fn get_provider(
    store: &Store,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Provider>, KernelError> {
    let sql = format!("SELECT {PROVIDER_COLS} FROM providers WHERE id = $1 AND user_id = $2");
    fetch_optional_both!(store, &sql, Provider, id, user_id).map_err(storage_err)
}

/// 更新 provider（label/base_url/extra；key 为 Some 时重加密）。返回更新后体；非本用户 → None。
#[allow(clippy::too_many_arguments)]
pub async fn update_provider(
    store: &Store,
    master: &MasterKey,
    user_id: Uuid,
    id: Uuid,
    label: &str,
    base_url: &str,
    key: Option<&str>,
    extra: Value,
) -> Result<Option<Provider>, KernelError> {
    let now = Utc::now();
    let affected = match key {
        Some(k) => {
            let key_enc = master.encrypt(k.as_bytes())?;
            execute_both!(
                store,
                "UPDATE providers SET label=$1, base_url=$2, key_enc=$3, extra_json=$4, updated_at=$5
                 WHERE id=$6 AND user_id=$7",
                label, base_url, &key_enc, &extra, now, id, user_id
            )
            .map_err(storage_err)?
        }
        None => execute_both!(
            store,
            "UPDATE providers SET label=$1, base_url=$2, extra_json=$3, updated_at=$4
             WHERE id=$5 AND user_id=$6",
            label,
            base_url,
            &extra,
            now,
            id,
            user_id
        )
        .map_err(storage_err)?,
    };
    if affected == 0 {
        return Ok(None);
    }
    get_provider(store, user_id, id).await
}

/// 删除 provider（级联删其 bindings，外键 ON DELETE CASCADE）。返回是否删除。
pub async fn delete_provider(store: &Store, user_id: Uuid, id: Uuid) -> Result<bool, KernelError> {
    let n = execute_both!(
        store,
        "DELETE FROM providers WHERE id=$1 AND user_id=$2",
        id,
        user_id
    )
    .map_err(storage_err)?;
    Ok(n > 0)
}

/// 取 provider 密文行（绑定解析内部用）。**带 user_id 归属条件**（审计 C2：不解密他人 provider）。
pub(crate) async fn get_provider_secret(
    store: &Store,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<ProviderSecret>, KernelError> {
    fetch_optional_both!(
        store,
        "SELECT base_url, key_enc, extra_json FROM providers WHERE id = $1 AND user_id = $2",
        ProviderSecret,
        id,
        user_id
    )
    .map_err(storage_err)
}

/// 取用户 provider 的解密连接（models/voices 测试端点用）。非本用户/不存在 → None。
pub async fn provider_conn(
    store: &Store,
    master: &MasterKey,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<ProviderConn>, KernelError> {
    if get_provider(store, user_id, id).await?.is_none() {
        return Ok(None);
    }
    let secret = get_provider_secret(store, id, user_id)
        .await?
        .ok_or_else(|| KernelError::Storage(format!("provider 不存在：{id}")))?;
    let key_bytes = master.decrypt(&secret.key_enc)?;
    let key = String::from_utf8(key_bytes)
        .map_err(|_| KernelError::Internal("provider key 非 UTF-8".to_string()))?;
    Ok(Some(ProviderConn {
        base_url: secret.base_url,
        api_key: SecretString::from(key),
        extra: secret.extra_json.unwrap_or_else(|| json!({})),
    }))
}

/// 新建 binding。UNIQUE(user,module,cap,role) 冲突 → Storage 错误。
#[allow(clippy::too_many_arguments)]
pub async fn create_binding(
    store: &Store,
    user_id: Uuid,
    module_id: &str,
    capability: &str,
    role: &str,
    provider_id: Uuid,
    model: &str,
    params: Value,
) -> Result<Binding, KernelError> {
    // provider 归属校验（审计 C2）：不可绑定他人 provider。
    if get_provider(store, user_id, provider_id).await?.is_none() {
        return Err(KernelError::Storage(format!(
            "provider 不存在或非本用户：{provider_id}"
        )));
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    execute_both!(
        store,
        "INSERT INTO bindings (id, user_id, module_id, capability, role, provider_id, model, params_json, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        id, user_id, module_id, capability, role, provider_id, model, &params, now, now
    )
    .map_err(storage_err)?;
    Ok(Binding {
        id,
        user_id,
        module_id: module_id.to_string(),
        capability: capability.to_string(),
        role: role.to_string(),
        provider_id,
        model: model.to_string(),
        params_json: params,
        created_at: now,
        updated_at: now,
    })
}

/// 列出用户 bindings。
pub async fn list_bindings(store: &Store, user_id: Uuid) -> Result<Vec<Binding>, KernelError> {
    let sql = format!("SELECT {BINDING_COLS} FROM bindings WHERE user_id = $1 ORDER BY created_at");
    fetch_all_both!(store, &sql, Binding, user_id).map_err(storage_err)
}

/// 取用户单个 binding。
pub async fn get_binding(
    store: &Store,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Binding>, KernelError> {
    let sql = format!("SELECT {BINDING_COLS} FROM bindings WHERE id = $1 AND user_id = $2");
    fetch_optional_both!(store, &sql, Binding, id, user_id).map_err(storage_err)
}

/// 更新 binding 的 provider/model/params（module/cap/role 为身份键，改它须删建）。非本用户 → None。
pub async fn update_binding(
    store: &Store,
    user_id: Uuid,
    id: Uuid,
    provider_id: Uuid,
    model: &str,
    params: Value,
) -> Result<Option<Binding>, KernelError> {
    // provider 归属校验（审计 C2）：不可改绑他人 provider。
    if get_provider(store, user_id, provider_id).await?.is_none() {
        return Err(KernelError::Storage(format!(
            "provider 不存在或非本用户：{provider_id}"
        )));
    }
    let now = Utc::now();
    let affected = execute_both!(
        store,
        "UPDATE bindings SET provider_id=$1, model=$2, params_json=$3, updated_at=$4
         WHERE id=$5 AND user_id=$6",
        provider_id,
        model,
        &params,
        now,
        id,
        user_id
    )
    .map_err(storage_err)?;
    if affected == 0 {
        return Ok(None);
    }
    get_binding(store, user_id, id).await
}

/// 删除 binding。返回是否删除。
pub async fn delete_binding(store: &Store, user_id: Uuid, id: Uuid) -> Result<bool, KernelError> {
    let n = execute_both!(
        store,
        "DELETE FROM bindings WHERE id=$1 AND user_id=$2",
        id,
        user_id
    )
    .map_err(storage_err)?;
    Ok(n > 0)
}

/// 按 (user,module,cap,role) 取 binding（绑定解析用，module_id 已含哨兵 `*`）。
pub(crate) async fn find_binding(
    store: &Store,
    user_id: Uuid,
    module_id: &str,
    capability: &str,
    role: &str,
) -> Result<Option<Binding>, KernelError> {
    let sql = format!(
        "SELECT {BINDING_COLS} FROM bindings WHERE user_id=$1 AND module_id=$2 AND capability=$3 AND role=$4"
    );
    fetch_optional_both!(store, &sql, Binding, user_id, module_id, capability, role)
        .map_err(storage_err)
}
