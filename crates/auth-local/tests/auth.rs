//! auth-local 双后端测试（纪律➀）。PG 独立 schema 隔离避并行竞态。
//! 验证 argon2 round-trip + 注册首用户管理员 + username 唯一 + invite（用尽/过期）+ 登录（禁用）
//! + 会话令牌（创建/校验/登出）+ 管理员重置 + 用户状态。

use std::sync::Arc;

use auth_local::{
    active_admin_count, admin_reset_password, change_password, create_invite, create_reset_token,
    create_session, delete_session, hash_password, is_admin, list_users, register,
    reset_with_token, set_user_status, set_user_status_guarded, validate_session, verify_login,
    verify_password, LocalAuth,
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

    // 密码策略（审计 R1）：register 拒绝弱密码（空/超短/纯字母/纯数字），接受字母+数字。
    for weak in ["", "a1", "abcdefgh", "12345678", "abc123"] {
        assert!(
            register(&store, "weakuser", weak, None, None, false)
                .await
                .is_err(),
            "{b} 弱密码拒绝：{weak:?}"
        );
    }
    let long = format!("a1{}", "x".repeat(127)); // 129 字符
    assert!(
        register(&store, "weakuser", &long, None, None, false)
            .await
            .is_err(),
        "{b} 超长密码拒绝"
    );
    // 区分大小写：Abcd1234 与 abcd1234 为不同密码。
    let case_user = register(&store, "caseuser", "Abcd1234", None, None, false)
        .await
        .unwrap();
    assert!(
        verify_login(&store, "caseuser", "Abcd1234").await.is_ok(),
        "{b} 大小写敏感正确密码"
    );
    assert!(
        verify_login(&store, "caseuser", "abcd1234").await.is_err(),
        "{b} 大小写敏感错密码"
    );

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

    // 禁用账户 → 登录 Disabled + 旧 cookie 失效（审计 C3）。
    let user4_token = create_session(&store, user4, Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(
        validate_session(&store, &user4_token).await.unwrap(),
        Some(user4),
        "{b} 禁用前会话有效"
    );
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
    assert_eq!(
        validate_session(&store, &user4_token).await.unwrap(),
        None,
        "{b} 禁用后旧 cookie 失效"
    );

    // 管理员重置密码 → 旧会话撤销（审计 C3）。
    let user1_token = create_session(&store, user1, Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(
        validate_session(&store, &user1_token).await.unwrap(),
        Some(user1),
        "{b} 重置前会话有效"
    );
    assert!(
        admin_reset_password(&store, user1, "new-pass-12345")
            .await
            .unwrap(),
        "{b} 重置"
    );
    assert_eq!(
        validate_session(&store, &user1_token).await.unwrap(),
        None,
        "{b} 重置后旧会话撤销"
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

    // 重置令牌 token 化（审计 C8）：单次使用 + 撤销旧会话。
    let rt_token = create_reset_token(&store, user1).await.unwrap();
    // 弱密码重置拒绝且不消费令牌（审计 R1：校验先于认领，令牌仍可重试）。
    assert!(
        reset_with_token(&store, &rt_token, "weak").await.is_err(),
        "{b} 弱密码重置拒绝"
    );
    assert!(
        reset_with_token(&store, &rt_token, "abcdefgh")
            .await
            .is_err(),
        "{b} 无数字重置拒绝"
    );
    let u1_sess = create_session(&store, user1, Duration::hours(1))
        .await
        .unwrap();
    assert!(
        reset_with_token(&store, &rt_token, "token-pass-123")
            .await
            .unwrap(),
        "{b} 令牌重置成功"
    );
    assert!(
        verify_login(&store, "user1", "token-pass-123")
            .await
            .is_ok(),
        "{b} 令牌新密码登录"
    );
    assert_eq!(
        validate_session(&store, &u1_sess).await.unwrap(),
        None,
        "{b} 令牌重置撤销旧会话"
    );
    assert!(
        !reset_with_token(&store, &rt_token, "again-pass-123")
            .await
            .unwrap(),
        "{b} 令牌单次使用"
    );
    assert!(
        !reset_with_token(&store, "bogus-token", "x-pass-12345")
            .await
            .unwrap(),
        "{b} 无效令牌拒绝"
    );

    // N4.3：无本地凭据账户拒绝签发重置令牌。
    let nocred = register(&store, "nocred-issue", "pass-12345", None, None, false)
        .await
        .unwrap();
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM local_credentials WHERE user_id = $1",
        nocred.0
    )
    .unwrap();
    assert!(
        create_reset_token(&store, nocred).await.is_err(),
        "{b} N4.3：无凭据账户拒绝签发"
    );

    // N4.2：改密 0 行回滚令牌认领（不消费、不假成功）。
    let u0 = register(&store, "nocred-reset", "pass-12345", None, None, false)
        .await
        .unwrap();
    let tok0 = create_reset_token(&store, u0).await.unwrap();
    kernel_api::execute_both!(
        &*store,
        "DELETE FROM local_credentials WHERE user_id = $1",
        u0.0
    )
    .unwrap();
    assert!(
        reset_with_token(&store, &tok0, "newpass-12345")
            .await
            .is_err(),
        "{b} N4.2：无凭据改密失败"
    );
    // 令牌未消费：恢复凭据后重试成功。
    let h0 = hash_password("restore-pass-123").unwrap();
    kernel_api::execute_both!(
        &*store,
        "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at) VALUES ($1,$2,$3,$4)",
        u0.0,
        h0,
        Utc::now(),
        Utc::now()
    )
    .unwrap();
    assert!(
        reset_with_token(&store, &tok0, "newpass-12345")
            .await
            .unwrap(),
        "{b} N4.2：令牌未消费可重试"
    );

    // 用户列表。
    let users = list_users(&store).await.unwrap();
    assert!(users.len() >= 3, "{b} 用户列表");
    assert!(
        users.iter().any(|u| u.id == admin.0 && u.is_admin),
        "{b} 列表含管理员"
    );

    // 自助改密（M1d）：验旧密 + 撤销其他会话保留当前 + 弱密码/错旧密拒绝。
    let chg = register(&store, "chgpwd", "orig-pass-123", None, None, false).await.unwrap();
    let cur_token = create_session(&store, chg, Duration::hours(1)).await.unwrap();
    let other_token = create_session(&store, chg, Duration::hours(1)).await.unwrap();
    // 错旧密拒绝。
    assert!(
        change_password(&store, chg, "wrong-old", "new-pass-123", Some(&cur_token)).await.is_err(),
        "{b} 错旧密拒绝"
    );
    // 弱新密码拒绝（不撤销会话）。
    assert!(
        change_password(&store, chg, "orig-pass-123", "weak", Some(&cur_token)).await.is_err(),
        "{b} 弱新密码拒绝"
    );
    assert_eq!(validate_session(&store, &other_token).await.unwrap(), Some(chg), "{b} 弱密码不撤销会话");
    // 成功改密：保留当前，撤销其他。
    assert!(
        change_password(&store, chg, "orig-pass-123", "new-pass-123", Some(&cur_token)).await.unwrap(),
        "{b} 改密成功"
    );
    assert_eq!(validate_session(&store, &cur_token).await.unwrap(), Some(chg), "{b} 当前会话保留");
    assert_eq!(validate_session(&store, &other_token).await.unwrap(), None, "{b} 其他会话撤销");
    assert!(verify_login(&store, "chgpwd", "new-pass-123").await.is_ok(), "{b} 新密码可登录");
    assert!(verify_login(&store, "chgpwd", "orig-pass-123").await.is_err(), "{b} 旧密码失效");

    // 清理（级联删 credentials/sessions）。
    for u in [admin, user1, user4, case_user, nocred, u0, chg] {
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

/// 并发重置断言（审计 R1）：N 个并发请求持同一令牌，仅一个成功，其余 false；新密码可登录；令牌已用。
async fn assert_concurrent_reset(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = register(&store, "resetme", "orig-pass-123", None, None, false)
        .await
        .unwrap();
    let token = create_reset_token(&store, user).await.unwrap();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = store.clone();
        let t = token.clone();
        handles.push(tokio::spawn(async move {
            reset_with_token(&s, &t, "new-pass-12345").await
        }));
    }
    let mut success = 0;
    for h in handles {
        if h.await.unwrap().unwrap() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "{b} 并发重置仅一次成功（实得 {success}）");
    assert!(
        verify_login(&store, "resetme", "new-pass-12345")
            .await
            .is_ok(),
        "{b} 重置后新密码可登录"
    );
    assert!(
        !reset_with_token(&store, &token, "another-pass-123")
            .await
            .unwrap(),
        "{b} 令牌已用拒绝再用"
    );
}

#[tokio::test]
async fn reset_token_concurrent_single_use() {
    // SQLite 文件库（多连接共享 + WAL 单写者串行，真实并发窗口）。
    let dir = std::env::temp_dir().join(format!("gw-reset-conc-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_concurrent_reset(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_reset_conc_{}", Uuid::new_v4().simple());
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
        assert_concurrent_reset(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}

/// 管理员状态守卫断言（审计 R7）：自禁拒绝 + 最后管理员拒绝 + 并发互禁后仍≥1 活跃管理员（不失锁）。
async fn assert_admin_guard(store: Arc<Store>) {
    let b = store.backend();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let a = register(&store, "admin-a", "pass-a-1234", None, None, false)
        .await
        .unwrap();
    let bb = register(&store, "admin-b", "pass-b-1234", None, None, false)
        .await
        .unwrap();
    // 测试 setup：提升 B 为管理员（生产首用户成管理员，这里直接置位构造两管理员）。
    kernel_api::execute_both!(
        &*store,
        "UPDATE users SET is_admin = $1 WHERE id = $2",
        true,
        bb.0
    )
    .unwrap();
    assert_eq!(
        active_admin_count(&store).await.unwrap(),
        2,
        "{b} 两活跃管理员"
    );
    // 自禁拒绝。
    assert!(
        set_user_status_guarded(&store, a, a, "disabled")
            .await
            .is_err(),
        "{b} 不可自禁"
    );
    // 串行：A 禁 B 成功（2→1）；B（已禁用）禁 A 被最后管理员守卫拒绝。
    assert!(
        set_user_status_guarded(&store, a, bb, "disabled")
            .await
            .unwrap(),
        "{b} A 禁 B"
    );
    assert_eq!(
        active_admin_count(&store).await.unwrap(),
        1,
        "{b} 禁 B 后剩 1"
    );
    assert!(
        set_user_status_guarded(&store, bb, a, "disabled")
            .await
            .is_err(),
        "{b} 不可禁最后管理员"
    );
    assert_eq!(
        active_admin_count(&store).await.unwrap(),
        1,
        "{b} 最后管理员守卫保住 1"
    );
    // 恢复 B 活跃 → 两管理员，供并发互禁。
    assert!(
        set_user_status_guarded(&store, a, bb, "active")
            .await
            .unwrap(),
        "{b} 恢复 B"
    );
    assert_eq!(
        active_admin_count(&store).await.unwrap(),
        2,
        "{b} 恢复两管理员"
    );
    // 并发互禁：恰一个成功，另一个被守卫拒绝；最终≥1 活跃管理员。
    let s1 = store.clone();
    let s2 = store.clone();
    let h1 = tokio::spawn(async move { set_user_status_guarded(&s1, a, bb, "disabled").await });
    let h2 = tokio::spawn(async move { set_user_status_guarded(&s2, bb, a, "disabled").await });
    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();
    let ok_count = [r1.is_ok(), r2.is_ok()].iter().filter(|x| **x).count();
    assert_eq!(ok_count, 1, "{b} 并发互禁恰一个成功（实得 {ok_count}）");
    let n = active_admin_count(&store).await.unwrap();
    assert!(n >= 1, "{b} 并发互禁后仍≥1 活跃管理员（实得 {n}）");
}

#[tokio::test]
async fn admin_status_guard_concurrent() {
    let dir = std::env::temp_dir().join(format!("gw-admin-conc-{}", Uuid::new_v4()));
    let sqlite = kernel::open_sqlite(&dir.join("db").join("t.sqlite"))
        .await
        .unwrap();
    assert_admin_guard(Arc::new(sqlite)).await;
    std::fs::remove_dir_all(&dir).unwrap();

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_admin_conc_{}", Uuid::new_v4().simple());
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
        assert_admin_guard(Arc::new(Store::Pg(pg))).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}
