//! 会话文件夹仓储（M1d，虚拟路径即身份）。登记表持久空文件夹；模块 chroot 命名空间。
//! 改名/移动=前缀批量改写（Rust 侧匹配 + 单事务原子）；删非空文件夹→子树上移一层（绝不级联删会话）。
//! 读经薄分发宏（pool），写收口驱动专属事务（原子）；私有部署单用户自组织，写事务保完整性。
//! 并发写 TOCTOU（读改写间隙）fail-closed：UNIQUE/条件冲突 → 事务回滚（无损坏，500 语义），不丢数据。
//! 审计修复：祖先链登记与改写同事务（A 孤儿路径）；改名冲突预检（B）；改写后上限重校验（D）；
//! 删除上移冲突预检 + 目标先删释放路径（F1，与改名对称）。

use chrono::{DateTime, Utc};
use kernel_api::{fetch_all_both, fetch_optional_both, KernelError, Store};
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

/// 路径的祖先链前缀（`/a/b/c` → `["/a","/a/b","/a/b/c"]`）；include_self=false 去掉末段。
fn chain(path: &str, include_self: bool) -> Vec<String> {
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let n = if include_self {
        segs.len()
    } else {
        segs.len().saturating_sub(1)
    };
    let mut acc = String::new();
    let mut out = Vec::new();
    for seg in segs.iter().take(n) {
        acc = format!("{acc}/{seg}");
        out.push(acc.clone());
    }
    out
}

/// 唯一约束冲突识别（两后端；create/改名的 Conflict 语义，区别真实存储故障）。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .map(|d| d.is_unique_violation())
        .unwrap_or(false)
}

/// 文件夹写事务（原子）：祖先链登记（DO NOTHING）+ 可选自身插入（冲突即 Conflict）
/// + 文件夹/会话路径批量改写 + 可选删除。审计 A：链登记与改写同事务，失败整体回滚不留孤儿。
#[allow(clippy::too_many_arguments)]
async fn write_tx(
    store: &Store,
    user: Uuid,
    module: &str,
    chain_paths: &[String],
    insert_self: Option<(Uuid, &str)>,
    folder_updates: &[(Uuid, String)],
    session_updates: &[(Uuid, Option<String>)],
    delete_folder: Option<Uuid>,
) -> Result<(), FolderError> {
    let now = Utc::now();
    macro_rules! tx_body {
        ($pool:expr, $ph:literal) => {{
            let mut tx = $pool.begin().await.map_err(storage_err)?;
            for p in chain_paths {
                sqlx::query(concat!(
                    "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at) ",
                    "VALUES (", $ph, "1, ", $ph, "2, ", $ph, "3, ", $ph, "4, ", $ph, "5, ", $ph, "6) ",
                    "ON CONFLICT (user_id, module_id, path) DO NOTHING"
                ))
                .bind(Uuid::new_v4())
                .bind(user)
                .bind(module)
                .bind(p)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(storage_err)?;
            }
            if let Some((self_id, p)) = insert_self {
                let r = sqlx::query(concat!(
                    "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at) ",
                    "VALUES (", $ph, "1, ", $ph, "2, ", $ph, "3, ", $ph, "4, ", $ph, "5, ", $ph, "6)"
                ))
                .bind(self_id)
                .bind(user)
                .bind(module)
                .bind(p)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await;
                if let Err(e) = r {
                    if is_unique_violation(&e) {
                        return Err(FolderError::Conflict(p.to_string()));
                    }
                    return Err(storage_err(e));
                }
            }
            // 删除先于改写（审计 F1）：目标路径先释放，子文件夹上移可复用目标路径（/a/a → /a）。
            if let Some(id) = delete_folder {
                sqlx::query(concat!("DELETE FROM session_folders WHERE id = ", $ph, "1"))
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_err)?;
            }
            for (id, new_path) in folder_updates {
                sqlx::query(concat!(
                    "UPDATE session_folders SET path = ", $ph, "1, updated_at = ", $ph, "2 WHERE id = ", $ph, "3"
                ))
                .bind(new_path)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage_err)?;
            }
            for (id, new_path) in session_updates {
                sqlx::query(concat!(
                    "UPDATE sessions SET path = ", $ph, "1, updated_at = ", $ph, "2 WHERE id = ", $ph, "3"
                ))
                .bind(new_path)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage_err)?;
            }
            tx.commit().await.map_err(storage_err)?;
        }};
    }
    match store {
        Store::Sqlite(pool) => tx_body!(pool, "?"),
        Store::Pg(pool) => tx_body!(pool, "$"),
    }
    Ok(())
}

/// 新建文件夹（归一化 + 祖先链同事务登记 + UNIQUE 冲突→Conflict）。
pub async fn create_folder(
    store: &Store,
    user: Uuid,
    module: &str,
    raw_path: &str,
) -> Result<Folder, FolderError> {
    let path = path::normalize_path(raw_path)?;
    let id = Uuid::new_v4();
    write_tx(
        store,
        user,
        module,
        &chain(&path, false),
        Some((id, &path)),
        &[],
        &[],
        None,
    )
    .await?;
    // 二审 R2-2：返回落库行（时间戳以事务内为准，不返回构造近似值）。
    get_folder(store, user, id)
        .await
        .map_err(storage_err)?
        .ok_or_else(|| FolderError::Storage(format!("新建文件夹读回失败：{id}")))
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

/// 改写后路径上限重校验（审计 D：上限是稳定性硬边界，派生路径亦不得越界；段内容不变故段上限天然保持）。
fn check_caps(paths: impl Iterator<Item = String>) -> Result<(), FolderError> {
    for p in paths {
        if path::depth(&p) > path::MAX_DEPTH {
            return Err(FolderError::Invalid(PathError::Invalid(format!(
                "改写后嵌套深度超上限 {}：{p}",
                path::MAX_DEPTH
            ))));
        }
        if p.chars().count() > path::MAX_PATH_CHARS {
            return Err(FolderError::Invalid(PathError::Invalid(format!(
                "改写后全路径超上限 {} 字符",
                path::MAX_PATH_CHARS
            ))));
        }
    }
    Ok(())
}

/// 改名/移动文件夹：前缀批量改写子树（文件夹 + 会话），目标不可在源子树内（防环），
/// 目标/派生路径不得与子树外既有文件夹冲突（审计 B），改写后上限重校验（审计 D），
/// 目标祖先链同事务登记（审计 A），单事务原子。
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
    if new_path == old.path {
        return Ok(Some(old));
    }
    if path::is_within(&old.path, &new_path) {
        return Err(FolderError::Cycle(new_path));
    }
    let module = &old.module_id;
    let folders = list_folders(store, user, module)
        .await
        .map_err(storage_err)?;
    // 子树外既有路径集（冲突预检基准）。
    let outside: std::collections::HashSet<&str> = folders
        .iter()
        .filter(|f| !path::is_within(&old.path, &f.path))
        .map(|f| f.path.as_str())
        .collect();
    let folder_updates: Vec<(Uuid, String)> = folders
        .iter()
        .filter(|f| path::is_within(&old.path, &f.path))
        .map(|f| (f.id, path::rewrite_prefix(&f.path, &old.path, &new_path)))
        .collect();
    if let Some((_, p)) = folder_updates
        .iter()
        .find(|(_, p)| outside.contains(p.as_str()))
    {
        return Err(FolderError::Conflict(p.clone()));
    }
    let sessions = session_paths(store, user, module).await?;
    let session_updates: Vec<(Uuid, Option<String>)> = sessions
        .iter()
        .filter(|(_, p)| p.as_deref().is_some_and(|p| path::is_within(&old.path, p)))
        .map(|(sid, p)| {
            let np = path::rewrite_prefix(p.as_deref().unwrap(), &old.path, &new_path);
            (*sid, Some(np))
        })
        .collect();
    check_caps(
        folder_updates
            .iter()
            .map(|(_, p)| p.clone())
            .chain(session_updates.iter().filter_map(|(_, p)| p.clone())),
    )?;
    write_tx(
        store,
        user,
        module,
        &chain(&new_path, false),
        None,
        &folder_updates,
        &session_updates,
        None,
    )
    .await?;
    get_folder(store, user, id).await.map_err(storage_err)
}

/// 删文件夹：子树（文件夹 + 会话）上移一层（移除本段），绝不级联删会话；
/// 父链同事务登记（审计 A：上移目标祖先必在登记表，前端树可见）；单事务原子。
/// **冲突预检（审计 F1，与 rename 对称）**：上移后路径不得与子树外既有文件夹冲突 → Conflict(400)；
/// 目标自身路径除外（同事务先删目标，子文件夹可上移复用其路径，如 /a/a → /a）。
pub async fn delete_folder(store: &Store, user: Uuid, id: Uuid) -> Result<bool, FolderError> {
    let Some(target) = get_folder(store, user, id).await.map_err(storage_err)? else {
        return Ok(false);
    };
    let module = &target.module_id;
    let parent = path::parent(&target.path);
    let new_base = parent.clone().unwrap_or_default();
    let folders = list_folders(store, user, module)
        .await
        .map_err(storage_err)?;
    let folder_updates: Vec<(Uuid, String)> = folders
        .iter()
        .filter(|f| f.id != id && path::is_within(&target.path, &f.path))
        .map(|f| (f.id, path::rewrite_prefix(&f.path, &target.path, &new_base)))
        .collect();
    // 子树外既有路径集（冲突预检基准；目标自身即将删除，其路径视为释放）。
    let outside: std::collections::HashSet<&str> = folders
        .iter()
        .filter(|f| f.id != id && !path::is_within(&target.path, &f.path))
        .map(|f| f.path.as_str())
        .collect();
    if let Some((_, p)) = folder_updates
        .iter()
        .find(|(_, p)| outside.contains(p.as_str()))
    {
        return Err(FolderError::Conflict(p.clone()));
    }
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
    // 上移不增深度（删一段），上限天然保持；父链登记保证可见性。
    let parent_chain = parent.map(|p| chain(&p, true)).unwrap_or_default();
    write_tx(
        store,
        user,
        module,
        &parent_chain,
        None,
        &folder_updates,
        &session_updates,
        Some(id),
    )
    .await?;
    Ok(true)
}

/// 移动会话到路径（None=未分类）；目标链同事务登记；归属校验 + 自取会话命名空间。
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
    let now = Utc::now();
    let target_chain = normalized
        .as_deref()
        .map(|p| chain(p, true))
        .unwrap_or_default();
    // 链登记 + 移动同事务（审计 A：失败整体回滚，不留半登记）。
    macro_rules! move_tx {
        ($pool:expr, $ph:literal) => {{
            let mut tx = $pool.begin().await.map_err(storage_err)?;
            for p in &target_chain {
                sqlx::query(concat!(
                    "INSERT INTO session_folders (id, user_id, module_id, path, created_at, updated_at) ",
                    "VALUES (", $ph, "1, ", $ph, "2, ", $ph, "3, ", $ph, "4, ", $ph, "5, ", $ph, "6) ",
                    "ON CONFLICT (user_id, module_id, path) DO NOTHING"
                ))
                .bind(Uuid::new_v4())
                .bind(user)
                .bind(&module)
                .bind(p)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(storage_err)?;
            }
            let n = sqlx::query(concat!(
                "UPDATE sessions SET path = ", $ph, "1, updated_at = ", $ph, "2 ",
                "WHERE id = ", $ph, "3 AND user_id = ", $ph, "4"
            ))
            .bind(&normalized)
            .bind(now)
            .bind(session)
            .bind(user)
            .execute(&mut *tx)
            .await
            .map_err(storage_err)?
            .rows_affected();
            tx.commit().await.map_err(storage_err)?;
            // 审计 O8：预检后并发删除 → rows=0，不假成功。
            if n == 0 {
                return Ok(false);
            }
        }};
    }
    match store {
        Store::Sqlite(pool) => move_tx!(pool, "?"),
        Store::Pg(pool) => move_tx!(pool, "$"),
    }
    Ok(true)
}
