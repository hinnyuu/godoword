//! DB 会话仓储双后端测试（纪律➀）。
//!
//! SQLite 恒跑（内存库 + foreign_keys ON 使级联生效）；PG 用**独立 schema 隔离**（search_path），
//! 避免与 storage/binding 测试的 public schema 重置并行竞态。验证 create/meta round-trip +
//! history seq 顺序 + list 用户隔离 + exists + delete 级联删 messages。

use std::sync::Arc;

use kernel::{
    run_migrations, DbSessions, OnDisconnect, SessionMeta, SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{ChatMessage, Eviction, ModuleId, SessionId, SessionParams, Store, UserId};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

async fn run_test(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let repo = DbSessions::new(store.clone());

    let user = UserId::new();
    let other = UserId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    kernel::repo::ensure_user(&store, other.0).await.unwrap();

    // create 携带元数据 → meta round-trip（module_id/on_disconnect/params）。
    let sid = SessionId::new();
    let meta = SessionMeta {
        module_id: Some(ModuleId("teach-gojuon".to_string())),
        on_disconnect: OnDisconnect::Continue,
        params: SessionParams {
            context_budget_tokens: 4000,
            eviction: Eviction::Summarize,
            tool_iteration_cap: 5,
        },
        summary: Some("先前摘要".to_string()),
        evicted_up_to: 7,
    };
    repo.create(user, sid, meta.clone()).await.unwrap();
    let got = repo.meta(user, sid).await.unwrap().expect("meta 存在");
    assert_eq!(got.module_id, meta.module_id, "{b} module_id round-trip");
    assert_eq!(
        got.on_disconnect,
        OnDisconnect::Continue,
        "{b} on_disconnect round-trip"
    );
    assert_eq!(got.params.context_budget_tokens, 4000, "{b} params 预算");
    assert_eq!(got.params.eviction, Eviction::Summarize, "{b} params 淘汰");
    assert_eq!(got.params.tool_iteration_cap, 5, "{b} params 工具上限");
    assert_eq!(
        got.summary.as_deref(),
        Some("先前摘要"),
        "{b} summary round-trip"
    );
    assert_eq!(got.evicted_up_to, 7, "{b} 水位 round-trip");
    // evict_session 原子推进水位 + 滚动摘要 + outbox 同事务（审计 C7/R4）。
    kernel::evict_session(
        &store,
        user.0,
        sid.0,
        "teach-gojuon",
        Some("新摘要".to_string()),
        12,
        &["m1".to_string()],
    )
    .await
    .unwrap();
    let got_w = repo.meta(user, sid).await.unwrap().unwrap();
    assert_eq!(got_w.summary.as_deref(), Some("新摘要"), "{b} 水位更新摘要");
    assert_eq!(got_w.evicted_up_to, 12, "{b} 水位推进");
    let ev: (i64,) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE session_id = $1 AND kind = 'session-evicted'",
        (i64,),
        sid.0
    )
    .unwrap();
    assert_eq!(ev.0, 1, "{b} evict_session outbox 同事务落事件");
    assert!(repo.exists(user, sid).await.unwrap(), "{b} exists");
    assert!(
        repo.meta(user, SessionId::new()).await.unwrap().is_none(),
        "{b} meta 不存在 None"
    );

    // 默认元数据（None module / cancel / 默认 params）。
    let sid2 = SessionId::new();
    repo.create(user, sid2, SessionMeta::default())
        .await
        .unwrap();
    let got2 = repo.meta(user, sid2).await.unwrap().unwrap();
    assert!(got2.module_id.is_none(), "{b} 默认 module None");
    assert_eq!(got2.on_disconnect, OnDisconnect::Cancel, "{b} 默认 cancel");

    // append → history seq 顺序 + JSON 整取 + 可空列。
    let m1 = ChatMessage {
        role: "user".to_string(),
        content: "教我こんにちは".to_string(),
        reasoning: None,
        tool_calls: None,
        tool_call_id: None,
    };
    let m2 = ChatMessage {
        role: "assistant".to_string(),
        content: "好的".to_string(),
        reasoning: Some("思考".to_string()),
        tool_calls: Some(json!([{"id":"call_1","name":"speak","arguments":{"text":"こんにちは"}}])),
        tool_call_id: None,
    };
    let m3 = ChatMessage {
        role: "tool".to_string(),
        content: "{\"audio_id\":\"x\"}".to_string(),
        reasoning: None,
        tool_calls: None,
        tool_call_id: Some("call_1".to_string()),
    };
    repo.append(user, sid, m1).await.unwrap();
    repo.append(user, sid, m2).await.unwrap();
    repo.append(user, sid, m3).await.unwrap();
    let hist = repo.history(user, sid).await.unwrap();
    assert_eq!(hist.len(), 3, "{b} history 条数");
    assert_eq!(hist[0].role, "user", "{b} seq 顺序 0");
    assert_eq!(hist[1].role, "assistant", "{b} seq 顺序 1");
    assert_eq!(
        hist[1].reasoning.as_deref(),
        Some("思考"),
        "{b} reasoning 可空 round-trip"
    );
    assert_eq!(
        hist[1].tool_calls.as_ref().unwrap()[0]["name"],
        "speak",
        "{b} tool_calls JSON 整取"
    );
    assert_eq!(
        hist[2].tool_call_id.as_deref(),
        Some("call_1"),
        "{b} tool_call_id round-trip"
    );

    // list 用户隔离。
    let list = repo.list(user).await.unwrap();
    assert_eq!(list.len(), 2, "{b} list 本用户两会话");
    assert!(
        repo.list(other).await.unwrap().is_empty(),
        "{b} list 用户隔离"
    );

    // 两用户越权负例（审计 C1）：other 不可读/写/删 user 的会话。
    assert!(
        repo.meta(other, sid).await.unwrap().is_none(),
        "{b} 越权 meta None"
    );
    assert!(
        !repo.exists(other, sid).await.unwrap(),
        "{b} 越权 exists false"
    );
    assert!(
        repo.history(other, sid).await.unwrap().is_empty(),
        "{b} 越权 history 空"
    );
    assert!(
        repo.history_with_ids(other, sid).await.unwrap().is_empty(),
        "{b} 越权 history_with_ids 空"
    );
    let bad_append = repo
        .append(
            other,
            sid,
            ChatMessage {
                role: "user".to_string(),
                content: "越权写入".to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            },
        )
        .await;
    assert!(bad_append.is_err(), "{b} 越权 append 拒绝");
    repo.delete(other, sid).await.unwrap(); // 非本用户删除无效果
    assert!(
        repo.exists(user, sid).await.unwrap(),
        "{b} 越权 delete 不生效"
    );
    assert_eq!(
        repo.history(user, sid).await.unwrap().len(),
        3,
        "{b} 越权未写入"
    );

    // delete 级联删 messages（外键 CASCADE）。
    repo.delete(user, sid).await.unwrap();
    assert!(
        !repo.exists(user, sid).await.unwrap(),
        "{b} delete 后 exists false"
    );
    assert!(
        repo.history(user, sid).await.unwrap().is_empty(),
        "{b} delete 级联 messages"
    );
    let cnt: (i64,) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM messages WHERE session_id = $1",
        (i64,),
        sid.0
    )
    .unwrap();
    assert_eq!(cnt.0, 0, "{b} messages 级联清空");

    // 清理。
    repo.delete(user, sid2).await.unwrap();
    kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", user.0).unwrap();
    kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", other.0).unwrap();
}

#[tokio::test]
async fn session_repo_crud() {
    // SQLite：内存库 + foreign_keys ON（级联生效）。
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    run_test(Arc::new(Store::Sqlite(sqlite))).await;

    // PG：独立 schema 隔离（避与 storage/binding 的 public 重置竞态）。
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_sess_{}", Uuid::new_v4().simple());
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
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过——仅测 SQLite（CI storage job 必设此变量跑双后端）");
    }
}
