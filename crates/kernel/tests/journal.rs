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
            module_id: None,
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
            module_id: None,
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
        &kernel_api::ProgressInput {
            kind: Some(kernel_api::EventKind::SessionEvicted),
            skill_tags: vec![],
            outcome: None,
            item_ref: None,
            difficulty: None,
            detail: json!({ "message_ids": ["m1", "m2"] }),
        },
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

/// N2 并发 end_session 唯一认领断言：8 并发恰一个 true、恰一个 session-ended 事件、会话已删。
async fn assert_end_concurrent(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let sessions = kernel::DbSessions::new(store.clone());
    let sid = kernel_api::SessionId::new();
    sessions
        .create(
            kernel_api::UserId(user),
            sid,
            kernel::SessionMeta::default(),
        )
        .await
        .unwrap();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            kernel::end_session(&s, user, sid.0).await
        }));
    }
    let mut ok = 0;
    for h in handles {
        if h.await.unwrap().unwrap() {
            ok += 1;
        }
    }
    assert_eq!(ok, 1, "{b} N2：并发 end_session 恰一个 true（实得 {ok}）");
    let ev: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE session_id = $1 AND kind = 'session-ended'",
        (i64,),
        sid.0
    )
    .unwrap();
    assert_eq!(
        ev.0, 1,
        "{b} N2：恰一个 session-ended 事件（实得 {}）",
        ev.0
    );
    assert!(
        !sessions
            .exists(kernel_api::UserId(user), sid)
            .await
            .unwrap(),
        "{b} N2：会话已删"
    );
}

#[tokio::test]
async fn end_session_concurrent_unique() {
    let dir = std::env::temp_dir().join(format!("gw-end-conc-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_end_concurrent(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_end_conc_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(4)
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
        assert_end_concurrent(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}

// ===== M2 P1b：结构化进度事件 + call_log 模块归因 + complete_session + teach-free 迁移 =====

async fn assert_progress_structured(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let ev_id = emit_progress(
        &store,
        user,
        None,
        "teach-gojuon",
        &kernel_api::ProgressInput {
            kind: Some(kernel_api::EventKind::ExerciseOutcome),
            skill_tags: vec!["kana:あ".to_string()],
            outcome: Some(kernel_api::Outcome::Correct),
            item_ref: Some("あ".to_string()),
            difficulty: Some(0.25),
            detail: json!({ "mode": "identify" }),
        },
    )
    .await
    .unwrap();
    let pending = pending_events(&store, 100).await.unwrap();
    let ev = pending
        .iter()
        .find(|e| e.id == ev_id)
        .expect("事件在 pending");
    assert_eq!(ev.kind, "exercise-outcome", "{b} kind kebab");
    assert_eq!(ev.outcome.as_deref(), Some("correct"), "{b} outcome 列");
    assert_eq!(ev.item_ref.as_deref(), Some("あ"), "{b} item_ref 列");
    assert_eq!(ev.difficulty, Some(0.25), "{b} difficulty REAL↔f32");
    assert_eq!(ev.skill_tags, Some(json!(["kana:あ"])), "{b} skill_tags");
    // for_user 事件无会话（B3：session_id 可空）。
    assert_eq!(ev.session_id, None, "{b} 无会话事件");

    // call_log 模块归因（B4）。
    record_call(
        &store,
        CallRecord {
            user_id: user,
            session_id: None,
            run_id: None,
            module_id: Some("teach-assess".to_string()),
            capability: "llm".to_string(),
            adapter_id: "mock".to_string(),
            model: "m".to_string(),
            prompt_tokens: Some(1),
            completion_tokens: Some(2),
            reasoning_tokens: None,
            total_tokens: Some(3),
            characters: None,
            latency_ms: 10,
            status: "ok".to_string(),
            error_code: None,
        },
    )
    .await
    .unwrap();
    let row: (Option<String>,) = fetch_one_both!(
        &*store,
        "SELECT module_id FROM call_log WHERE user_id = $1",
        (Option<String>,),
        user
    )
    .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("teach-assess"),
        "{b} call_log.module_id 归因"
    );
}

#[tokio::test]
async fn progress_structured_and_module_attribution() {
    let dir = std::env::temp_dir().join(format!("gw-p1b-prog-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_progress_structured(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_p1b_prog_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(4)
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
        assert_progress_structured(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过");
    }
}

async fn assert_complete_session(store: Arc<Store>) {
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
                module_id: Some(kernel_api::ModuleId("teach-free".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    sessions
        .append(
            kernel_api::UserId(a),
            sid,
            kernel_api::ChatMessage {
                role: "user".to_string(),
                content: "你好".to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            },
        )
        .await
        .unwrap();
    // 越权 complete → false，无事件。
    assert!(
        !kernel::complete_session(&store, other, sid.0)
            .await
            .unwrap(),
        "{b} 越权 false"
    );
    // 不存在 → false。
    assert!(
        !kernel::complete_session(&store, a, Uuid::new_v4())
            .await
            .unwrap(),
        "{b} 不存在 false"
    );
    // 完成 → true + session-ended 事件（module 归因）+ 转录保留（完成≠删除）。
    assert!(
        kernel::complete_session(&store, a, sid.0).await.unwrap(),
        "{b} 完成 true"
    );
    assert!(
        sessions.exists(kernel_api::UserId(a), sid).await.unwrap(),
        "{b} 完成不删会话"
    );
    let msgs = sessions.history(kernel_api::UserId(a), sid).await.unwrap();
    assert_eq!(msgs.len(), 1, "{b} 转录保留");
    let pending = pending_events(&store, 100).await.unwrap();
    let ended: Vec<_> = pending
        .iter()
        .filter(|e| e.kind == "session-ended")
        .collect();
    assert_eq!(ended.len(), 1, "{b} 恰一个 session-ended");
    assert_eq!(ended[0].module_id, "teach-free", "{b} 事件 module 归因");
    assert_eq!(ended[0].session_id, Some(sid.0), "{b} 事件带会话");
    // 幂等：重复完成 → false，不重复发事件。
    assert!(
        !kernel::complete_session(&store, a, sid.0).await.unwrap(),
        "{b} 重复完成 false"
    );
    let pending = pending_events(&store, 100).await.unwrap();
    assert_eq!(
        pending.iter().filter(|e| e.kind == "session-ended").count(),
        1,
        "{b} 事件不重复"
    );
}

#[tokio::test]
async fn complete_session_semantics() {
    let dir = std::env::temp_dir().join(format!("gw-p1b-comp-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_complete_session(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_p1b_comp_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(4)
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
        assert_complete_session(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过");
    }
}

async fn assert_complete_concurrent(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let sessions = kernel::DbSessions::new(store.clone());
    let sid = kernel_api::SessionId::new();
    sessions
        .create(
            kernel_api::UserId(user),
            sid,
            kernel::SessionMeta::default(),
        )
        .await
        .unwrap();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            kernel::complete_session(&s, user, sid.0).await.unwrap()
        }));
    }
    let mut trues = 0;
    for h in handles {
        if h.await.unwrap() {
            trues += 1;
        }
    }
    assert_eq!(trues, 1, "{b} 并发完成恰一个 true（条件 UPDATE 唯一认领）");
    let pending = pending_events(&store, 100).await.unwrap();
    assert_eq!(
        pending.iter().filter(|e| e.kind == "session-ended").count(),
        1,
        "{b} 并发恰一个事件"
    );
}

#[tokio::test]
async fn complete_session_concurrent_unique() {
    let dir = std::env::temp_dir().join(format!("gw-p1b-conc-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_complete_concurrent(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_p1b_conc_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(8)
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
        assert_complete_concurrent(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过");
    }
}

/// teach-free 迁移数据效果（B8）：0005 态 NULL 会话/default 文件夹 → 0006 后迁入 teach-free。
async fn assert_teach_free_migration(store: Arc<Store>) {
    let b = store.backend();
    // 先跑 0001–0005（旧态）。
    run_migrations(&store, &KERNEL_MIGRATIONS[..5], None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let sid = Uuid::new_v4();
    let now = chrono::Utc::now();
    kernel_api::execute_both!(
        &*store,
        "INSERT INTO sessions (id, user_id, module_id, on_disconnect, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6)",
        sid,
        user,
        Option::<String>::None,
        "cancel",
        now,
        now
    )
    .unwrap();
    kernel_api::execute_both!(
        &*store,
        "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6)",
        Uuid::new_v4(),
        user,
        "default",
        "/日语",
        now,
        now
    )
    .unwrap();
    // 0006 迁入。
    let applied = run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    assert_eq!(applied, 1, "{b} 仅 0006 待应用");
    let row: (Option<String>,) = fetch_one_both!(
        &*store,
        "SELECT module_id FROM sessions WHERE id = $1",
        (Option<String>,),
        sid
    )
    .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("teach-free"),
        "{b} NULL 会话迁入 teach-free"
    );
    let f: (String,) = fetch_one_both!(
        &*store,
        "SELECT module_id FROM session_folders WHERE user_id = $1",
        (String,),
        user
    )
    .unwrap();
    assert_eq!(f.0, "teach-free", "{b} default 文件夹迁入 teach-free");
    // 幂等重跑。
    assert_eq!(
        run_migrations(&store, KERNEL_MIGRATIONS, None)
            .await
            .unwrap(),
        0,
        "{b} 幂等"
    );
}

#[tokio::test]
async fn teach_free_migration_data() {
    let dir = std::env::temp_dir().join(format!("gw-p1b-mig-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_teach_free_migration(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_p1b_mig_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = PgPoolOptions::new()
            .max_connections(4)
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
        assert_teach_free_migration(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过");
    }
}
