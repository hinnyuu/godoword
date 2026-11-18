//! auth-local 双后端测试（纪律➀）。PG 独立 schema 隔离避并行竞态。
//! 验证 argon2 round-trip + 注册首用户管理员 + username 唯一 + invite（用尽/过期）+ 登录（禁用）
//! + 会话令牌（创建/校验/登出）+ 管理员重置 + 用户状态。

use std::sync::Arc;

use auth_local::{
    admin_reset_password, create_invite, create_session, delete_session, hash_password, is_admin,
    list_users, register, set_user_status, validate_session, verify_login, verify_password,
    LocalAuth,
};
use chrono::{Duration, Utc};
use kernel::{run_migrations, KERNEL_MIGRATIONS};
use kernel_api::{AuthKind, AuthOutcome, AuthProvider, AuthRequest, Store};
use secrecy::SecretString;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

async fn run_test(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();

    // argon2 round-trip + 错密码。
    let h = hash_password("correct-password").unwrap();
    assert!(
        verify_password("correct-password", &h),
        "{b} argon2 正确密码"
    );
    assert!(!verify_password("wrong", &h), "{b} argon2 错密码");

    // 首用户注册 → 管理员（require_invite=true 但首用户豁免）。
    let admin = register(
        &store,
        "admin",
        "admin-pass-123",
        Some("管理员"),
        None,
        true,
    )
    .await
    .unwrap();
    assert!(is_admin(&store, admin).await.unwrap(), "{b} 首用户成管理员");

    // username 唯一。
    let dup = register(&store, "admin", "x-pass-12345", None, None, false).await;
    assert!(dup.is_err(), "{b} username 唯一拒绝重复");

    // 非首用户 require_invite 无码 → 拒绝。
    let no_invite = register(&store, "user1", "user-pass-123", None, None, true).await;
    assert!(no_invite.is_err(), "{b} invite 模式无码拒绝");

    // 邀请码注册（max_uses=1）。
    let code = create_invite(&store, admin, 1, None).await.unwrap();
    let user1 = register(&store, "user1", "user-pass-123", None, Some(&code), true)
        .await
        .unwrap();
    assert!(
        !is_admin(&store, user1).await.unwrap(),
        "{b} 邀请用户非管理员"
    );
    // 邀请码用尽 → 拒绝。
    let exhausted = register(&store, "user2", "user-pass-123", None, Some(&code), true).await;
    assert!(exhausted.is_err(), "{b} 邀请码用尽拒绝");

    // 过期邀请码 → 拒绝。
    let expired_code = create_invite(&store, admin, 5, Some(Utc::now() - Duration::hours(1)))
        .await
        .unwrap();
    let expired = register(
        &store,
        "user3",
        "user-pass-123",
        None,
        Some(&expired_code),
        true,
    )
    .await;
    assert!(expired.is_err(), "{b} 过期邀请码拒绝");

    // 开放注册（require_invite=false）。
    let user4 = register(&store, "user4", "user-pass-123", None, None, false)
        .await
        .unwrap();

    // 登录验证。
    let logged = verify_login(&store, "admin", "admin-pass-123")
        .await
        .unwrap();
    assert_eq!(logged, admin, "{b} 登录成功");
    assert!(
        verify_login(&store, "admin", "wrong").await.is_err(),
        "{b} 错密码登录拒绝"
    );
    assert!(
        verify_login(&store, "nobody", "x").await.is_err(),
        "{b} 不存在用户拒绝"
    );

    // AuthProvider.authenticate。
    let auth = LocalAuth::new(store.clone());
    assert_eq!(auth.id(), "auth-local");
    assert_eq!(auth.kind(), AuthKind::Local);
    let outcome = auth
        .authenticate(AuthRequest::Password {
            username: "admin".to_string(),
            password: SecretString::from("admin-pass-123".to_string()),
        })
        .await
        .unwrap();
    assert!(
        matches!(outcome, AuthOutcome::Authenticated(u) if u == admin),
        "{b} authenticate"
    );

    // 会话令牌：创建/校验/登出。
    let token = create_session(&store, admin, Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(
        validate_session(&store, &token).await.unwrap(),
        Some(admin),
        "{b} 会话校验"
    );
    assert_eq!(
        validate_session(&store, "bogus").await.unwrap(),
        None,
        "{b} 无效令牌 None"
    );
    delete_session(&store, &token).await.unwrap();
    assert_eq!(
        validate_session(&store, &token).await.unwrap(),
        None,
        "{b} 登出后失效"
    );

    // 过期会话 → None。
    let exp_token = create_session(&store, user1, Duration::seconds(-1))
        .await
        .unwrap();
    assert_eq!(
        validate_session(&store, &exp_token).await.unwrap(),
        None,
        "{b} 过期会话 None"
    );

    // 禁用账户 → 登录 Disabled。
    assert!(
        set_user_status(&store, user4, "disabled").await.unwrap(),
        "{b} 设状态"
    );
    assert!(
        verify_login(&store, "user4", "user-pass-123")
            .await
            .is_err(),
        "{b} 禁用登录拒绝"
    );

    // 管理员重置密码。
    assert!(
        admin_reset_password(&store, user1, "new-pass-12345")
            .await
            .unwrap(),
        "{b} 重置"
    );
    assert!(
        verify_login(&store, "user1", "new-pass-12345")
            .await
            .is_ok(),
        "{b} 新密码登录"
    );
    assert!(
        verify_login(&store, "user1", "user-pass-123")
            .await
            .is_err(),
        "{b} 旧密码失效"
    );

    // 用户列表。
    let users = list_users(&store).await.unwrap();
    assert!(users.len() >= 3, "{b} 用户列表");
    assert!(
        users.iter().any(|u| u.id == admin.0 && u.is_admin),
        "{b} 列表含管理员"
    );

    // 清理（级联删 credentials/sessions）。
    for u in [admin, user1, user4] {
        kernel_api::execute_both!(&*store, "DELETE FROM users WHERE id = $1", u.0).unwrap();
    }
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM invites WHERE created_by = $1",
        admin.0
    )
    .unwrap();
}

#[tokio::test]
async fn auth_local_flow() {
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    run_test(Arc::new(Store::Sqlite(sqlite))).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_auth_{}", Uuid::new_v4().simple());
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
