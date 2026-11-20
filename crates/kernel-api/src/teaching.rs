//! 教学模块契约与受控 kernel 服务接口（依赖倒置）。

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;

use crate::error::{KernelError, ModuleError};
use crate::llm::ChatStream;
use crate::plan::LearningPlan;
use crate::progress::{PersistedEvent, ProgressInput};
use crate::types::{
    AssetAudio, ChatRequest, ModuleManifest, Role, SessionId, SessionParams, ToolCall, ToolResult,
    ToolSpec, TtsRequest, UserId,
};

/// 模块 scoped 存储句柄：双具体驱动 enum（见 [`crate::store`]）+ 表前缀约定 mod_<id>_*。
pub type ModuleStore = crate::store::Store;

/// 模块受控访问 kernel 服务，由 kernel 实现。模块永不直接接触上游 HTTP 与 key。
///
/// **会话可选（B3 定案 history 2130）**：`for_session` 工厂构造的 ctx 带会话，
/// `for_user`（placement/plan 路由/outbox 消费）无会话 → `session_id()` 返回 None；
/// 需会话的方法（complete_session）对 None fail-closed 报错。
#[async_trait]
pub trait ModuleContext: Send + Sync {
    fn user_id(&self) -> UserId;
    /// 会话 id（for_user ctx 为 None）。
    fn session_id(&self) -> Option<SessionId>;
    /// scoped 存储句柄。
    fn store(&self) -> &ModuleStore;
    /// 发进度输入，kernel 包裹信封后写入持久 outbox（结构化 outcome/item_ref/difficulty 落列）。
    async fn emit_progress(&self, input: ProgressInput) -> Result<(), KernelError>;
    /// 读当前教学方案（路由到 PlanProvider）；None 引导 placement（无 PlanProvider 注册亦 None）。
    async fn current_plan(&self) -> Result<Option<LearningPlan>, KernelError>;
    /// kernel 中介 LLM 调用：按 (user,module,cap,role) 解析绑定解密，model 由绑定填（req.model 忽略），
    /// 全计量 call_log（含 module_id 归因）。模块永不触 key/上游。
    async fn llm(&self, role: Role, req: ChatRequest) -> Result<ChatStream, KernelError>;
    /// 完成语义（完成≠删除）：置 completed_at + 发 session-ended 事件，转录保留。
    /// 幂等（重复完成 false）；无会话（for_user）→ 错误。
    async fn complete_session(&self) -> Result<bool, KernelError>;
    /// 白名单缓存合成（UX-A3 用户侧标准音频）：模块声明稳定 key（如 `gojuon:あ`），
    /// 命中持久资产直供（cache_hit=true，零上游）；未命中按用户 TTS 绑定合成→持久化 /data/assets
    /// （豁免淘汰）→计量（module 归因）。作用域 user+module+key+provider 身份+voice+model，
    /// 换音色/服务商懒再生新版旧版保留。无 TTS 绑定 → Unconfigured（模块降级不阻塞）。
    async fn get_or_synthesize(
        &self,
        key: &str,
        req: TtsRequest,
    ) -> Result<AssetAudio, KernelError>;
}

/// 模块 ctx 工厂（每模块一实例，module_id 构造期绑定）：组合根构造模块时注入，
/// `TeachingModule::routes()` 签名不动（模块路由内经工厂取 per-request ctx）。
pub trait ModuleContextFactory: Send + Sync {
    /// 会话级 ctx（回合/会话路由用）。
    fn for_session(&self, user: UserId, session: SessionId) -> Arc<dyn ModuleContext>;
    /// 用户级 ctx（placement/plan 路由/outbox 消费；session_id=None）。
    fn for_user(&self, user: UserId) -> Arc<dyn ModuleContext>;
}

/// 教学模块（teach-gojuon / teach-dialogue / teach-assess）。
#[async_trait]
pub trait TeachingModule: Send + Sync {
    /// 模块清单（含前端清单、所需能力、会话参数）。
    fn manifest(&self) -> &ModuleManifest;
    /// 模块系统提示词片段；运行时拼接 persona + plan.teacher_prompt + 约束。
    fn persona(&self) -> String;
    /// 模块专属工具；speak 由 kernel 统一提供。
    fn tools(&self) -> Vec<ToolSpec> {
        vec![]
    }
    /// 会话运行时参数（预算/淘汰/工具上限）。
    fn session_params(&self) -> SessionParams {
        SessionParams::default()
    }
    /// 模块 API 路由，挂载于 /api/v1/m/{id}/。
    fn routes(&self) -> Router {
        Router::new()
    }
    /// 执行模块专属工具。
    async fn execute_tool(
        &self,
        ctx: &dyn ModuleContext,
        call: ToolCall,
    ) -> Result<ToolResult, ModuleError>;
    /// 订阅持久事件（评估模块用于进度提取）；默认空。
    async fn handle_event(
        &self,
        _ctx: &dyn ModuleContext,
        _event: &PersistedEvent,
    ) -> Result<(), ModuleError> {
        Ok(())
    }
    async fn on_session_start(&self, _ctx: &dyn ModuleContext) -> Result<(), ModuleError> {
        Ok(())
    }
    async fn on_session_end(&self, _ctx: &dyn ModuleContext) -> Result<(), ModuleError> {
        Ok(())
    }
}
