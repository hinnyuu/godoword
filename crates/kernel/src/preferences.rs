//! 用户偏好仓储（M1d，跨设备随账户）。prefs_json 整取回 Rust（双兼容子集）。
//! 部署级配置（env/file）与用户级偏好正交（history 0530）；偏好仅 DB + webui。

use chrono::Utc;
use kernel_api::{execute_both, fetch_optional_both, KernelError, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// 用户偏好（typed 已知键；未知键 serde 忽略，扩展加字段免迁移——JSON 整取）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Preferences {
    /// 主题预设 id（如 light/dark/自定义）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// 音频自动播放（默认手动）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_play: Option<bool>,
    /// 会话断开行为全局默认（cancel/continue）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disconnect_default: Option<String>,
    /// 侧栏折叠态。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_collapsed: Option<bool>,
    /// 密度（compact/comfortable）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<String>,
    /// 字号（px）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u32>,
    /// 回合硬超时秒数（用户级 BYOK 成本/耐心权衡，同 on_disconnect 轴；缺省内核默认 180s，
    /// 使用时钳位 [30,3600]；sign-off 轮裁决：用户自配而非部署级）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_timeout_secs: Option<u32>,
    /// 高级自定义 CSS（scoped 注入，一键重置）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_css: Option<String>,
    /// 用户自定义主题变量（CSS 变量名→值）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_vars: Option<Value>,
}

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

/// 读用户偏好（无则默认）。
pub async fn get_preferences(store: &Store, user: Uuid) -> Result<Preferences, KernelError> {
    let row: Option<(Value,)> = fetch_optional_both!(
        store,
        "SELECT prefs_json FROM user_preferences WHERE user_id = $1",
        (Value,),
        user
    )
    .map_err(storage_err)?;
    Ok(match row {
        Some((v,)) => serde_json::from_value(v).unwrap_or_default(),
        None => Preferences::default(),
    })
}

/// 写用户偏好（UPSERT，last-write-wins）。返回写入值。
pub async fn put_preferences(
    store: &Store,
    user: Uuid,
    prefs: &Preferences,
) -> Result<Preferences, KernelError> {
    let now = Utc::now();
    let json = serde_json::to_value(prefs)
        .map_err(|e| KernelError::Internal(format!("偏好序列化失败：{e}")))?;
    execute_both!(
        store,
        "INSERT INTO user_preferences (user_id, prefs_json, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET prefs_json = $2, updated_at = $3",
        user,
        &json,
        now
    )
    .map_err(storage_err)?;
    Ok(prefs.clone())
}
