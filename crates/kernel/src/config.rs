//! 部署级配置加载（来源轴：env > 配置文件 > 内置默认；见 history 0530）。
//!
//! 配置文件 `/data/config.toml`（`GODOWORD_CONFIG` 可改路径），TOML + serde。env 逐项覆盖文件。
//! 用户级配置（providers/bindings/个人资料）仅 DB + webui，与本轴正交。配置文件可定义 provider 预设
//! （端点+适配器+建议模型，**不含 key**）→ 设置页一键模板，key 仍用户自填（BYOK 不破）。
//! webui 管理页只读展示生效值（脱敏），不回写 env/文件；file/env 变更需重启。

use std::path::{Path, PathBuf};

use kernel_api::KernelError;
use serde::{Deserialize, Serialize};

/// env 访问抽象（便于无全局 env 竞态的单元测试）。
pub trait EnvSource {
    /// 取 env 值（空串视为未设）。
    fn get(&self, key: &str) -> Option<String>;
}

/// 真实系统 env。
pub struct SystemEnv;

impl EnvSource for SystemEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|s| !s.is_empty())
    }
}

/// 注册模式（默认 invite，首用户例外成管理员；M1c 认证落地）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationMode {
    /// 邀请码注册。
    #[default]
    Invite,
    /// 开放注册。
    Open,
    /// 关闭注册。
    Disabled,
}

impl RegistrationMode {
    fn parse(s: &str) -> Result<Self, KernelError> {
        match s {
            "invite" => Ok(Self::Invite),
            "open" => Ok(Self::Open),
            "disabled" => Ok(Self::Disabled),
            other => Err(KernelError::Internal(format!(
                "registration_mode 须为 invite/open/disabled，实得 {other}"
            ))),
        }
    }
}

/// provider 预设（配置文件定义，不含 key）。
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct ProviderPreset {
    /// 预设标识。
    pub id: String,
    /// 适配器 id（如 llm-openai / tts-qwen-audio）。
    pub adapter_id: String,
    /// 显示名。
    pub label: String,
    /// 端点 base_url。
    pub base_url: String,
    /// 建议模型列表。
    #[serde(default)]
    pub suggested_models: Vec<String>,
    /// 能力提示（llm/tts，可选）。
    #[serde(default)]
    pub capability: Option<String>,
}

/// 部署级配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 监听地址（默认双栈 [::]:8080）。
    pub listen: String,
    /// 持久状态根目录（db/keys/cache/backups）。
    pub data_dir: PathBuf,
    /// PostgreSQL 连接串；设则用 PG，否则 SQLite（data_dir/db/godoword.sqlite）。
    pub database_url: Option<String>,
    /// 注册模式。
    pub registration_mode: RegistrationMode,
    /// provider 预设（仅配置文件）。
    pub provider_presets: Vec<ProviderPreset>,
    /// 音频缓存 TTL（秒，默认 30 天）；过期清理（缓存可再生）。
    pub cache_ttl_secs: u64,
    /// 音频缓存容量上限（字节，默认 1GiB）；超限淘汰最旧。
    pub cache_max_bytes: u64,
    /// on_disconnect=continue 的 detach 并发回合上限（默认 4）；超限降级 cancel。
    pub max_detached_turns: u32,
    /// 会话 cookie Secure 标志（默认 true；冒烟/本地 HTTP 设 false）。
    pub cookie_secure: bool,
    /// 可信反向代理 IP/CIDR 列表（审计 R9.4）：仅这些来源的 X-Forwarded-For 被采信解析客户端 IP；
    /// 空则不信任 XFF（直连语义）。防 IP 限流在反代后退化为全局限流。
    pub trusted_proxies: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "[::]:8080".to_string(),
            data_dir: PathBuf::from("/data"),
            database_url: None,
            registration_mode: RegistrationMode::default(),
            // 内置默认预设（首跑一键模板，不含 key；config.toml 可覆盖/追加）。
            // TTS base_url 为 host 根（适配器自 append DashScope 路径）；LLM 为 v1 根。
            provider_presets: vec![
                ProviderPreset {
                    id: "qwen-llm".to_string(),
                    adapter_id: "llm-openai".to_string(),
                    label: "Qwen LLM".to_string(),
                    base_url: "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1".to_string(),
                    suggested_models: vec!["qwen4-max-preview".to_string()],
                    capability: Some("llm".to_string()),
                },
                ProviderPreset {
                    id: "qwen-tts".to_string(),
                    adapter_id: "tts-qwen-audio".to_string(),
                    label: "Qwen TTS".to_string(),
                    base_url: "https://token-plan.maas.qianwenaiapi.com".to_string(),
                    suggested_models: vec!["qwen-audio-3.0-tts-plus".to_string()],
                    capability: Some("tts".to_string()),
                },
            ],
            cache_ttl_secs: 30 * 24 * 3600,
            cache_max_bytes: 1024 * 1024 * 1024,
            max_detached_turns: 4,
            cookie_secure: true,
            trusted_proxies: Vec::new(),
        }
    }
}

impl Config {
    /// 加载配置（系统 env）。
    pub fn load() -> Result<Self, KernelError> {
        Self::load_with(&SystemEnv)
    }

    /// 加载配置（注入 env 源）：默认 → 配置文件 overlay → env 逐项覆盖。
    pub fn load_with(env: &dyn EnvSource) -> Result<Self, KernelError> {
        let env_data_dir = env.get("GODOWORD_DATA_DIR");
        let base_data_dir = env_data_dir.clone().unwrap_or_else(|| "/data".to_string());
        let config_path = env
            .get("GODOWORD_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(&base_data_dir).join("config.toml"));

        let mut cfg = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| KernelError::Internal(format!("读配置文件失败：{e}")))?;
            toml::from_str::<Config>(&content)
                .map_err(|e| KernelError::Internal(format!("解析配置文件失败：{e}")))?
        } else {
            Config::default()
        };

        // env 逐项覆盖（env > file > default）。
        if let Some(d) = env_data_dir {
            cfg.data_dir = PathBuf::from(d);
        }
        if let Some(l) = env.get("GODOWORD_LISTEN") {
            cfg.listen = l;
        }
        if let Some(u) = env.get("GODOWORD_DATABASE_URL") {
            cfg.database_url = Some(u);
        }
        if let Some(r) = env.get("GODOWORD_REGISTRATION_MODE") {
            cfg.registration_mode = RegistrationMode::parse(&r)?;
        }
        if let Some(v) = env.get("GODOWORD_CACHE_TTL_SECS") {
            cfg.cache_ttl_secs = v.parse().map_err(|_| {
                KernelError::Internal(format!("GODOWORD_CACHE_TTL_SECS 须为整数，实得 {v}"))
            })?;
        }
        if let Some(v) = env.get("GODOWORD_CACHE_MAX_BYTES") {
            cfg.cache_max_bytes = v.parse().map_err(|_| {
                KernelError::Internal(format!("GODOWORD_CACHE_MAX_BYTES 须为整数，实得 {v}"))
            })?;
        }
        if let Some(v) = env.get("GODOWORD_MAX_DETACHED_TURNS") {
            cfg.max_detached_turns = v.parse().map_err(|_| {
                KernelError::Internal(format!("GODOWORD_MAX_DETACHED_TURNS 须为整数，实得 {v}"))
            })?;
        }
        if let Some(v) = env.get("GODOWORD_COOKIE_SECURE") {
            cfg.cookie_secure = matches!(v.as_str(), "true" | "1" | "yes");
        }
        if let Some(v) = env.get("GODOWORD_TRUSTED_PROXIES") {
            cfg.trusted_proxies = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        Ok(cfg)
    }

    /// SQLite 数据库文件路径（data_dir/db/godoword.sqlite）。
    pub fn sqlite_path(&self) -> PathBuf {
        self.data_dir.join("db").join("godoword.sqlite")
    }

    /// 备份目录（data_dir/backups）。
    pub fn backup_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    /// 音频缓存目录（data_dir/cache）。
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    /// 音频缓存 TTL。
    pub fn cache_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.cache_ttl_secs)
    }

    /// 脱敏只读视图（GET /config 用）：不暴露 database_url 凭据，仅后端类型。
    pub fn view(&self) -> ConfigView {
        ConfigView {
            listen: self.listen.clone(),
            data_dir: self.data_dir.to_string_lossy().into_owned(),
            database_backend: if self.database_url.is_some() {
                "postgres"
            } else {
                "sqlite"
            }
            .to_string(),
            registration_mode: self.registration_mode,
            provider_presets: self.provider_presets.clone(),
            trusted_proxies: self.trusted_proxies.clone(),
        }
    }
}

/// 配置脱敏只读视图（REST 序列化）。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ConfigView {
    pub listen: String,
    pub data_dir: String,
    pub database_backend: String,
    pub registration_mode: RegistrationMode,
    pub provider_presets: Vec<ProviderPreset>,
    pub trusted_proxies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapEnv(HashMap<String, String>);
    impl EnvSource for MapEnv {
        fn get(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned().filter(|s| !s.is_empty())
        }
    }
    fn env(pairs: &[(&str, &str)]) -> MapEnv {
        MapEnv(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn defaults() {
        let cfg = Config::load_with(&env(&[])).unwrap();
        assert_eq!(cfg.listen, "[::]:8080");
        assert_eq!(cfg.data_dir, PathBuf::from("/data"));
        assert!(cfg.database_url.is_none());
        assert_eq!(cfg.registration_mode, RegistrationMode::Invite);
        assert_eq!(cfg.provider_presets.len(), 2, "内置默认预设 qwen-llm/qwen-tts");
        assert_eq!(cfg.provider_presets[0].id, "qwen-llm");
        assert_eq!(cfg.provider_presets[1].id, "qwen-tts");
        assert_eq!(cfg.view().database_backend, "sqlite");
    }

    #[test]
    fn env_overrides() {
        let cfg = Config::load_with(&env(&[
            ("GODOWORD_LISTEN", "0.0.0.0:9090"),
            ("GODOWORD_DATA_DIR", "/custom/data"),
            ("GODOWORD_DATABASE_URL", "postgres://u:p@host/db"),
            ("GODOWORD_REGISTRATION_MODE", "open"),
        ]))
        .unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:9090");
        assert_eq!(cfg.data_dir, PathBuf::from("/custom/data"));
        assert_eq!(cfg.registration_mode, RegistrationMode::Open);
        assert_eq!(cfg.view().database_backend, "postgres");
        // 视图脱敏：不含 database_url 凭据。
        let view_json = serde_json::to_string(&cfg.view()).unwrap();
        assert!(!view_json.contains("postgres://"), "视图不暴露连接串");
        assert!(!view_json.contains("u:p@"), "视图不暴露凭据");
    }

    #[test]
    fn bad_registration_mode_fails() {
        assert!(Config::load_with(&env(&[("GODOWORD_REGISTRATION_MODE", "bogus")])).is_err());
    }

    #[test]
    fn trusted_proxies_env_parse() {
        let cfg = Config::load_with(&env(&[(
            "GODOWORD_TRUSTED_PROXIES",
            "10.0.0.1, 10.0.0.0/8 , ",
        )]))
        .unwrap();
        assert_eq!(
            cfg.trusted_proxies,
            vec!["10.0.0.1".to_string(), "10.0.0.0/8".to_string()],
            "R9.4：逗号分隔 trim 去空"
        );
        assert_eq!(cfg.view().trusted_proxies.len(), 2, "R9.4：视图暴露");
        assert!(
            Config::load_with(&env(&[]))
                .unwrap()
                .trusted_proxies
                .is_empty(),
            "R9.4：默认空（不信任 XFF）"
        );
    }

    #[test]
    fn toml_file_and_presets() {
        let dir = std::env::temp_dir().join(format!("gw-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            r#"
listen = "127.0.0.1:7070"
registration_mode = "disabled"

[[provider_presets]]
id = "qwen-llm"
adapter_id = "llm-openai"
label = "Qwen LLM"
base_url = "https://example.com/compatible-mode/v1"
suggested_models = ["qwen4-max-preview"]
capability = "llm"

[[provider_presets]]
id = "qwen-tts"
adapter_id = "tts-qwen-audio"
label = "Qwen TTS"
base_url = "https://example.com"
"#,
        )
        .unwrap();
        let cfg = Config::load_with(&env(&[("GODOWORD_CONFIG", path.to_str().unwrap())])).unwrap();
        assert_eq!(cfg.listen, "127.0.0.1:7070");
        assert_eq!(cfg.registration_mode, RegistrationMode::Disabled);
        assert_eq!(cfg.provider_presets.len(), 2);
        assert_eq!(cfg.provider_presets[0].id, "qwen-llm");
        assert_eq!(
            cfg.provider_presets[0].suggested_models,
            vec!["qwen4-max-preview"]
        );
        assert!(cfg.provider_presets[1].suggested_models.is_empty());
        // 预设不含 key。
        let json = serde_json::to_string(&cfg.provider_presets).unwrap();
        assert!(!json.contains("key"), "预设不含 key");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn env_overrides_file() {
        let dir = std::env::temp_dir().join(format!("gw-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "listen = \"127.0.0.1:7070\"\n").unwrap();
        let cfg = Config::load_with(&env(&[
            ("GODOWORD_CONFIG", path.to_str().unwrap()),
            ("GODOWORD_LISTEN", "0.0.0.0:8080"),
        ]))
        .unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:8080", "env > file");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
