//! 教学方案契约。teach-assess 实现，薄接口。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::PlanError;
use crate::types::UserId;

/// 结构化方案（供 UI 与难度逻辑）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStructured {
    pub level: String,
    pub focus: Vec<String>,
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub extra: Value,
}

/// 教学方案：双重表示（结构化 + 渲染好的系统提示词片段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPlan {
    pub version: u32,
    pub structured: PlanStructured,
    pub teacher_prompt: String,
    pub updated_at: DateTime<Utc>,
}

/// 方案提供者（teach-assess）。bootstrap 与事件消费封装在模块内部。
#[async_trait]
pub trait PlanProvider: Send + Sync {
    /// 读当前方案；None 表示需 placement，前端引导评估模块。
    async fn current_plan(&self, user_id: UserId) -> Result<Option<LearningPlan>, PlanError>;
}
