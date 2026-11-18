//! 自定义双后端迁移 runner（sqlx `migrate!` 宏单驱动 pin，无法服务 Store 双驱动）。
//!
//! 迁移 SQL 编译期 `include_str!` 内嵌（单二进制、distroless 友好），按版本序逐语句执行
//! （双兼容子集 DDL，幂等 IF NOT EXISTS），版本表 `_schema_migrations` 追踪 + checksum 防漂移。
//! 迁移前自动备份：SQLite `VACUUM INTO`（WAL 一致快照）；PostgreSQL 镜像无 pg_dump 跳过+告警
//! （PG 部署自备外部备份，见 wiki/deploy）。runner 接受有序迁移列表，M2 模块迁移按注册序追加。

use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use kernel_api::{execute_both, fetch_all_both, KernelError, Store};
use sha2::{Digest, Sha256};

/// 一条迁移：版本序键 + 名 + 内嵌 SQL。
pub struct Migration {
    /// 版本键（如 "0001"），单调递增，决定执行序。
    pub version: &'static str,
    /// 迁移名（描述用）。
    pub name: &'static str,
    /// 迁移 SQL（双兼容子集，多语句以 `;` 分隔，幂等）。
    pub sql: &'static str,
}

/// kernel 自有迁移集（M2 教学模块迁移经注册序追加于其后）。
pub const KERNEL_MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0001",
        name: "init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: "0002",
        name: "auth",
        sql: include_str!("../migrations/0002_auth.sql"),
    },
    Migration {
        version: "0003",
        name: "eviction_watermark",
        sql: include_str!("../migrations/0003_eviction_watermark.sql"),
    },
];

const VERSION_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS _schema_migrations (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL
)";

/// SQL 内容 sha256 hex（漂移检测：已应用迁移的 SQL 被改动 → 报错）。
fn checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 按 `;` 切分迁移为单语句（DDL 无字面量内嵌分号）；跳过空与纯注释片段。
fn split_statements(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| {
            s.lines()
                .any(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        })
        .map(str::to_string)
        .collect()
}

/// 迁移前备份。SQLite 一致快照入 backup_dir；PG 无 pg_dump 跳过+告警。
async fn backup(store: &Store, backup_dir: &Path) -> Result<(), KernelError> {
    std::fs::create_dir_all(backup_dir)
        .map_err(|e| KernelError::Storage(format!("创建备份目录失败：{e}")))?;
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    match store {
        Store::Sqlite(pool) => {
            let target = backup_dir.join(format!("godoword-{stamp}.sqlite"));
            let target_str = target.to_string_lossy().into_owned();
            // VACUUM INTO 要求字面量文件名（不接受绑定参数）；路径完全受控（时间戳目录，无用户输入），
            // 单引号转义后内联。VACUUM 属管理语句，须用 raw_sql（exec 式）而非 query().execute()。
            // 此为 SQLite 专属方言点，收口于本备份 helper。
            let escaped = target_str.replace('\'', "''");
            let vacuum_sql = format!("VACUUM INTO '{escaped}'");
            sqlx::raw_sql(&vacuum_sql)
                .execute(pool)
                .await
                .map_err(|e| KernelError::Storage(format!("SQLite 备份失败：{e}")))?;
            tracing::info!("迁移前备份 SQLite → {target_str}");
        }
        Store::Pg(_) => {
            tracing::warn!(
                "PostgreSQL 迁移前备份跳过：运行时镜像无 pg_dump，请自备外部备份（见 wiki/deploy）"
            );
        }
    }
    Ok(())
}

/// 跑迁移：建版本表 → 校验已应用 checksum → 备份（有待应用时）→ 逐条应用 → 记录版本。
/// 返回本次应用的迁移数。幂等：重跑无待应用则零操作。
pub async fn run_migrations(
    store: &Store,
    migrations: &[Migration],
    backup_dir: Option<&Path>,
) -> Result<usize, KernelError> {
    execute_both!(store, VERSION_TABLE_DDL)
        .map_err(|e| KernelError::Storage(format!("建版本表失败：{e}")))?;

    let applied: Vec<(String, String)> = fetch_all_both!(
        store,
        "SELECT version, checksum FROM _schema_migrations",
        (String, String)
    )
    .map_err(|e| KernelError::Storage(format!("读版本表失败：{e}")))?;
    let applied_map: HashMap<String, String> = applied.into_iter().collect();

    for m in migrations {
        if let Some(cs) = applied_map.get(m.version) {
            if cs != &checksum(m.sql) {
                return Err(KernelError::Storage(format!(
                    "迁移 {} 内容漂移：已应用 checksum 与当前 SQL 不符（禁止改已应用迁移，应新增迁移）",
                    m.version
                )));
            }
        }
    }

    let pending: Vec<&Migration> = migrations
        .iter()
        .filter(|m| !applied_map.contains_key(m.version))
        .collect();
    if pending.is_empty() {
        return Ok(0);
    }

    if let Some(dir) = backup_dir {
        backup(store, dir).await?;
    }

    for m in &pending {
        for stmt in split_statements(m.sql) {
            execute_both!(store, &stmt)
                .map_err(|e| KernelError::Storage(format!("迁移 {} 执行失败：{e}", m.version)))?;
        }
        let now = Utc::now();
        execute_both!(
            store,
            "INSERT INTO _schema_migrations (version, name, checksum, applied_at) VALUES ($1,$2,$3,$4)",
            m.version,
            m.name,
            checksum(m.sql),
            now
        )
        .map_err(|e| KernelError::Storage(format!("记录迁移 {} 失败：{e}", m.version)))?;
        tracing::info!("已应用迁移 {}（{}）", m.version, m.name);
    }

    Ok(pending.len())
}
