//! 用量计量 + outbox 双后端测试（纪律➀）。PG 独立 schema 隔离避并行竞态。
//! 验证 call_log 写入（LLM/TTS）+ outbox emit/pending + dispatch 路由标记 + 无处理器留待。

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use kernel::{
    dispatch_once, emit_progress, pending_events, record_call, run_migrations, CallRecord,
    OutboxEvent, OutboxHandler, KERNEL_MIGRATIONS,
};
use kernel_api::{fetch_one_both, KernelError, Store};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

struct CountHandler(AtomicUsize);
#[async_trait::async_trait]
impl OutboxHandler for CountHandler {
    async fn handle(&self, _ev: &OutboxEvent) -> Result<(), KernelError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn run_test(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let session = Uuid::new_v4();
    let run = Uuid::new_v4();

    // call_log：LLM + TTS 各一条。
    record_call(
        &store,
        CallRecord {
            user_id: user,
            session_id: Some(session),
            run_id: Some(run),
            capability: "llm".to_string(),
            adapter_id: "llm-openai".to_string(),
            model: "qwen4-max-preview".to_string(),
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            reasoning_tokens: None,
            total_tokens: Some(15),
            characters: None,
            latency_ms: 120,
            status: "ok".to_string(),
            error_code: None,
        },
    )
    .await
    .unwrap();
    record_call(
        &store,
        CallRecord {
            user_id: user,
            session_id: Some(session),
            run_id: Some(run),
            capability: "tts".to_string(),
            adapter_id: "tts-qwen-audio".to_string(),
            model: "qwen-audio-3.0-tts-plus".to_string(),
            prompt_tokens: None,
            completion_tokens: None,
            reasoning_tokens: None,
            total_tokens: None,
            characters: Some(5),
            latency_ms: 80,
            status: "ok".to_string(),
            error_code: None,
        },
    )
    .await
    .unwrap();
    let cnt: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM call_log WHERE user_id = $1",
        (i64,),
        user
    )
    .unwrap();
    assert_eq!(cnt.0, 2, "{b} call_log 两条");
    let chars: (Option<i32>,) = fetch_one_both!(
        &*store,
        "SELECT characters FROM call_log WHERE capability = 'tts' AND user_id = $1",
        (Option<i32>,),
        user
    )
    .unwrap();
    assert_eq!(chars.0, Some(5), "{b} TTS characters round-trip");

    // outbox：emit → pending → dispatch（有处理器）→ processed。
    let ev_id = emit_progress(
        &store,
        user,
        Some(session),
        "kernel",
        "session-evicted",
        json!([]),
        json!({ "message_ids": ["m1", "m2"] }),
    )
    .await
    .unwrap();
    let pending = pending_events(&store, 100).await.unwrap();
    assert!(pending.iter().any(|e| e.id == ev_id), "{b} pending 含事件");
    assert_eq!(pending[0].module_id, "kernel", "{b} module_id 哨兵");

    // 无处理器：留待（不标记）。
    let empty: HashMap<String, Arc<dyn OutboxHandler>> = HashMap::new();
    let n = dispatch_once(&store, &empty, 100).await.unwrap();
    assert_eq!(n, 0, "{b} 无处理器不处理");
    assert!(
        pending_events(&store, 100)
            .await
            .unwrap()
            .iter()
            .any(|e| e.id == ev_id),
        "{b} 无处理器事件留待"
    );

    // 有处理器：处理 + 标记 processed。
    let handler = Arc::new(CountHandler(AtomicUsize::new(0)));
    let mut handlers: HashMap<String, Arc<dyn OutboxHandler>> = HashMap::new();
    handlers.insert("kernel".to_string(), handler.clone());
    let n = dispatch_once(&store, &handlers, 100).await.unwrap();
    assert_eq!(n, 1, "{b} 有处理器处理 1 条");
    assert_eq!(handler.0.load(Ordering::SeqCst), 1, "{b} handler 调用 1 次");
    assert!(
        !pending_events(&store, 100)
            .await
            .unwrap()
            .iter()
            .any(|e| e.id == ev_id),
        "{b} 处理后不再 pending"
    );

    kernel_api::execute_both!(&*store, "DELETE FROM call_log WHERE user_id = $1", user).unwrap();
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM progress_events WHERE user_id = $1",
        user
    )
    .unwrap();
    kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", user).unwrap();
}

#[tokio::test]
async fn journal_and_outbox() {
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_test(Arc::new(Store::Sqlite(sqlite))).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_journal_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(2)
            .after_connect(move |conn, _| {
                let s = s.clone();
                Box::pin(async move {
                    sqlx::query(&format!("SET search_path TO {s}"))
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        run_test(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}
