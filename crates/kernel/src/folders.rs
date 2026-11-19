//! 会话文件夹仓储（M1d，虚拟路径即身份）。登记表持久空文件夹；模块 chroot 命名空间。
//! 改名/移动=前缀批量改写（Rust 侧匹配 + 单事务原子）；删非空文件夹→子树上移一层（绝不级联删会话）。
//! 读经薄分发宏（pool），写收口驱动专属事务（原子）；私有部署单用户自组织，写事务保完整性。

use chrono::{DateTime, Utc};
use kernel_api::{execute_both, fetch_all_both, fetch_optional_both, KernelError, Store};
use serde::Serialize;
use uuid::Uuid;

use crate::path::{self, PathError};

/// 文件夹错误（REST 映射：Invalid/Conflict/Cycle→400，Storage→500）。
#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error(transparent)]
    Invalid(#[from] PathError),
    #[error("文件夹路径已存在：{0}")]
    Conflict(String),
    #[error("目标不可在源子树内：{0}")]
    Cycle(String),
    #[error("存储错误：{0}")]
    Storage(String),
}

fn storage_err(e: impl std::fmt::Display) -> FolderError {
    FolderError::Storage(e.to_string())
}

/// 文件夹域类型（登记表行）。
#[derive(Debug, Clone, sqlx::FromRow, Serialize, utoipa::ToSchema)]
pub struct Folder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub module_id: String,
    pub path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const FOLDER_COLS: &str = "id, user_id, module_id, path, created_at, updated_at";

/// 列出用户某命名空间文件夹。
pub async fn list_folders(
    store: &Store,
    user: Uuid,
    module: &str,
) -> Result<Vec<Folder>, KernelError> {
    let sql = format!(
        "SELECT {FOLDER_COLS} FROM session_folders WHERE user_id = $1 AND module_id = $2 ORDER BY path"
    );
    fetch_all_both!(store, &sql, Folder, user, module)
        .map_err(|e| KernelError::Storage(e.to_string()))
}

/// 取用户单个文件夹（归属校验）。
pub async fn get_folder(
    store: &Store,
    user: Uuid,
    id: Uuid,
) -> Result<Option<Folder>, KernelError> {
    let sql = format!("SELECT {FOLDER_COLS} FROM session_folders WHERE id = $1 AND user_id = $2");
    fetch_optional_both!(store, &sql, Folder, id, user)
        .map_err(|e| KernelError::Storage(e.to_string()))
}

/// 确保文件夹路径链存在（移动会话自动注册祖先；ON CONFLICT DO NOTHING 幂等）。
async fn ensure_chain(
    store: &Store,
    user: Uuid,
    module: &str,
    path: &str,
) -> Result<(), FolderError> {
    let now = Utc::now();
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let mut acc = String::new();
    for seg in segs {
        acc = format!("{acc}/{seg}");
        execute_both!(
            store,
            "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (user_id, module_id, path) DO NOTHING",
            Uuid::new_v4(),
            user,
            module,
            &acc,
            now,
            now
        )
        .map_err(storage_err)?;
    }
    Ok(())
}

/// 新建文件夹（归一化 + UNIQUE 冲突→Conflict）。
pub async fn create_folder(
    store: &Store,
    user: Uuid,
    module: &str,
    raw_path: &str,
) -> Result<Folder, FolderError> {
    let path = path::normalize_path(raw_path)?;
    let now = Utc::now();
    let id = Uuid::new_v4();
    let res = execute_both!(
        store,
        "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        id,
        user,
        module,
        &path,
        now,
        now
    );
    match res {
        Ok(_) => Ok(Folder {
            id,
            user_id: user,
            module_id: module.to_string(),
            path,
            created_at: now,
            updated_at: now,
        }),
        Err(e) => Err(FolderError::Conflict(format!("{path}（{e}）"))),
    }
}

/// 批量改写事务：对给定 (folder_id, new_path) 与 (session_id, new_path Option) 原子落库。
async fn apply_rewrites(
    store: &Store,
    folder_updates: &[(Uuid, String)],
    session_updates: &[(Uuid, Option<String>)],
    delete_folder: Option<Uuid>,
) -> Result<(), FolderError> {
    let now = Utc::now();
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            for (id, new_path) in folder_updates {
                sqlx::query("UPDATE session_folders SET path = ?, updated_at = ? WHERE id = ?")
                    .bind(new_path)
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            for (id, new_path) in session_updates {
                sqlx::query("UPDATE sessions SET path = ?, updated_at = ? WHERE id = ?")
                    .bind(new_path)
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            if let Some(id) = delete_folder {
                sqlx::query("DELETE FROM session_folders WHERE id = ?")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            tx.commit().await.map_err(storage_err)?;
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(storage_err)?;
            for (id, new_path) in folder_updates {
                sqlx::query("UPDATE session_folders SET path = $1, updated_at = $2 WHERE id = $3")
                    .bind(new_path)
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            for (id, new_path) in session_updates {
                sqlx::query("UPDATE sessions SET path = $1, updated_at = $2 WHERE id = $3")
                    .bind(new_path)
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            if let Some(id) = delete_folder {
                sqlx::query("DELETE FROM session_folders WHERE id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            tx.commit().await.map_err(storage_err)?;
        }
    }
    Ok(())
}

/// 取用户某命名空间所有会话 (id, path)（前缀匹配 Rust 侧）。
async fn session_paths(
    store: &Store,
    user: Uuid,
    module: &str,
) -> Result<Vec<(Uuid, Option<String>)>, FolderError> {
    // module 哨兵 default ↔ sessions.module_id NULL 映射。
    let rows: Vec<(Uuid, Option<String>, Option<String>)> = fetch_all_both!(
        store,
        "SELECT s.id, s.path, s.module_id FROM sessions s WHERE s.user_id = $1",
        (Uuid, Option<String>, Option<String>),
        user
    )
    .map_err(storage_err)?;
    Ok(rows
        .into_iter()
        .filter(|(_, _, m)| m.as_deref().unwrap_or(path::DEFAULT_NAMESPACE) == module)
        .map(|(id, p, _)| (id, p))
        .collect())
}

/// 改名/移动文件夹：前缀批量改写子树（文件夹 + 会话），目标不可在源子树内（防环），单事务原子。
pub async fn rename_folder(
    store: &Store,
    user: Uuid,
    id: Uuid,
    raw_new: &str,
) -> Result<Option<Folder>, FolderError> {
    let Some(old) = get_folder(store, user, id).await.map_err(storage_err)? else {
        return Ok(None);
    };
    let new_path = path::normalize_path(raw_new)?;
    if path::is_within(&old.path, &new_path) {
        return Err(FolderError::Cycle(new_path));
    }
    let module = &old.module_id;
    // 子树内文件夹（含自身）改写。
    let folders = list_folders(store, user, module)
        .await
        .map_err(storage_err)?;
    let folder_updates: Vec<(Uuid, String)> = folders
        .iter()
        .filter(|f| path::is_within(&old.path, &f.path))
        .map(|f| (f.id, path::rewrite_prefix(&f.path, &old.path, &new_path)))
        .collect();
    // 子树内会话改写。
    let sessions = session_paths(store, user, module).await?;
    let session_updates: Vec<(Uuid, Option<String>)> = sessions
        .iter()
        .filter(|(_, p)| p.as_deref().is_some_and(|p| path::is_within(&old.path, p)))
        .map(|(sid, p)| {
            let np = path::rewrite_prefix(p.as_deref().unwrap(), &old.path, &new_path);
            (*sid, Some(np))
        })
        .collect();
    apply_rewrites(store, &folder_updates, &session_updates, None).await?;
    get_folder(store, user, id).await.map_err(storage_err)
}

/// 删文件夹：子树（文件夹 + 会话）上移一层（移除本段），绝不级联删会话；单事务原子。
pub async fn delete_folder(store: &Store, user: Uuid, id: Uuid) -> Result<bool, FolderError> {
    let Some(target) = get_folder(store, user, id).await.map_err(storage_err)? else {
        return Ok(false);
    };
    let module = &target.module_id;
    let parent = path::parent(&target.path);
    let new_base = parent.clone().unwrap_or_default();
    // 后代文件夹（不含自身）上移。
    let folders = list_folders(store, user, module)
        .await
        .map_err(storage_err)?;
    let folder_updates: Vec<(Uuid, String)> = folders
        .iter()
        .filter(|f| f.id != id && path::is_within(&target.path, &f.path))
        .map(|f| (f.id, path::rewrite_prefix(&f.path, &target.path, &new_base)))
        .collect();
    // 子树内会话上移（根级→NULL 未分类）。
    let sessions = session_paths(store, user, module).await?;
    let session_updates: Vec<(Uuid, Option<String>)> = sessions
        .iter()
        .filter(|(_, p)| {
            p.as_deref()
                .is_some_and(|p| path::is_within(&target.path, p))
        })
        .map(|(sid, p)| {
            let np = path::rewrite_prefix(p.as_deref().unwrap(), &target.path, &new_base);
            (*sid, if np.is_empty() { None } else { Some(np) })
        })
        .collect();
    apply_rewrites(store, &folder_updates, &session_updates, Some(id)).await?;
    Ok(true)
}

/// 移动会话到路径（None=未分类）；自动注册目标路径链；归属校验 + 自取会话命名空间。
pub async fn move_session(
    store: &Store,
    user: Uuid,
    session: Uuid,
    raw_path: Option<&str>,
) -> Result<bool, FolderError> {
    // 归属校验 + 取命名空间（module_id NULL → default）。
    let row: Option<(Option<String>,)> = fetch_optional_both!(
        store,
        "SELECT module_id FROM sessions WHERE id = $1 AND user_id = $2",
        (Option<String>,),
        session,
        user
    )
    .map_err(storage_err)?;
    let Some((module_id,)) = row else {
        return Ok(false);
    };
    let module = module_id.unwrap_or_else(|| path::DEFAULT_NAMESPACE.to_string());
    let normalized = match raw_path {
        Some(p) => Some(path::normalize_path(p)?),
        None => None,
    };
    if let Some(p) = &normalized {
        ensure_chain(store, user, &module, p).await?;
    }
    let now = Utc::now();
    execute_both!(
        store,
        "UPDATE sessions SET path = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
        &normalized,
        now,
        session,
        user
    )
    .map_err(storage_err)?;
    Ok(true)
}
