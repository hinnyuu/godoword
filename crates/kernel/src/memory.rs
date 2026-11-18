//! 内存接缝实现：M1a 生产默认（临时态，dev-debt T2）+ 永久测试替身。
//! M1b/c 换 DB/文件系统实现作生产默认；本 impl 保留供测试。

use async_trait::async_trait;
use kernel_api::{AudioId, ChatMessage, KernelError, SessionId, UserId};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::seam::{AudioCache, CachedAudio, SessionMeta, SessionRepo};

struct SessionState {
    user: UserId,
    meta: SessionMeta,
    messages: Vec<ChatMessage>,
}

/// 内存会话仓储。
#[derive(Default)]
pub struct MemSessions {
    inner: Mutex<HashMap<SessionId, SessionState>>,
}

impl MemSessions {
    /// 新建空仓储。
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionRepo for MemSessions {
    async fn create(
        &self,
        user: UserId,
        id: SessionId,
        meta: SessionMeta,
    ) -> Result<(), KernelError> {
        let mut g = self.inner.lock().unwrap();
        g.entry(id).or_insert(SessionState {
            user,
            meta,
            messages: Vec::new(),
        });
        Ok(())
    }

    async fn meta(&self, id: SessionId) -> Result<Option<SessionMeta>, KernelError> {
        Ok(self.inner.lock().unwrap().get(&id).map(|s| s.meta.clone()))
    }

    async fn exists(&self, id: SessionId) -> Result<bool, KernelError> {
        Ok(self.inner.lock().unwrap().contains_key(&id))
    }

    async fn history(&self, id: SessionId) -> Result<Vec<ChatMessage>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| s.messages.clone())
            .unwrap_or_default())
    }

    async fn append(&self, id: SessionId, msg: ChatMessage) -> Result<(), KernelError> {
        let mut g = self.inner.lock().unwrap();
        if let Some(s) = g.get_mut(&id) {
            s.messages.push(msg);
        }
        Ok(())
    }

    async fn list(&self, user: UserId) -> Result<Vec<SessionId>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.user == user)
            .map(|(id, _)| *id)
            .collect())
    }

    async fn delete(&self, id: SessionId) -> Result<(), KernelError> {
        self.inner.lock().unwrap().remove(&id);
        Ok(())
    }
}

/// 内存音频缓存。
#[derive(Default)]
pub struct MemAudio {
    inner: Mutex<HashMap<AudioId, CachedAudio>>,
}

impl MemAudio {
    /// 新建空缓存。
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AudioCache for MemAudio {
    async fn put(&self, id: AudioId, audio: CachedAudio) -> Result<(), KernelError> {
        self.inner.lock().unwrap().insert(id, audio);
        Ok(())
    }

    async fn get(&self, id: AudioId) -> Result<Option<CachedAudio>, KernelError> {
        Ok(self.inner.lock().unwrap().get(&id).map(|a| CachedAudio {
            pcm: a.pcm.clone(),
            words: a.words.clone(),
            format: a.format,
            sample_rate: a.sample_rate,
        }))
    }
}
