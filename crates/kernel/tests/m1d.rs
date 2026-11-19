//! M1d 偏好/文件夹/会话元数据双后端测试（纪律➀）。PG 独立 schema 隔离避并行竞态。
//! 验证 preferences UPSERT + folders 虚拟路径（归一化/UNIQUE/子树改写/删非空上移/移动自动注册/防环）
//! + sessions-meta（title/count/last_active/文件夹过滤）。

use std::sync::Arc;

use kernel::{
    run_migrations, DbSessions, Preferences, SessionMeta, SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{ChatMessage, SessionId, Store, UserId};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

fn msg(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: content.to_string(),
        reasoning: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

async fn run_test(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    kernel::repo::ensure_user(&store, user).await.unwrap();
    let ns = kernel::DEFAULT_NAMESPACE;

    // ── preferences：默认 → put → get round-trip → UPSERT 覆盖。
    let default = kernel::get_preferences(&store, user).await.unwrap();
    assert!(default.theme.is_none(), "{b} 偏好默认空");
    let p1 = Preferences {
        theme: Some("dark".to_string()),
        auto_play: Some(true),
        ..Default::default()
    };
    kernel::put_preferences(&store, user, &p1).await.unwrap();
    let got = kernel::get_preferences(&store, user).await.unwrap();
    assert_eq!(got.theme.as_deref(), Some("dark"), "{b} 偏好 round-trip");
    assert_eq!(got.auto_play, Some(true), "{b} 偏好 auto_play");
    let p2 = Preferences {
        theme: Some("light".to_string()),
        ..Default::default()
    };
    kernel::put_preferences(&store, user, &p2).await.unwrap();
    let got2 = kernel::get_preferences(&store, user).await.unwrap();
    assert_eq!(got2.theme.as_deref(), Some("light"), "{b} UPSERT 覆盖");
    assert!(got2.auto_play.is_none(), "{b} UPSERT 整体替换");

    // ── folders：创建归一化 + UNIQUE 冲突。
    let f = kernel::create_folder(&store, user, ns, "/日语/五十音/")
        .await
        .unwrap();
    assert_eq!(f.path, "/日语/五十音", "{b} 路径归一化去尾斜杠");
    let dup = kernel::create_folder(&store, user, ns, "/日语/五十音").await;
    assert!(dup.is_err(), "{b} UNIQUE 冲突拒绝");
    let bad = kernel::create_folder(&store, user, ns, "/a/../b").await;
    assert!(bad.is_err(), "{b} 非法路径拒绝");

    // ── 会话 + 移动自动注册路径链。
    let sessions = DbSessions::new(store.clone());
    let s1 = SessionId::new();
    sessions
        .create(UserId(user), s1, SessionMeta::default())
        .await
        .unwrap();
    sessions
        .append(UserId(user), s1, msg("user", "教我こんにちは"))
        .await
        .unwrap();
    sessions
        .append(UserId(user), s1, msg("assistant", "好的"))
        .await
        .unwrap();
    let moved = kernel::move_session(&store, user, s1.0, Some("/日语/五十音"))
        .await
        .unwrap();
    assert!(moved, "{b} 移动会话成功");
    let folders = kernel::list_folders(&store, user, ns).await.unwrap();
    let paths: Vec<&str> = folders.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"/日语"), "{b} 自动注册祖先 /日语");
    assert!(paths.contains(&"/日语/五十音"), "{b} 自动注册 /日语/五十音");
    assert_eq!(
        kernel::get_session_path(&store, user, s1.0)
            .await
            .unwrap()
            .as_deref(),
        Some("/日语/五十音"),
        "{b} 会话 path 落库"
    );

    // ── sessions-meta：title/count/last_active + 文件夹过滤。
    let meta = kernel::list_sessions_meta(&store, user, None, None)
        .await
        .unwrap();
    let item = meta.iter().find(|m| m.id == s1.0).unwrap();
    assert_eq!(item.title, "教我こんにちは", "{b} title 首条用户消息");
    assert_eq!(item.message_count, 2, "{b} message_count");
    assert!(item.last_active.is_some(), "{b} last_active");
    let filtered = kernel::list_sessions_meta(&store, user, None, Some("/日语"))
        .await
        .unwrap();
    assert!(
        filtered.iter().any(|m| m.id == s1.0),
        "{b} 文件夹子树过滤命中"
    );
    let none = kernel::list_sessions_meta(&store, user, None, Some("/不存在"))
        .await
        .unwrap();
    assert!(none.is_empty(), "{b} 文件夹过滤无命中");

    // ── 第二会话入子文件夹，供改名/删除子树测试。
    let s2 = SessionId::new();
    sessions
        .create(UserId(user), s2, SessionMeta::default())
        .await
        .unwrap();
    sessions
        .append(UserId(user), s2, msg("user", "第二课"))
        .await
        .unwrap();
    kernel::move_session(&store, user, s2.0, Some("/日语/五十音/第一课"))
        .await
        .unwrap();

    // ── 改名文件夹子树批量改写（文件夹 + 会话）。
    let root = folders.iter().find(|f| f.path == "/日语").unwrap();
    kernel::rename_folder(&store, user, root.id, "/日本語")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        kernel::get_session_path(&store, user, s1.0)
            .await
            .unwrap()
            .as_deref(),
        Some("/日本語/五十音"),
        "{b} 改名后会话路径改写"
    );
    assert_eq!(
        kernel::get_session_path(&store, user, s2.0)
            .await
            .unwrap()
            .as_deref(),
        Some("/日本語/五十音/第一课"),
        "{b} 改名后深层会话路径改写"
    );
    let folders2 = kernel::list_folders(&store, user, ns).await.unwrap();
    assert!(
        folders2.iter().any(|f| f.path == "/日本語/五十音/第一课"),
        "{b} 子文件夹路径改写"
    );
    assert!(
        !folders2.iter().any(|f| f.path.starts_with("/日语")),
        "{b} 旧前缀清零"
    );

    // ── 防环：改名目标在源子树内拒绝。
    let root2 = folders2.iter().find(|f| f.path == "/日本語").unwrap();
    let cycle = kernel::rename_folder(&store, user, root2.id, "/日本語/五十音").await;
    assert!(cycle.is_err(), "{b} 防环拒绝");

    // ── 删非空文件夹：子树上移一层（绝不删会话）。
    let sub = folders2
        .iter()
        .find(|f| f.path == "/日本語/五十音")
        .unwrap();
    let deleted = kernel::delete_folder(&store, user, sub.id).await.unwrap();
    assert!(deleted, "{b} 删文件夹成功");
    assert_eq!(
        kernel::get_session_path(&store, user, s1.0)
            .await
            .unwrap()
            .as_deref(),
        Some("/日本語"),
        "{b} 删后直接会话上移一层"
    );
    assert_eq!(
        kernel::get_session_path(&store, user, s2.0)
            .await
            .unwrap()
            .as_deref(),
        Some("/日本語/第一课"),
        "{b} 删后深层会话上移一层"
    );
    assert!(
        sessions.exists(UserId(user), s1).await.unwrap(),
        "{b} 会话未被级联删"
    );
    assert!(
        sessions.exists(UserId(user), s2).await.unwrap(),
        "{b} 会话未被级联删 2"
    );

    // ── 移动回未分类（NULL）。
    kernel::move_session(&store, user, s1.0, None)
        .await
        .unwrap();
    assert!(
        kernel::get_session_path(&store, user, s1.0)
            .await
            .unwrap()
            .is_none(),
        "{b} 回未分类 NULL"
    );

    // ── 越权负例：他人不可移动/删本用户文件夹/会话。
    let other = Uuid::new_v4();
    kernel::repo::ensure_user(&store, other).await.unwrap();
    assert!(
        !kernel::move_session(&store, other, s2.0, Some("/x"))
            .await
            .unwrap(),
        "{b} 越权移动 false"
    );
    let remain = kernel::list_folders(&store, user, ns).await.unwrap();
    let any = remain.first().unwrap();
    assert!(
        !kernel::delete_folder(&store, other, any.id).await.unwrap(),
        "{b} 越权删文件夹 false"
    );

    // 清理。
    kernel_api::execute_both!(&*store, "DELETE FROM sessions WHERE user_id = $1", user).unwrap();
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM session_folders WHERE user_id = $1",
        user
    )
    .unwrap();
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM user_preferences WHERE user_id = $1",
        user
    )
    .unwrap();
    kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", user).unwrap();
    kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", other).unwrap();
}

#[tokio::test]
async fn m1d_preferences_folders_meta() {
    let sqlite = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    run_test(Arc::new(Store::Sqlite(sqlite))).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_m1d_{}", Uuid::new_v4().simple());
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
