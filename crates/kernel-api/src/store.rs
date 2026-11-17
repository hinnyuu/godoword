//! 存储分发契约：双具体驱动（SQLite / PostgreSQL），薄 enum 分发，**不隐藏方言**。
//!
//! 共享查询用「双兼容 SQL 子集」（见 docs/project/conventions.md）+ 运行时 `query_as`，
//! 类型安全由 Row 元组类型 + CI 双后端测试保证。编译期 `query!` 宏仅用于驱动专属代码
//! （单宏 pin 死单驱动，无法服务双后端共享代码）。方言点（自增/默认值/JSON 运算符）
//! 靠约定设计掉：uuid 主键、应用侧时间戳、JSON 整取回 Rust。

use sqlx::{PgPool, SqlitePool};

/// 模块 scoped 存储句柄：双具体驱动 enum + 表前缀约定 `mod_<id>_*`。
pub enum Store {
    Sqlite(SqlitePool),
    Pg(PgPool),
}

impl Store {
    /// 后端名（日志/测试用）。
    pub fn backend(&self) -> &'static str {
        match self {
            Store::Sqlite(_) => "sqlite",
            Store::Pg(_) => "postgres",
        }
    }
}

/// 双后端执行（INSERT/UPDATE/DELETE/DDL），返回受影响行数。
/// SQL 须属双兼容子集；绑定值按位置 `$1,$2…`（SQLite 由驱动翻译为 `?`）。
#[macro_export]
macro_rules! execute_both {
    ($store:expr, $sql:expr $(, $bind:expr)* $(,)?) => {{
        match $store {
            $crate::store::Store::Sqlite(p) => {
                let mut q = ::sqlx::query($sql);
                $( q = q.bind($bind); )*
                q.execute(p).await.map(|r| r.rows_affected())
            }
            $crate::store::Store::Pg(p) => {
                let mut q = ::sqlx::query($sql);
                $( q = q.bind($bind); )*
                q.execute(p).await.map(|r| r.rows_affected())
            }
        }
    }};
}

/// 双后端取单行（Row 须对两驱动皆实现 FromRow）。
#[macro_export]
macro_rules! fetch_one_both {
    ($store:expr, $sql:expr, $row:ty $(, $bind:expr)* $(,)?) => {{
        match $store {
            $crate::store::Store::Sqlite(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_one(p).await
            }
            $crate::store::Store::Pg(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_one(p).await
            }
        }
    }};
}

/// 双后端取可选单行。
#[macro_export]
macro_rules! fetch_optional_both {
    ($store:expr, $sql:expr, $row:ty $(, $bind:expr)* $(,)?) => {{
        match $store {
            $crate::store::Store::Sqlite(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_optional(p).await
            }
            $crate::store::Store::Pg(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_optional(p).await
            }
        }
    }};
}

/// 双后端取多行。
#[macro_export]
macro_rules! fetch_all_both {
    ($store:expr, $sql:expr, $row:ty $(, $bind:expr)* $(,)?) => {{
        match $store {
            $crate::store::Store::Sqlite(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_all(p).await
            }
            $crate::store::Store::Pg(p) => {
                let mut q = ::sqlx::query_as::<_, $row>($sql);
                $( q = q.bind($bind); )*
                q.fetch_all(p).await
            }
        }
    }};
}
