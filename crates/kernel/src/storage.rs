//! Store pool 初始化（双具体驱动）。SQLite 默认（WAL + 外键启用，`/data/db`），
//! PostgreSQL 外接（连接串）。池句柄包进 kernel_api::Store 供薄分发宏跑双兼容子集。

use std::path::Path;

use kernel_api::{execute_both, KernelError, Store};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::PgPool;

/// 存储连通探活（SELECT 1，readyz 用）。
pub async fn ping(store: &Store) -> Result<(), KernelError> {
    execute_both!(store, "SELECT 1")
        .map_err(|e| KernelError::Storage(format!("存储探活失败：{e}")))?;
    Ok(())
}

/// 打开 SQLite（WAL 日志模式 + 外键约束启用，文件缺失自动建）。
pub async fn open_sqlite(db_path: &Path) -> Result<Store, KernelError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| KernelError::Storage(format!("建数据库目录失败：{e}")))?;
    }
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(|e| KernelError::Storage(format!("打开 SQLite 失败：{e}")))?;
    Ok(Store::Sqlite(pool))
}

/// 打开 PostgreSQL（外接连接串）。
pub async fn open_pg(url: &str) -> Result<Store, KernelError> {
    let pool = PgPool::connect(url)
        .await
        .map_err(|e| KernelError::Storage(format!("连接 PostgreSQL 失败：{e}")))?;
    Ok(Store::Pg(pool))
}
