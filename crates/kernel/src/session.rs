//! DB 会话仓储（M1c 生产路径，替换 M1a 内存默认；内存 impl 降为测试替身）。
//!
//! sessions/messages 表（M1b 已建），双兼容 SQL 子集：uuid 主键应用生成、时间戳应用侧、JSON 整取回 Rust。
//! messages.seq 在每会话单活跃回合锁内分配（COALESCE(MAX(seq),0)+1，i32），避并发竞态。
//! 删除会话经外键 ON DELETE CASCADE 级联删 messages（SQLite 须 pool 启用 foreign_keys）。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use kernel_api::{
    execute_both, fetch_all_both, fetch_one_both, fetch_optional_both, ChatMessage, KernelError,
    ModuleId, SessionId, Store, UserId,
};
use serde_json::Value;
use uuid::Uuid;

use crate::seam::{OnDisconnect, SessionMeta, SessionRepo, StoredMessage};

/// DB 会话仓储（Store 注入）。
pub struct DbSessions {
    store: Arc<Store>,
}

impl DbSessions {
    /// 新建仓储。
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

/// messages 行（整取 JSON，映射 ChatMessage；audio_refs 独立列不进 ChatMessage）。
#[derive(sqlx::FromRow)]
struct MessageRow {
    role: String,
    content: String,
    reasoning: Option<String>,
    tool_calls: Option<Value>,
    tool_call_id: Option<String>,
}

/// sessions 元数据行。
#[derive(sqlx::FromRow)]
struct SessionRow {
    module_id: Option<String>,
    on_disconnect: String,
    params_json: Option<Value>,
    summary: Option<String>,
    evicted_up_to: Option<i32>,
}

/// messages 行含 id/seq（淘汰 span 引用消息 ID 用）。
#[derive(sqlx::FromRow)]
struct MessageIdRow {
    id: Uuid,
    seq: i32,
    role: String,
    content: String,
    reasoning: Option<String>,
    tool_calls: Option<Value>,
    tool_call_id: Option<String>,
}

#[async_trait]
impl SessionRepo for DbSessions {
    async fn create(
        &self,
        user: UserId,
        id: SessionId,
        meta: SessionMeta,
    ) -> Result<(), KernelError> {
        let now = Utc::now();
        let module_id = meta.module_id.as_ref().map(|m| m.0.clone());
        let params = serde_json::to_value(&meta.params)
            .map_err(|e| KernelError::Internal(format!("会话参数序列化失败：{e}")))?;
        execute_both!(
            &*self.store,
            "INSERT INTO sessions (id, user_id, module_id, plan_version, on_disconnect, params_json, summary, evicted_up_to, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            id.0,
            user.0,
            module_id,
            Option::<String>::None,
            meta.on_disconnect.as_str(),
            &params,
            meta.summary,
            meta.evicted_up_to,
            now,
            now
        )
        .map_err(storage_err)?;
        Ok(())
    }

    async fn meta(&self, user: UserId, id: SessionId) -> Result<Option<SessionMeta>, KernelError> {
        let row: Option<SessionRow> = fetch_optional_both!(
            &*self.store,
            "SELECT module_id, on_disconnect, params_json, summary, evicted_up_to FROM sessions WHERE id = $1 AND user_id = $2",
            SessionRow,
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(row.map(|r| SessionMeta {
            module_id: r.module_id.map(ModuleId),
            on_disconnect: OnDisconnect::parse(&r.on_disconnect),
            params: r
                .params_json
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            summary: r.summary,
            evicted_up_to: r.evicted_up_to.unwrap_or(0),
        }))
    }

    async fn exists(&self, user: UserId, id: SessionId) -> Result<bool, KernelError> {
        let row: Option<(Uuid,)> = fetch_optional_both!(
            &*self.store,
            "SELECT id FROM sessions WHERE id = $1 AND user_id = $2",
            (Uuid,),
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(row.is_some())
    }

    async fn history(&self, user: UserId, id: SessionId) -> Result<Vec<ChatMessage>, KernelError> {
        let rows: Vec<MessageRow> = fetch_all_both!(
            &*self.store,
            "SELECT role, content, reasoning, tool_calls, tool_call_id FROM messages
             WHERE session_id = $1 AND session_id IN (SELECT id FROM sessions WHERE user_id = $2) ORDER BY seq",
            MessageRow,
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .map(|r| ChatMessage {
                role: r.role,
                content: r.content,
                reasoning: r.reasoning,
                tool_calls: r.tool_calls,
                tool_call_id: r.tool_call_id,
            })
            .collect())
    }

    async fn history_with_ids(
        &self,
        user: UserId,
        id: SessionId,
    ) -> Result<Vec<StoredMessage>, KernelError> {
        let rows: Vec<MessageIdRow> = fetch_all_both!(
            &*self.store,
            "SELECT id, seq, role, content, reasoning, tool_calls, tool_call_id FROM messages
             WHERE session_id = $1 AND session_id IN (SELECT id FROM sessions WHERE user_id = $2) ORDER BY seq",
            MessageIdRow,
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(rows
            .into_iter()
            .map(|r| StoredMessage {
                id: r.id,
                seq: r.seq,
                message: ChatMessage {
                    role: r.role,
                    content: r.content,
                    reasoning: r.reasoning,
                    tool_calls: r.tool_calls,
                    tool_call_id: r.tool_call_id,
                },
            })
            .collect())
    }

    async fn append(
        &self,
        user: UserId,
        id: SessionId,
        msg: ChatMessage,
    ) -> Result<(), KernelError> {
        // 归属校验（失败关闭，审计 C1/C4）：会话不存在或非本用户 → 错误，不写入。
        let owned: Option<(Uuid,)> = fetch_optional_both!(
            &*self.store,
            "SELECT id FROM sessions WHERE id = $1 AND user_id = $2",
            (Uuid,),
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        if owned.is_none() {
            return Err(KernelError::Storage(format!("会话不存在或非本用户：{id}")));
        }
        // seq 分配：单活跃回合锁内串行，COALESCE(MAX(seq),0)+1（PG INTEGER↔i32）。
        let next: (i32,) = fetch_one_both!(
            &*self.store,
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM messages WHERE session_id = $1",
            (i32,),
            id.0
        )
        .map_err(storage_err)?;
        let now = Utc::now();
        execute_both!(
            &*self.store,
            "INSERT INTO messages (id, session_id, seq, role, content, reasoning, tool_calls, tool_call_id, audio_refs, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            Uuid::new_v4(),
            id.0,
            next.0,
            &msg.role,
            &msg.content,
            &msg.reasoning,
            &msg.tool_calls,
            &msg.tool_call_id,
            Option::<Value>::None,
            now
        )
        .map_err(storage_err)?;
        Ok(())
    }

    async fn list(&self, user: UserId) -> Result<Vec<SessionId>, KernelError> {
        let rows: Vec<(Uuid,)> = fetch_all_both!(
            &*self.store,
            "SELECT id FROM sessions WHERE user_id = $1 ORDER BY created_at",
            (Uuid,),
            user.0
        )
        .map_err(storage_err)?;
        Ok(rows.into_iter().map(|(id,)| SessionId(id)).collect())
    }

    async fn delete(&self, user: UserId, id: SessionId) -> Result<(), KernelError> {
        // messages 经外键 ON DELETE CASCADE 级联删；限本用户（审计 C1）。
        execute_both!(
            &*self.store,
            "DELETE FROM sessions WHERE id = $1 AND user_id = $2",
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(())
    }

    async fn update_eviction(
        &self,
        user: UserId,
        id: SessionId,
        summary: Option<String>,
        evicted_up_to: i32,
    ) -> Result<(), KernelError> {
        execute_both!(
            &*self.store,
            "UPDATE sessions SET summary = $1, evicted_up_to = $2, updated_at = $3 WHERE id = $4 AND user_id = $5",
            summary,
            evicted_up_to,
            Utc::now(),
            id.0,
            user.0
        )
        .map_err(storage_err)?;
        Ok(())
    }
}
