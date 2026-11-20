//! 用量计量（call_log）+ 进度事件 outbox（progress_events）写入与投递（M1c）。
//!
//! call_log 记录所有上游调用（LLM/TTS/summarize），含 tokens/characters/latency/status（用量看板、预算警告）。
//! outbox 持久进度事件（processed_at NULL=未处理），dispatcher 轮询路由到注册模块 handle_event；
//! M1c 无教学模块消费者（骨架），LLM 提取报告归 teach-assess M2。双兼容 SQL 子集。

use chrono::{DateTime, Utc};
use kernel_api::{
    execute_both, fetch_all_both, fetch_one_both, fetch_optional_both, KernelError, Store,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::seam::StoredMessage;

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

/// 上游调用计量记录（映射 call_log 列；tokens/characters 可空）。
pub struct CallRecord {
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    /// 模块归因（审计 B4：用量看板按模块聚合 + ctx.llm 全计量；写入时填，会话删除后不丢归因）。
    pub module_id: Option<String>,
    /// 能力（llm/tts）。
    pub capability: String,
    pub adapter_id: String,
    pub model: String,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub characters: Option<i32>,
    pub latency_ms: i32,
    /// ok / error / cache_hit。
    pub status: String,
    pub error_code: Option<String>,
}

/// 记一条上游调用计量。写失败 warn 留痕（审计 O2：计量 fail-open 不阻塞回合，
/// 但 M2 用量看板以 call_log 为事实源，静默丢弃不可接受）。
#[allow(clippy::too_many_arguments)]
pub async fn record_call(store: &Store, rec: CallRecord) -> Result<(), KernelError> {
    let now = Utc::now();
    let r = execute_both!(
        store,
        "INSERT INTO call_log (id, user_id, session_id, run_id, module_id, capability, adapter_id, model,
            prompt_tokens, completion_tokens, reasoning_tokens, total_tokens, characters, latency_ms, status, error_code, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
        Uuid::new_v4(),
        rec.user_id,
        rec.session_id,
        rec.run_id,
        &rec.module_id,
        &rec.capability,
        &rec.adapter_id,
        &rec.model,
        rec.prompt_tokens,
        rec.completion_tokens,
        rec.reasoning_tokens,
        rec.total_tokens,
        rec.characters,
        rec.latency_ms,
        &rec.status,
        rec.error_code,
        now
    );
    match r {
        Ok(_) => Ok(()),
        Err(e) => {
            let ke = storage_err(e);
            tracing::warn!("call_log 计量写入失败（不阻塞回合，用量可能少计）：{ke}");
            Err(ke)
        }
    }
}

/// 用户 call_log 行（status, characters, total_tokens；用量看板/测试断言用）。
pub async fn call_log_rows(
    store: &Store,
    user_id: Uuid,
) -> Result<Vec<(String, Option<i32>, Option<i32>)>, KernelError> {
    fetch_all_both!(
        store,
        "SELECT status, characters, total_tokens FROM call_log WHERE user_id = $1 ORDER BY created_at",
        (String, Option<i32>, Option<i32>),
        user_id
    )
    .map_err(storage_err)
}

/// 用户 call_log 计数（测试断言用）。
pub async fn call_log_count(store: &Store, user_id: Uuid) -> Result<i64, KernelError> {
    let row: (i64,) = fetch_one_both!(
        store,
        "SELECT COUNT(*) FROM call_log WHERE user_id = $1",
        (i64,),
        user_id
    )
    .map_err(storage_err)?;
    Ok(row.0)
}

/// 发进度事件入 outbox（processed_at NULL=未处理）。返回事件 id。
/// 结构化字段 outcome/item_ref/difficulty 落独立列（0006，审计 B1）；kind 缺省 custom。
pub async fn emit_progress(
    store: &Store,
    user_id: Uuid,
    session_id: Option<Uuid>,
    module_id: &str,
    input: &kernel_api::ProgressInput,
) -> Result<Uuid, KernelError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let kind = input.kind.unwrap_or(kernel_api::EventKind::Custom);
    let kind_str = serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "custom".to_string());
    let skill_tags = serde_json::to_value(&input.skill_tags).unwrap_or(json!([]));
    let outcome_str = input
        .outcome
        .map(|o| {
            serde_json::to_value(o)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        })
        .unwrap_or(None);
    execute_both!(
        store,
        "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, outcome, item_ref, difficulty, detail, created_at, processed_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        id,
        user_id,
        session_id,
        module_id,
        &kind_str,
        &skill_tags,
        &outcome_str,
        &input.item_ref,
        input.difficulty,
        &input.detail,
        now,
        Option::<DateTime<Utc>>::None
    )
    .map_err(storage_err)?;
    Ok(id)
}

/// outbox 未处理事件行（dispatcher 用）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub module_id: String,
    pub kind: String,
    pub skill_tags: Option<Value>,
    pub outcome: Option<String>,
    pub item_ref: Option<String>,
    pub difficulty: Option<f32>,
    pub detail: Option<Value>,
    pub created_at: DateTime<Utc>,
}

/// 读未处理事件（按创建序，limit 上限）。
pub async fn pending_events(store: &Store, limit: i64) -> Result<Vec<OutboxEvent>, KernelError> {
    fetch_all_both!(
        store,
        "SELECT id, user_id, session_id, module_id, kind, skill_tags, outcome, item_ref, difficulty, detail, created_at
         FROM progress_events WHERE processed_at IS NULL ORDER BY created_at LIMIT $1",
        OutboxEvent,
        limit
    )
    .map_err(storage_err)
}

/// 标记事件已处理。
pub async fn mark_processed(store: &Store, id: Uuid) -> Result<(), KernelError> {
    let now = Utc::now();
    execute_both!(
        store,
        "UPDATE progress_events SET processed_at = $1 WHERE id = $2",
        now,
        id
    )
    .map_err(storage_err)?;
    Ok(())
}

/// outbox 事件处理器（M2 教学模块按 module_id 注册；M1c 无消费者，骨架）。
#[async_trait::async_trait]
pub trait OutboxHandler: Send + Sync {
    /// 处理事件；Ok 则标记 processed，Err 留待重试。
    async fn handle(&self, event: &OutboxEvent) -> Result<(), KernelError>;
}

/// 投递一轮未处理事件：按 module_id 路由到注册处理器，成功则标记 processed。
/// 无处理器的事件留待（M2 模块注册后消费）。返回本轮处理数。
pub async fn dispatch_once(
    store: &Store,
    handlers: &std::collections::HashMap<String, std::sync::Arc<dyn OutboxHandler>>,
    limit: i64,
) -> Result<usize, KernelError> {
    let events = pending_events(store, limit).await?;
    let mut processed = 0;
    for ev in events {
        if let Some(h) = handlers.get(&ev.module_id) {
            if h.handle(&ev).await.is_ok() {
                mark_processed(store, ev.id).await?;
                processed += 1;
            }
        }
    }
    Ok(processed)
}

/// 会话结束原子落库（审计 R6/N2）：**DELETE...RETURNING module_id 唯一认领** → 取得一行才同事务插入
/// session-ended outbox → 提交。并发删除仅一个事务取得行（恰好一个事件、一个 true）；不存在/非本用户/已被
/// 并发删除 → Ok(false)。emit 失败则 DELETE 回滚（不丢事件不伪造结束）；事件提交后才可见 dispatcher 不提前消费。
pub async fn end_session(
    store: &Store,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<bool, KernelError> {
    let now = Utc::now();
    let skill_tags = json!([]);
    let detail = json!({});
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            let row: Option<(Option<String>,)> = sqlx::query_as(
                "DELETE FROM sessions WHERE id = ? AND user_id = ? RETURNING module_id",
            )
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_err)?;
            let Some((module_id,)) = row else {
                return Ok(false);
            };
            let module = module_id.unwrap_or_else(|| "kernel".to_string());
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(&module)
            .bind("session-ended")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            tx.commit().await.map_err(storage_err)?;
            Ok(true)
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            let row: Option<(Option<String>,)> = sqlx::query_as(
                "DELETE FROM sessions WHERE id = $1 AND user_id = $2 RETURNING module_id",
            )
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_err)?;
            let Some((module_id,)) = row else {
                return Ok(false);
            };
            let module = module_id.unwrap_or_else(|| "kernel".to_string());
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(&module)
            .bind("session-ended")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            tx.commit().await.map_err(storage_err)?;
            Ok(true)
        }
    }
}

/// 钉住会话方案版本（M2 P1：每回合取当前方案写入；限本用户）。
pub async fn set_plan_version(
    store: &Store,
    user_id: Uuid,
    session_id: Uuid,
    version: u32,
) -> Result<(), KernelError> {
    let now = Utc::now();
    let n = execute_both!(
        store,
        "UPDATE sessions SET plan_version = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
        version as i32,
        now,
        session_id,
        user_id
    )
    .map_err(storage_err)?;
    if n == 0 {
        return Err(KernelError::Storage(format!(
            "会话不存在或非本用户：{session_id}"
        )));
    }
    Ok(())
}

/// 会话完成原子落库（M2 P1，完成≠删除）：**条件 UPDATE completed_at IS NULL 唯一认领** →
/// 取得一行才同事务插入 session-ended outbox → 提交。转录保留（触发提取，assess P3 消费）；
/// 重复完成/不存在/非本用户 → Ok(false)（幂等）。并发完成仅一个事务取得行（恰一个事件）。
/// **DELETE 隐私删除不发事件**语义翻转归 P2（审计 B2，现 end_session 行为暂不动）。
pub async fn complete_session(
    store: &Store,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<bool, KernelError> {
    let now = Utc::now();
    let skill_tags = json!([]);
    let detail = json!({});
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            let row: Option<(Option<String>,)> = sqlx::query_as(
                "UPDATE sessions SET completed_at = ?, updated_at = ? WHERE id = ? AND user_id = ? AND completed_at IS NULL RETURNING module_id",
            )
            .bind(now)
            .bind(now)
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_err)?;
            let Some((module_id,)) = row else {
                return Ok(false);
            };
            let module = module_id.unwrap_or_else(|| "kernel".to_string());
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(&module)
            .bind("session-ended")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            tx.commit().await.map_err(storage_err)?;
            Ok(true)
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            let row: Option<(Option<String>,)> = sqlx::query_as(
                "UPDATE sessions SET completed_at = $1, updated_at = $2 WHERE id = $3 AND user_id = $4 AND completed_at IS NULL RETURNING module_id",
            )
            .bind(now)
            .bind(now)
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_err)?;
            let Some((module_id,)) = row else {
                return Ok(false);
            };
            let module = module_id.unwrap_or_else(|| "kernel".to_string());
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(&module)
            .bind("session-ended")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            tx.commit().await.map_err(storage_err)?;
            Ok(true)
        }
    }
}

/// 淘汰 span 幂等键（审计 R4）：消息 ID 集排序后 sha256。同一 span 跨回合稳定 → summarize 计费去重。
pub fn summary_span_hash(evicted: &[StoredMessage]) -> String {
    let mut ids: Vec<String> = evicted.iter().map(|m| m.id.to_string()).collect();
    ids.sort();
    let mut h = Sha256::new();
    for id in &ids {
        h.update(id.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 查已摘要 span fragment（命中复用，免重复 LLM 计费，审计 R4）。
pub async fn lookup_summary_span(
    store: &Store,
    span_hash: &str,
) -> Result<Option<String>, KernelError> {
    let row: Option<(String,)> = fetch_optional_both!(
        store,
        "SELECT fragment FROM summary_spans WHERE span_hash = $1",
        (String,),
        span_hash
    )
    .map_err(storage_err)?;
    Ok(row.map(|(f,)| f))
}

/// 持久化摘要 span fragment（LLM 调用后独立提交；evict 失败下回合 lookup 命中复用免重计费，审计 R4）。
pub async fn store_summary_span(
    store: &Store,
    span_hash: &str,
    session_id: Uuid,
    fragment: &str,
) -> Result<(), KernelError> {
    execute_both!(
        store,
        "INSERT INTO summary_spans (span_hash, session_id, fragment, created_at) VALUES ($1,$2,$3,$4)",
        span_hash,
        session_id,
        fragment,
        Utc::now()
    )
    .map_err(storage_err)?;
    Ok(())
}

/// 淘汰原子落库（审计 R4）：session-evicted outbox INSERT + 水位/滚动摘要 UPDATE **同事务**。
/// 任一步失败整体回滚——emit 失败则水位不推进（不丢事件），update 失败则 outbox 回滚（不重复/无孤儿）。限本用户。
#[allow(clippy::too_many_arguments)]
pub async fn evict_session(
    store: &Store,
    user_id: Uuid,
    session_id: Uuid,
    module_id: &str,
    summary: Option<String>,
    watermark: i32,
    message_ids: &[String],
) -> Result<(), KernelError> {
    let now = Utc::now();
    let detail = json!({ "message_ids": message_ids });
    let skill_tags = json!([]);
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(module_id)
            .bind("session-evicted")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            let n = sqlx::query(
                "UPDATE sessions SET summary = ?, evicted_up_to = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            )
            .bind(&summary)
            .bind(watermark)
            .bind(now)
            .bind(session_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?
            .rows_affected();
            if n == 0 {
                return Err(KernelError::Storage(format!(
                    "会话不存在或非本用户：{session_id}"
                )));
            }
            tx.commit().await.map_err(storage_err)?;
            Ok(())
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            sqlx::query(
                "INSERT INTO progress_events (id, user_id, session_id, module_id, kind, skill_tags, detail, created_at, processed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(session_id)
            .bind(module_id)
            .bind("session-evicted")
            .bind(&skill_tags)
            .bind(&detail)
            .bind(now)
            .bind(Option::<DateTime<Utc>>::None)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?;
            let n = sqlx::query(
                "UPDATE sessions SET summary = $1, evicted_up_to = $2, updated_at = $3 WHERE id = $4 AND user_id = $5",
            )
            .bind(&summary)
            .bind(watermark)
            .bind(now)
            .bind(session_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?
            .rows_affected();
            if n == 0 {
                return Err(KernelError::Storage(format!(
                    "会话不存在或非本用户：{session_id}"
                )));
            }
            tx.commit().await.map_err(storage_err)?;
            Ok(())
        }
    }
}
