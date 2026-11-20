//! 会话元数据列表（M1d，P25 完整列表）。title=首条用户消息截断（免费，LLM 标题后置）；
//! last_active=MAX(messages.created_at)（sessions.updated_at 仅淘汰时更新不可靠）；message_count=COUNT。
//! 聚合经 SQL GROUP BY（双兼容）+ 首条用户消息 Rust 侧取（守 JSON 整取纪律）；前缀/模块过滤 Rust 侧。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use kernel_api::{fetch_all_both, KernelError, Store};
use serde::Serialize;
use uuid::Uuid;

use crate::path;

/// 标题截断字符数。
const TITLE_CHARS: usize = 40;

/// 会话行（id, module_id, path, created_at）。
type SessionRow = (Uuid, Option<String>, Option<String>, DateTime<Utc>);
/// 消息聚合行（session_id, count, last_active）。
type AggRow = (Uuid, i64, Option<DateTime<Utc>>);

/// 会话列表项（元数据）。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SessionListItem {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: Option<DateTime<Utc>>,
    pub message_count: i64,
    pub path: Option<String>,
    pub module_id: Option<String>,
}

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

/// 取会话虚拟路径（归属校验；不存在/未分类 → None）。REST get_session 用。
pub async fn get_session_path(
    store: &Store,
    user: Uuid,
    session: Uuid,
) -> Result<Option<String>, KernelError> {
    let row: Option<(Option<String>,)> = kernel_api::fetch_optional_both!(
        store,
        "SELECT path FROM sessions WHERE id = $1 AND user_id = $2",
        (Option<String>,),
        session,
        user
    )
    .map_err(storage_err)?;
    Ok(row.and_then(|(p,)| p))
}

fn truncate_title(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() > TITLE_CHARS {
        let head: String = trimmed.chars().take(TITLE_CHARS).collect();
        format!("{head}…")
    } else if trimmed.is_empty() {
        "新会话".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 列出用户会话元数据。module_filter：None=全部，Some("default")=无模块，Some(id)=该模块。
/// folder_filter：Some(path)=该文件夹子树（含自身），None=全部。
pub async fn list_sessions_meta(
    store: &Store,
    user: Uuid,
    module_filter: Option<&str>,
    folder_filter: Option<&str>,
) -> Result<Vec<SessionListItem>, KernelError> {
    let sessions: Vec<SessionRow> = fetch_all_both!(
        store,
        "SELECT id, module_id, path, created_at FROM sessions WHERE user_id = $1 ORDER BY created_at",
        SessionRow,
        user
    )
    .map_err(storage_err)?;

    // 聚合：count + last_active。
    let agg: Vec<AggRow> = fetch_all_both!(
        store,
        "SELECT session_id, COUNT(*), MAX(created_at) FROM messages
         WHERE session_id IN (SELECT id FROM sessions WHERE user_id = $1) GROUP BY session_id",
        AggRow,
        user
    )
    .map_err(storage_err)?;
    let agg_map: HashMap<Uuid, (i64, Option<DateTime<Utc>>)> =
        agg.into_iter().map(|(id, c, t)| (id, (c, t))).collect();

    // 首条用户消息（按 seq 序，Rust 取每会话首条）。
    let user_msgs: Vec<(Uuid, String)> = fetch_all_both!(
        store,
        "SELECT session_id, content FROM messages
         WHERE role = 'user' AND session_id IN (SELECT id FROM sessions WHERE user_id = $1)
         ORDER BY session_id, seq",
        (Uuid, String),
        user
    )
    .map_err(storage_err)?;
    let mut title_map: HashMap<Uuid, String> = HashMap::new();
    for (sid, content) in user_msgs {
        title_map
            .entry(sid)
            .or_insert_with(|| truncate_title(&content));
    }

    let mut out = Vec::new();
    for (id, module_id, path, created_at) in sessions {
        // 模块过滤（default ↔ NULL）。
        if let Some(want) = module_filter {
            let actual = module_id.as_deref().unwrap_or(path::DEFAULT_NAMESPACE);
            if actual != want {
                continue;
            }
        }
        // 文件夹子树过滤。
        if let Some(folder) = folder_filter {
            let Some(p) = path.as_deref() else { continue };
            if !path::is_within(folder, p) {
                continue;
            }
        }
        let (count, last_active) = agg_map.get(&id).cloned().unwrap_or((0, None));
        out.push(SessionListItem {
            id,
            title: title_map
                .get(&id)
                .cloned()
                .unwrap_or_else(|| "新会话".to_string()),
            created_at,
            last_active,
            message_count: count,
            path,
            module_id,
        });
    }
    Ok(out)
}
