//! 用量计量 + outbox 双后端测试（纪律➀）。PG 独立 schema 隔离避并行竞态。
//! 验证 call_log 写入（LLM/TTS）+ outbox emit/pending + dispatch 路由标记 + 无处理器留待。

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use kernel::{
    dispatch_once, emit_progress, pending_events, record_call, run_migrations, CallRecord,
    OutboxEvent, OutboxHandler, SessionRepo, KERNEL_MIGRATIONS,
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

/// R4 淘汰原子性失败点断言：emit/update 任一失败整体回滚（不丢事件/不推进水位致重复）。
async fn assert_evict_atomicity(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let a = Uuid::new_v4();
    let bb = Uuid::new_v4();
    kernel::repo::ensure_user(&store, a).await.unwrap();
    kernel::repo::ensure_user(&store, bb).await.unwrap();
    let sessions = kernel::DbSessions::new(store.clone());
    let sid = kernel_api::SessionId::new();
    sessions
        .create(kernel_api::UserId(a), sid, kernel::SessionMeta::default())
        .await
        .unwrap();
    let ev_count = |store: Arc<Store>, sid: Uuid| async move {
        let c: (i64,) = fetch_one_both!(
            &*store,
            "SELECT COUNT(*) FROM progress_events WHERE session_id = $1",
            (i64,),
            sid
        )
        .unwrap();
        c.0
    };

    // update 失败（非本用户）→ outbox INSERT 回滚，无孤儿事件，水位不动。
    let r1 = kernel::evict_session(
        &store,
        bb,
        sid.0,
        "kernel",
        Some("x".to_string()),
        5,
        &["m1".to_string()],
    )
    .await;
    assert!(r1.is_err(), "{b} R4：非本用户 evict 拒绝");
    assert_eq!(
        ev_count(store.clone(), sid.0).await,
        0,
        "{b} R4：update 失败 outbox 回滚无孤儿"
    );
    let m1 = sessions
        .meta(kernel_api::UserId(a), sid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(m1.evicted_up_to, 0, "{b} R4：update 失败水位不动");

    // emit 失败（user_id FK 不存在）→ 水位 UPDATE 回滚。
    let ghost = Uuid::new_v4();
    let r2 = kernel::evict_session(
        &store,
        ghost,
        sid.0,
        "kernel",
        Some("y".to_string()),
        9,
        &["m2".to_string()],
    )
    .await;
    assert!(r2.is_err(), "{b} R4：emit FK 失败拒绝");
    let m2 = sessions
        .meta(kernel_api::UserId(a), sid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(m2.evicted_up_to, 0, "{b} R4：emit 失败水位回滚不动");
    assert_eq!(
        ev_count(store.clone(), sid.0).await,
        0,
        "{b} R4：emit 失败无事件"
    );

    // happy：原子落库水位 + 摘要 + outbox。
    kernel::evict_session(
        &store,
        a,
        sid.0,
        "kernel",
        Some("z".to_string()),
        7,
        &["m3".to_string()],
    )
    .await
    .unwrap();
    let m3 = sessions
        .meta(kernel_api::UserId(a), sid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(m3.evicted_up_to, 7, "{b} R4：happy 水位推进");
    assert_eq!(m3.summary.as_deref(), Some("z"), "{b} R4：happy 摘要");
    assert_eq!(
        ev_count(store.clone(), sid.0).await,
        1,
        "{b} R4：happy outbox 一条"
    );
}

#[tokio::test]
async fn evict_session_atomicity() {
    let sqlite = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    assert_evict_atomicity(Arc::new(Store::Sqlite(sqlite))).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_evict_{}", Uuid::new_v4().simple());
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
        assert_evict_atomicity(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}

/// R6 会话结束原子断言：越权/不存在 false 不删不发事件；happy 删除 + session-ended outbox + module_id 路由。
async fn assert_end_session(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let a = Uuid::new_v4();
    let other = Uuid::new_v4();
    kernel::repo::ensure_user(&store, a).await.unwrap();
    kernel::repo::ensure_user(&store, other).await.unwrap();
    let sessions = kernel::DbSessions::new(store.clone());
    let sid = kernel_api::SessionId::new();
    sessions
        .create(
            kernel_api::UserId(a),
            sid,
            kernel::SessionMeta {
                module_id: Some(kernel_api::ModuleId("teach-gojuon".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // 越权 end → false，session 仍在，无事件。
    assert!(
        !kernel::end_session(&store, other, sid.0).await.unwrap(),
        "{b} R6：越权 end false"
    );
    assert!(
        sessions.exists(kernel_api::UserId(a), sid).await.unwrap(),
        "{b} R6：越权未删"
    );
    // 不存在 end → false。
    assert!(
        !kernel::end_session(&store, a, Uuid::new_v4())
            .await
            .unwrap(),
        "{b} R6：不存在 false"
    );
    let ev0: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE session_id = $1",
        (i64,),
        sid.0
    )
    .unwrap();
    assert_eq!(ev0.0, 0, "{b} R6：越权/不存在不发事件");
    // happy end → 删除 + session-ended outbox（module_id 路由）。
    assert!(
        kernel::end_session(&store, a, sid.0).await.unwrap(),
        "{b} R6：end 成功"
    );
    assert!(
        !sessions.exists(kernel_api::UserId(a), sid).await.unwrap(),
        "{b} R6：end 后删除"
    );
    let ev: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE session_id = $1 AND kind = 'session-ended' AND module_id = 'teach-gojuon'",
        (i64,),
        sid.0
    )
    .unwrap();
    assert_eq!(ev.0, 1, "{b} R6：session-ended outbox + module_id 路由");
    // 无模块 → kernel 哨兵。
    let sid2 = kernel_api::SessionId::new();
    sessions
        .create(kernel_api::UserId(a), sid2, kernel::SessionMeta::default())
        .await
        .unwrap();
    kernel::end_session(&store, a, sid2.0).await.unwrap();
    let ev2: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE session_id = $1 AND kind = 'session-ended' AND module_id = 'kernel'",
        (i64,),
        sid2.0
    )
    .unwrap();
    assert_eq!(ev2.0, 1, "{b} R6：无模块 kernel 哨兵");
}

#[tokio::test]
async fn end_session_atomicity() {
    let sqlite = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    assert_end_session(Arc::new(Store::Sqlite(sqlite))).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_end_{}", Uuid::new_v4().simple());
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
        assert_end_session(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}
