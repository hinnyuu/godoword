//! 教学模块契约与受控 kernel 服务接口（依赖倒置）。

use async_trait::async_trait;
use axum::Router;

use crate::error::{KernelError, ModuleError};
use crate::plan::LearningPlan;
use crate::progress::{PersistedEvent, ProgressInput};
use crate::types::{
    ModuleManifest, SessionId, SessionParams, ToolCall, ToolResult, ToolSpec, UserId,
};

/// 模块 scoped 存储句柄：sqlx Any pool + 表前缀约定 mod_<id>_*（实用派，不造存储抽象）。
pub type ModuleStore = sqlx::Pool<sqlx::Any>;

/// 模块受控访问 kernel 服务，由 kernel 实现。模块永不直接接触上游 HTTP 与 key。
#[async_trait]
pub trait ModuleContext: Send + Sync {
    fn user_id(&self) -> UserId;
    fn session_id(&self) -> SessionId;
    /// scoped 存储句柄。
    fn store(&self) -> &ModuleStore;
    /// 发进度输入，kernel 包裹信封后写入持久 outbox。
    async fn emit_progress(&self, input: ProgressInput) -> Result<(), KernelError>;
    /// 读当前教学方案（路由到 PlanProvider）；None 引导 placement。
    async fn current_plan(&self) -> Result<Option<LearningPlan>, KernelError>;
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
