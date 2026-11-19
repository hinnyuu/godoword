//! kernel 存储基底双后端测试（纪律➀：CI 真测 SQLite + PostgreSQL）。
//!
//! SQLite 恒跑（独立内存库）；PostgreSQL 在 TEST_PG_URL 设置时跑（CI storage job 必设）。
//! schema 表名固定，故 schema CRUD 集中于单测试函数内顺序遍历两后端，避免并行测试对共享 PG 库竞态。
//! 验证：迁移幂等 + 版本表 + checksum 漂移拒绝 + 全表 CRUD round-trip（uuid/chrono/json/bool/bytea）
//! + bindings 哨兵 `*` UNIQUE 约束 + SQLite 备份文件生成。

use chrono::{DateTime, Utc};
use kernel::{run_migrations, Migration, KERNEL_MIGRATIONS};
use kernel_api::{execute_both, fetch_all_both, fetch_one_both, fetch_optional_both, Store};
use serde_json::Value;
use sqlx::{PgPool, SqlitePool};
use std::sync::Arc;
use uuid::Uuid;

async fn backends() -> Vec<Store> {
    let mut v = Vec::new();
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    v.push(Store::Sqlite(sqlite));
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let pg = PgPool::connect(&url).await.unwrap();
        v.push(Store::Pg(pg));
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过——仅测 SQLite（CI storage job 必设此变量跑双后端）");
    }
    v
}

#[tokio::test]
async fn migrations_and_schema_crud() {
    for store in backends().await {
        let b = store.backend();

        // 测试专属重置：PG 库跨本地运行持久（CI service 每次新建），重置 public schema 保证 fresh；
        // SQLite 内存库每次新建无需重置。此为测试 setup 的后端专属操作，生产共享 SQL 仍守双兼容子集。
        if let Store::Pg(p) = &store {
            sqlx::query("DROP SCHEMA public CASCADE")
                .execute(p)
                .await
                .unwrap();
            sqlx::query("CREATE SCHEMA public")
                .execute(p)
                .await
                .unwrap();
        }

        // 迁移幂等：首次应用 4 条，重跑 0 条。
        let n1 = run_migrations(&store, KERNEL_MIGRATIONS, None)
            .await
            .unwrap();
        assert_eq!(n1, 4, "{b} 首次应用迁移数");
        let n2 = run_migrations(&store, KERNEL_MIGRATIONS, None)
            .await
            .unwrap();
        assert_eq!(n2, 0, "{b} 重跑幂等零操作");

        let versions: Vec<(String, String)> = fetch_all_both!(
            &store,
            "SELECT version, name FROM _schema_migrations ORDER BY version",
            (String, String)
        )
        .unwrap();
        assert_eq!(versions.len(), 4, "{b} 版本表行数");
        assert_eq!(versions[0].0, "0001", "{b} 版本键 0001");
        assert_eq!(versions[1].0, "0002", "{b} 版本键 0002");
        assert_eq!(versions[2].0, "0003", "{b} 版本键 0003");
        assert_eq!(versions[3].0, "0004", "{b} 版本键 0004");

        let now = Utc::now();
        let user_id = Uuid::new_v4();
        execute_both!(
            &store,
            "INSERT INTO users (id, email, display_name, is_admin, status, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
            user_id,
            Option::<String>::None,
            "测试用户",
            true,
            "active",
            now,
            now
        )
        .unwrap();

        // provider：key_enc BYTEA round-trip（nonce+密文+tag 占位字节）。
        let provider_id = Uuid::new_v4();
        let key_enc: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let extra = serde_json::json!({"region": "cn"});
        execute_both!(
            &store,
            "INSERT INTO providers (id, user_id, adapter_id, label, base_url, key_enc, extra_json, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
            provider_id,
            user_id,
            "llm-openai",
            "我的 Qwen",
            "https://example.com/compatible-mode/v1",
            &key_enc,
            &extra,
            now,
            now
        )
        .unwrap();

        // binding：哨兵 '*' 全模块默认 + params_json 整取。
        let binding_id = Uuid::new_v4();
        let params = serde_json::json!({"voice": "longanlingxin", "rate": 1.0});
        execute_both!(
            &store,
            "INSERT INTO bindings (id, user_id, module_id, capability, role, provider_id, model, params_json, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            binding_id,
            user_id,
            "*",
            "llm",
            "default",
            provider_id,
            "qwen4-max-preview",
            &params,
            now,
            now
        )
        .unwrap();

        // round-trip：uuid/json/bytea（provider）+ uuid/bool/可空/timestamp（user）。
        let prov: (Uuid, Vec<u8>, Value) = fetch_one_both!(
            &store,
            "SELECT id, key_enc, extra_json FROM providers WHERE id = $1",
            (Uuid, Vec<u8>, Value),
            provider_id
        )
        .unwrap();
        assert_eq!(prov.0, provider_id, "{b} provider uuid round-trip");
        assert_eq!(prov.1, key_enc, "{b} key_enc BYTEA round-trip");
        assert_eq!(prov.2["region"], "cn", "{b} provider extra_json round-trip");

        let usr: (Uuid, bool, Option<String>, DateTime<Utc>) = fetch_one_both!(
            &store,
            "SELECT id, is_admin, email, created_at FROM users WHERE id = $1",
            (Uuid, bool, Option<String>, DateTime<Utc>),
            user_id
        )
        .unwrap();
        assert_eq!(usr.0, user_id, "{b} user uuid");
        assert!(usr.1, "{b} user bool round-trip");
        assert!(usr.2.is_none(), "{b} user email 可空");
        assert!(
            (usr.3.timestamp_millis() - now.timestamp_millis()).abs() < 1000,
            "{b} user ts round-trip"
        );

        let bind: (Uuid, String, Value) = fetch_one_both!(
            &store,
            "SELECT id, module_id, params_json FROM bindings WHERE id = $1",
            (Uuid, String, Value),
            binding_id
        )
        .unwrap();
        assert_eq!(bind.1, "*", "{b} binding 哨兵 *");
        assert_eq!(
            bind.2["voice"], "longanlingxin",
            "{b} binding params_json 整取"
        );

        // UNIQUE(user,module,cap,role)：重复默认绑定须拒绝（哨兵 * 使约束两后端生效）。
        let dup = execute_both!(
            &store,
            "INSERT INTO bindings (id, user_id, module_id, capability, role, provider_id, model, params_json, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            Uuid::new_v4(),
            user_id,
            "*",
            "llm",
            "default",
            provider_id,
            "other-model",
            &params,
            now,
            now
        );
        assert!(dup.is_err(), "{b} 重复默认绑定须被 UNIQUE 拒绝");

        // sessions + messages：seq UNIQUE + JSON 整取 + 可空列。
        let session_id = Uuid::new_v4();
        execute_both!(
            &store,
            "INSERT INTO sessions (id, user_id, module_id, plan_version, on_disconnect, params_json, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            session_id,
            user_id,
            Option::<String>::None,
            Option::<String>::None,
            "cancel",
            Option::<Value>::None,
            now,
            now
        )
        .unwrap();
        let msg_id = Uuid::new_v4();
        let tool_calls = serde_json::json!([{"id": "call_1", "name": "speak", "arguments": {"text": "こんにちは"}}]);
        execute_both!(
            &store,
            "INSERT INTO messages (id, session_id, seq, role, content, reasoning, tool_calls, tool_call_id, audio_refs, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            msg_id,
            session_id,
            1i32,
            "assistant",
            "你好",
            Option::<String>::None,
            &tool_calls,
            Option::<String>::None,
            Option::<Value>::None,
            now
        )
        .unwrap();
        let msg: (Uuid, i32, Value, Option<String>) = fetch_one_both!(
            &store,
            "SELECT id, seq, tool_calls, reasoning FROM messages WHERE id = $1",
            (Uuid, i32, Value, Option<String>),
            msg_id
        )
        .unwrap();
        assert_eq!(msg.1, 1, "{b} message seq");
        assert_eq!(
            msg.2[0]["name"], "speak",
            "{b} message tool_calls JSON 整取"
        );
        assert!(msg.3.is_none(), "{b} message reasoning 可空");

        let missing: Option<(Uuid,)> = fetch_optional_both!(
            &store,
            "SELECT id FROM users WHERE id = $1",
            (Uuid,),
            Uuid::new_v4()
        )
        .unwrap();
        assert!(missing.is_none(), "{b} fetch_optional none");

        // 清理本测试插入的行（共享 PG 库整洁；随机 uuid 已避并行竞态）。
        execute_both!(
            &store,
            "DELETE FROM messages WHERE session_id = $1",
            session_id
        )
        .unwrap();
        execute_both!(&store, "DELETE FROM sessions WHERE id = $1", session_id).unwrap();
        execute_both!(&store, "DELETE FROM bindings WHERE user_id = $1", user_id).unwrap();
        execute_both!(&store, "DELETE FROM providers WHERE id = $1", provider_id).unwrap();
        execute_both!(&store, "DELETE FROM users WHERE id = $1", user_id).unwrap();
    }
}

#[tokio::test]
async fn sqlite_backup_creates_file() {
    // 备份为后端真方言点：仅 SQLite 测 VACUUM INTO 生成文件（PG 跳过+告警，无文件）。
    // 用生产路径 open_sqlite（文件库 + WAL + 多连接共享同一文件）；:memory: 池连接隔离不适用 VACUUM。
    let root = std::env::temp_dir().join(format!("gw-migrate-backup-{}", Uuid::new_v4()));
    let db_path = root.join("db").join("godoword.sqlite");
    let store = kernel::open_sqlite(&db_path).await.unwrap();
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();

    let backup_dir = root.join("backups");
    // 有待应用迁移时触发备份：用一条新增迁移。
    let extra = [Migration {
        version: "9999",
        name: "backup_probe",
        sql: "CREATE TABLE IF NOT EXISTS _backup_probe (id UUID PRIMARY KEY)",
    }];
    let n = run_migrations(&store, &extra, Some(&backup_dir))
        .await
        .unwrap();
    assert_eq!(n, 1);
    let files: Vec<_> = std::fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "sqlite").unwrap_or(false))
        .collect();
    assert_eq!(files.len(), 1, "SQLite 备份应生成 1 个 .sqlite 文件");
    // 备份文件可读且含已迁移 schema（验证一致快照非空）。
    let backup_store = kernel::open_sqlite(&files[0].path()).await.unwrap();
    let cnt: (i64,) = fetch_one_both!(
        &backup_store,
        "SELECT COUNT(*) FROM _schema_migrations",
        (i64,)
    )
    .unwrap();
    assert!(cnt.0 >= 1, "备份含版本表");
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn checksum_drift_rejected() {
    // runner 逻辑测试（SQLite 独立内存库，不污染共享 PG 版本表）。
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = Store::Sqlite(sqlite);
    let v1 = [Migration {
        version: "0001",
        name: "probe",
        sql: "CREATE TABLE IF NOT EXISTS _drift_probe (id UUID PRIMARY KEY)",
    }];
    run_migrations(&store, &v1, None).await.unwrap();
    // 同版本改 SQL → 漂移拒绝。
    let v1_changed = [Migration {
        version: "0001",
        name: "probe",
        sql: "CREATE TABLE IF NOT EXISTS _drift_probe (id UUID PRIMARY KEY, extra TEXT)",
    }];
    let err = run_migrations(&store, &v1_changed, None).await;
    assert!(err.is_err(), "已应用迁移内容漂移须拒绝");
}

/// N8/A2 迁移失败回滚探针：第二语句失败 → 首 DDL + 版本记录均回滚，修正后可重跑（双后端）。
async fn assert_migration_rollback(store: Arc<Store>, probe: &Migration, fixed: &Migration) {
    let b = store.backend();
    let r = run_migrations(&store, std::slice::from_ref(probe), None).await;
    assert!(r.is_err(), "{b} N8：第二语句失败迁移报错");
    // 首 DDL 回滚：probe_rb 不存在（查询报错）。
    let q = fetch_one_both!(&*store, "SELECT COUNT(*) FROM probe_rb", (i64,));
    assert!(q.is_err(), "{b} N8：首 DDL 回滚（probe_rb 不存在）");
    // 版本记录回滚。
    let ver: (i64,) = fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM _schema_migrations WHERE version = '9999'",
        (i64,)
    )
    .unwrap();
    assert_eq!(ver.0, 0, "{b} N8：版本记录回滚");
    // 修正后重跑成功。
    let n = run_migrations(&store, std::slice::from_ref(fixed), None)
        .await
        .unwrap();
    assert_eq!(n, 1, "{b} N8：修正后重跑应用 1 条");
    let cnt: (i64,) = fetch_one_both!(&*store, "SELECT COUNT(*) FROM probe_rb", (i64,)).unwrap();
    assert_eq!(cnt.0, 0, "{b} N8：修正后 probe_rb 存在");
}

#[tokio::test]
async fn migration_failure_rolls_back_ddl_and_version() {
    let probe = Migration {
        version: "9999",
        name: "probe_rb",
        sql:
            "CREATE TABLE probe_rb (id UUID PRIMARY KEY); INSERT INTO gw_no_such_table VALUES (1);",
    };
    let fixed = Migration {
        version: "9999",
        name: "probe_rb",
        sql: "CREATE TABLE probe_rb (id UUID PRIMARY KEY);",
    };
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    assert_migration_rollback(Arc::new(Store::Sqlite(sqlite)), &probe, &fixed).await;

    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let schema = format!("gw_migr_rb_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let s = schema.clone();
        let pg = sqlx::postgres::PgPoolOptions::new()
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
        assert_migration_rollback(Arc::new(Store::Pg(pg)), &probe, &fixed).await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过（CI storage job 必设跑双后端）");
    }
}
