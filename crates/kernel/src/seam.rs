//! 内核内部接缝（非 kernel-api 契约）：回合循环依赖这些 trait，
//! M1a 用内存/env 实现，M1b/c 换 DB/加密实现——循环逻辑不重写（零返工设计）。

use async_trait::async_trait;
use bytes::Bytes;
use kernel_api::{
    AudioFormat, Capability, ChatMessage, KernelError, ModuleId, ProviderConn, Role, SessionId,
    SessionParams, UserId, WordTimestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 解析后的绑定：kernel 解密后的连接 + 模型 + 参数。
pub struct ResolvedBinding {
    /// 上游连接（含解密 key）。
    pub conn: ProviderConn,
    /// 模型 id。
    pub model: String,
    /// 绑定参数（如默认音色）。
    pub params: Value,
}

/// 绑定解析接缝。M1a env 实现（dev），M1b/c DB 5 级 fallback + 解密。
#[async_trait]
pub trait BindingResolver: Send + Sync {
    /// 按 (user, module, capability, role) 解析绑定；未配置返回 KernelError::Unconfigured。
    async fn resolve(
        &self,
        user: UserId,
        module: Option<&ModuleId>,
        capability: Capability,
        role: Role,
    ) -> Result<ResolvedBinding, KernelError>;
}

/// 断开行为（sessions.on_disconnect）。cancel 默认省费 / continue 服务端跑完（detach）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnDisconnect {
    /// 断开即取消活跃回合（默认）。
    #[default]
    Cancel,
    /// 断开后服务端 detach 跑完回合。
    Continue,
}

impl OnDisconnect {
    /// 解析存储字符串（未知值回退默认 cancel）。
    pub fn parse(s: &str) -> Self {
        match s {
            "continue" => Self::Continue,
            _ => Self::Cancel,
        }
    }

    /// 存储字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Continue => "continue",
        }
    }
}

/// 会话元数据（创建时写入，运行时读取：on_disconnect 决定断开行为，params 决定预算/淘汰/工具上限）。
#[derive(Debug, Clone, Default)]
pub struct SessionMeta {
    /// 教学模块 id（M1 无模块为 None）。
    pub module_id: Option<ModuleId>,
    /// 断开行为。
    pub on_disconnect: OnDisconnect,
    /// 会话运行时参数（预算/淘汰/工具上限）。
    pub params: SessionParams,
}

/// 会话仓储接缝。M1a 内存实现，M1c DB 实现。
#[async_trait]
pub trait SessionRepo: Send + Sync {
    /// 创建会话（携带元数据）。
    async fn create(
        &self,
        user: UserId,
        id: SessionId,
        meta: SessionMeta,
    ) -> Result<(), KernelError>;
    /// 读会话元数据（不存在 → None）。
    async fn meta(&self, id: SessionId) -> Result<Option<SessionMeta>, KernelError>;
    /// 会话是否存在。
    async fn exists(&self, id: SessionId) -> Result<bool, KernelError>;
    /// 读历史消息（上下文装配用）。
    async fn history(&self, id: SessionId) -> Result<Vec<ChatMessage>, KernelError>;
    /// 追加消息（持久化转录）。
    async fn append(&self, id: SessionId, msg: ChatMessage) -> Result<(), KernelError>;
    /// 列出用户会话。
    async fn list(&self, user: UserId) -> Result<Vec<SessionId>, KernelError>;
    /// 删除会话（含转录）。
    async fn delete(&self, id: SessionId) -> Result<(), KernelError>;
}

/// 缓存音频。
pub struct CachedAudio {
    /// 完整裸 PCM。
    pub pcm: Bytes,
    /// 词级时间戳。
    pub words: Vec<WordTimestamp>,
    /// 编码。
    pub format: AudioFormat,
    /// 采样率。
    pub sample_rate: u32,
}

/// 音频缓存接缝。M1a 内存实现，M1c 文件系统 `/data/cache`。
#[async_trait]
pub trait AudioCache: Send + Sync {
    /// 存入缓存音频。
    async fn put(&self, id: kernel_api::AudioId, audio: CachedAudio) -> Result<(), KernelError>;
    /// 取缓存音频（回放用）。
    async fn get(&self, id: kernel_api::AudioId) -> Result<Option<CachedAudio>, KernelError>;
}
