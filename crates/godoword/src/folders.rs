//! 会话文件夹 REST（M1d，虚拟路径）。AuthUser + 对象级归属；FolderError→400/500。
//! 模块 chroot：module 查询参数默认 default（无模块命名空间）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use kernel::Folder;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::rest::{folder_err_status, AppState};

/// 文件夹命名空间查询（默认 default）。
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FolderQuery {
    #[serde(default = "default_module")]
    pub module: String,
}

fn default_module() -> String {
    kernel::DEFAULT_NAMESPACE.to_string()
}

/// 新建文件夹请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateFolderReq {
    /// 虚拟路径（如 /日语/五十音）。
    pub path: String,
}

/// 改名/移动文件夹请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RenameFolderReq {
    pub path: String,
}

/// 文件夹列表响应。
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct FolderListResp {
    pub folders: Vec<Folder>,
}

/// 列出文件夹。
#[utoipa::path(get, path = "/api/v1/folders", tag = "folders", params(FolderQuery), responses((status = 200, body = FolderListResp), (status = 401)))]
pub async fn list_folders(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<FolderQuery>,
) -> Result<Json<FolderListResp>, StatusCode> {
    let folders = kernel::list_folders(&st.store, user.0, &q.module)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(FolderListResp { folders }))
}

/// 新建文件夹。
#[utoipa::path(post, path = "/api/v1/folders", tag = "folders", params(FolderQuery), request_body = CreateFolderReq, responses((status = 201, body = Folder), (status = 400), (status = 401)))]
pub async fn create_folder(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<FolderQuery>,
    Json(req): Json<CreateFolderReq>,
) -> Result<(StatusCode, Json<Folder>), StatusCode> {
    let folder = kernel::create_folder(&st.store, user.0, &q.module, &req.path)
        .await
        .map_err(folder_err_status)?;
    Ok((StatusCode::CREATED, Json(folder)))
}

/// 改名/移动文件夹（子树前缀批量改写）。
#[utoipa::path(put, path = "/api/v1/folders/{id}", tag = "folders", params(("id" = Uuid, Path, description = "文件夹 id")), request_body = RenameFolderReq, responses((status = 200, body = Folder), (status = 400), (status = 401), (status = 404)))]
pub async fn rename_folder(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RenameFolderReq>,
) -> Result<Json<Folder>, StatusCode> {
    kernel::rename_folder(&st.store, user.0, id, &req.path)
        .await
        .map_err(folder_err_status)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// 删文件夹（子树上移一层，绝不级联删会话）。
#[utoipa::path(delete, path = "/api/v1/folders/{id}", tag = "folders", params(("id" = Uuid, Path, description = "文件夹 id")), responses((status = 204), (status = 401), (status = 404)))]
pub async fn delete_folder(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match kernel::delete_folder(&st.store, user.0, id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(e) => Err(folder_err_status(e)),
    }
}
