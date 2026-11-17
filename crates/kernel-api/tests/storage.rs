//! 双后端存储契约测试（纪律➀：CI 真测 SQLite + PostgreSQL）。
//!
//! SQLite 始终跑（独立内存库）；PostgreSQL 在 TEST_PG_URL 设置时跑（CI storage job 必设）。
//! 各测试用独立表名，避免并行测试对共享 PG 库的 CREATE TABLE 竞态。
//! 验证契约核心类型 uuid/chrono/json/bool/float 双后端 round-trip 与迁移幂等。

use chrono::{DateTime, Utc};
use kernel_api::{execute_both, fetch_all_both, fetch_one_both, Store};
use serde_json::Value;
use sqlx::{PgPool, SqlitePool};
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

// 单一 PG 风格类型名迁移集，靠 SQLite 类型亲和性双后端通用（不用自增/DB 默认值）。
fn ddl(table: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            id UUID PRIMARY KEY,
            ts TIMESTAMPTZ NOT NULL,
            prefs JSONB,
            active BOOLEAN NOT NULL,
            score DOUBLE PRECISION
        )"
    )
}

#[tokio::test]
async fn roundtrip_core_types() {
    let table = "rt_core";
    let create = ddl(table);
    let insert =
        format!("INSERT INTO {table} (id, ts, prefs, active, score) VALUES ($1,$2,$3,$4,$5)");
    let select = format!("SELECT id, ts, prefs, active, score FROM {table} WHERE id = $1");
    let drop = format!("DROP TABLE IF EXISTS {table}");
    for store in backends().await {
        let b = store.backend();
        execute_both!(&store, &drop).unwrap();
        execute_both!(&store, &create).unwrap();
        // 迁移幂等：重跑不报错
        execute_both!(&store, &create).unwrap();

        let id = Uuid::new_v4();
        let ts = Utc::now();
        let prefs = serde_json::json!({"voice": "longanlingxin", "rate": 1.0});
        let affected = execute_both!(&store, &insert, id, ts, &prefs, true, 0.95f64).unwrap();
        assert_eq!(affected, 1, "{b} insert affected");

        let row: (Uuid, DateTime<Utc>, Value, bool, f64) =
            fetch_one_both!(&store, &select, (Uuid, DateTime<Utc>, Value, bool, f64), id).unwrap();
        assert_eq!(row.0, id, "{b} uuid round-trip");
        assert!(row.3, "{b} bool round-trip");
        assert!((row.4 - 0.95).abs() < 1e-9, "{b} float round-trip");
        assert_eq!(row.2["voice"], "longanlingxin", "{b} json round-trip");
        // 跨后端时间戳精度差（SQLite 纳秒/PG 微秒），毫秒级比较安全
        assert!(
            (row.1.timestamp_millis() - ts.timestamp_millis()).abs() < 1000,
            "{b} ts round-trip"
        );
    }
}

#[tokio::test]
async fn multi_row_and_optional() {
    let table = "rt_multi";
    let create = ddl(table);
    let insert =
        format!("INSERT INTO {table} (id, ts, prefs, active, score) VALUES ($1,$2,$3,$4,$5)");
    let select_all = format!("SELECT id FROM {table}");
    let select_one = format!("SELECT id FROM {table} WHERE id = $1");
    let drop = format!("DROP TABLE IF EXISTS {table}");
    for store in backends().await {
        let b = store.backend();
        execute_both!(&store, &drop).unwrap();
        execute_both!(&store, &create).unwrap();
        for i in 0..3 {
            let id = Uuid::new_v4();
            let ts = Utc::now();
            let prefs = serde_json::json!({"i": i});
            execute_both!(&store, &insert, id, ts, &prefs, true, i as f64).unwrap();
        }
        let rows: Vec<(Uuid,)> = fetch_all_both!(&store, &select_all, (Uuid,)).unwrap();
        assert_eq!(rows.len(), 3, "{b} fetch_all");

        let missing: Option<(Uuid,)> =
            kernel_api::fetch_optional_both!(&store, &select_one, (Uuid,), Uuid::new_v4()).unwrap();
        assert!(missing.is_none(), "{b} fetch_optional none");
    }
}
