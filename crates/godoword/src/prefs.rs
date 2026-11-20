//! 用户偏好 REST（M1d，跨设备随账户）。AuthUser；prefs 仅 DB（history 0530 用户级配置）。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use kernel::Preferences;
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::rest::AppState;

/// 读用户偏好（无则默认）。
#[utoipa::path(get, path = "/api/v1/preferences", tag = "preferences", responses((status = 200, body = Preferences), (status = 401)))]
pub async fn get_preferences(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Preferences>, StatusCode> {
    let prefs = kernel::get_preferences(&st.store, user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(prefs))
}

/// 写用户偏好（last-write-wins）。
#[utoipa::path(put, path = "/api/v1/preferences", tag = "preferences", request_body = Preferences, responses((status = 200, body = Preferences), (status = 401)))]
pub async fn put_preferences(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(prefs): Json<Preferences>,
) -> Result<Json<Preferences>, StatusCode> {
    let saved = kernel::put_preferences(&st.store, user.0, &prefs)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthUser;
    use crate::testutil::{state, user_with_token, TtsMode};

    #[tokio::test]
    async fn prefs_roundtrip_and_user_isolation() {
        let st = state(TtsMode::Ok).await;
        let (u1, _) = user_with_token(&st, "pf1").await;
        let (u2, _) = user_with_token(&st, "pf2").await;
        let p = Preferences {
            theme: Some("dark".to_string()),
            auto_play: Some(true),
            ..Default::default()
        };
        let _ = put_preferences(State(st.clone()), AuthUser(u1), Json(p.clone()))
            .await
            .unwrap();
        let got = get_preferences(State(st.clone()), AuthUser(u1))
            .await
            .unwrap();
        assert_eq!(got.theme.as_deref(), Some("dark"), "偏好 round-trip");
        let other = get_preferences(State(st.clone()), AuthUser(u2))
            .await
            .unwrap();
        assert!(other.theme.is_none(), "用户隔离：u2 默认空");
    }
}
