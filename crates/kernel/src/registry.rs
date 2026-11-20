//! 模块注册表（M2 P1）：module.yaml 运行时反序列化（noyalib，B7 定案 history 2130）+ 注册校验。
//! 组合根按 cargo feature 注册教学模块；kernel 回合循环经注册表取 persona/tools/session_params，
//! REST 经注册表校验 module_id 与挂载模块路由。注册序即迁移序/事件扇出序（M2 P2+）。

use std::collections::HashMap;
use std::sync::Arc;

use kernel_api::{KernelError, ModuleContextFactory, ModuleKind, ModuleManifest, TeachingModule};

/// 解析 module.yaml（impact/cross_module 为 docs-gate 规则字段，serde 忽略）。
pub fn parse_manifest(yaml: &str) -> Result<ModuleManifest, KernelError> {
    noyalib::from_str(yaml).map_err(|e| KernelError::Internal(format!("module.yaml 解析失败：{e}")))
}

/// 已注册模块（清单 + trait 对象 + ctx 工厂）。
pub struct RegisteredModule {
    /// 反序列化清单（前端清单/能力声明/会话参数）。
    pub manifest: ModuleManifest,
    /// 模块 trait 对象。
    pub module: Arc<dyn TeachingModule>,
    /// 模块 ctx 工厂（module_id 构造期绑定；回合工具分发/REST 用）。
    pub factory: Arc<dyn ModuleContextFactory>,
}

/// 模块注册表（注册序保持；id 唯一）。
#[derive(Default)]
pub struct ModuleRegistry {
    modules: Vec<RegisteredModule>,
    by_id: HashMap<String, usize>,
}

impl ModuleRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册教学模块：YAML 反序列化 + kind=teaching 校验 + 清单 id 与 trait 对象一致 + 重复 id 拒绝。
    pub fn register_yaml(
        &mut self,
        yaml: &str,
        module: Arc<dyn TeachingModule>,
        factory: Arc<dyn ModuleContextFactory>,
    ) -> Result<(), KernelError> {
        let manifest = parse_manifest(yaml)?;
        if manifest.kind != ModuleKind::Teaching {
            return Err(KernelError::Internal(format!(
                "仅教学模块可注册：{}（kind={:?}）",
                manifest.id, manifest.kind
            )));
        }
        if manifest.id != module.manifest().id {
            return Err(KernelError::Internal(format!(
                "清单 id 与模块不一致：yaml={} module={}",
                manifest.id,
                module.manifest().id
            )));
        }
        if self.by_id.contains_key(&manifest.id) {
            return Err(KernelError::Internal(format!(
                "模块重复注册：{}",
                manifest.id
            )));
        }
        self.by_id.insert(manifest.id.clone(), self.modules.len());
        self.modules.push(RegisteredModule {
            manifest,
            module,
            factory,
        });
        Ok(())
    }

    /// 按 id 取已注册模块。
    pub fn get(&self, id: &str) -> Option<&RegisteredModule> {
        self.by_id.get(id).map(|&i| &self.modules[i])
    }

    /// 是否已注册。
    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// 注册序列表。
    pub fn list(&self) -> &[RegisteredModule] {
        &self.modules
    }
}
