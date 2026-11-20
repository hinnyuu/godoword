//! teach-free：自由对话轻量教学模块（T4 决议 history 1614，M2 P1 落地）。
//! 内核默认教师 persona 移入本模块；plan 可选（未 placement 也能自由对话，persona-only，转录仍喂评估）。
//! 无专属工具（speak 由 kernel 统一提供）；P1 无模块路由（前端包装归 P5）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_api::{
    ModuleContext, ModuleError, ModuleManifest, PersistedEvent, TeachingModule, ToolCall,
    ToolResult,
};

/// 模块 id（等于 crate 名）。
pub const TEACH_FREE_ID: &str = "teach-free";

/// 日语教师 persona（面向中文母语学习者；原 kernel DEFAULT_TEACHER_PERSONA 移入，T4）。
pub const PERSONA: &str = "你是 godoword 的日语教师，面向中文母语学习者。\
用中文讲解语法与词义，用日语示范发音与例句。需要发音、领读或听力示范时调用 speak 工具\
（慢速教学设 rate 小于 1，日语设 language_hints 为 [\"ja\"]）。循序渐进，多鼓励，\
主动纠正发音与用法错误，并给出简短练习。";

/// module.yaml 单一事实源（注册表与本模块共用）。
pub fn module_yaml() -> &'static str {
    include_str!("../module.yaml")
}

/// 自由对话模块。
pub struct TeachFree {
    manifest: ModuleManifest,
}

impl TeachFree {
    /// 新建（清单自 module.yaml 反序列化，与注册表同源）。
    pub fn new() -> Self {
        let manifest: ModuleManifest =
            noyalib::from_str(module_yaml()).expect("teach-free module.yaml 须合法");
        Self { manifest }
    }
}

impl Default for TeachFree {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TeachingModule for TeachFree {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn persona(&self) -> String {
        PERSONA.to_string()
    }

    async fn execute_tool(
        &self,
        _ctx: &dyn ModuleContext,
        call: ToolCall,
    ) -> Result<ToolResult, ModuleError> {
        // 自由对话无专属工具（speak kernel 统一）；未知工具如实报错回喂 LLM。
        Err(ModuleError::Tool(format!(
            "teach-free 无专属工具：{}",
            call.name
        )))
    }

    async fn handle_event(
        &self,
        _ctx: &dyn ModuleContext,
        _event: &PersistedEvent,
    ) -> Result<(), ModuleError> {
        // 自由对话不消费事件（转录经 assess 提取，P3）。
        Ok(())
    }
}

/// 组合根注册辅助：构造模块 trait 对象。
pub fn module() -> Arc<dyn TeachingModule> {
    Arc::new(TeachFree::new())
}
