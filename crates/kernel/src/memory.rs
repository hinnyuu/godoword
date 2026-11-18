//! 内存接缝实现：M1a 生产默认（临时态，dev-debt T2）+ 永久测试替身。
//! M1b/c 换 DB/文件系统实现作生产默认；本 impl 保留供测试。

use async_trait::async_trait;
use kernel_api::{AudioId, ChatMessage, KernelError, SessionId, UserId};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::seam::{AudioCache, CachedAudio, SessionMeta, SessionRepo, StoredMessage};

struct SessionState {
    user: UserId,
    meta: SessionMeta,
    messages: Vec<StoredMessage>,
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

    async fn meta(&self, user: UserId, id: SessionId) -> Result<Option<SessionMeta>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .filter(|s| s.user == user)
            .map(|s| s.meta.clone()))
    }

    async fn exists(&self, user: UserId, id: SessionId) -> Result<bool, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| s.user == user)
            .unwrap_or(false))
    }

    async fn history(&self, user: UserId, id: SessionId) -> Result<Vec<ChatMessage>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .filter(|s| s.user == user)
            .map(|s| s.messages.iter().map(|m| m.message.clone()).collect())
            .unwrap_or_default())
    }

    async fn history_with_ids(
        &self,
        user: UserId,
        id: SessionId,
    ) -> Result<Vec<StoredMessage>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .filter(|s| s.user == user)
            .map(|s| s.messages.clone())
            .unwrap_or_default())
    }

    async fn append(
        &self,
        user: UserId,
        id: SessionId,
        msg: ChatMessage,
    ) -> Result<(), KernelError> {
        let mut g = self.inner.lock().unwrap();
        match g.get_mut(&id) {
            Some(s) if s.user == user => {
                let seq = (s.messages.len() + 1) as i32;
                s.messages.push(StoredMessage {
                    id: Uuid::new_v4(),
                    seq,
                    message: msg,
                });
                Ok(())
            }
            _ => Err(KernelError::Storage(format!("会话不存在或非本用户：{id}"))),
        }
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

    async fn delete(&self, user: UserId, id: SessionId) -> Result<(), KernelError> {
        let mut g = self.inner.lock().unwrap();
        if g.get(&id).map(|s| s.user == user).unwrap_or(false) {
            g.remove(&id);
        }
        Ok(())
    }

    async fn update_eviction(
        &self,
        user: UserId,
        id: SessionId,
        summary: Option<String>,
        evicted_up_to: i32,
    ) -> Result<(), KernelError> {
        let mut g = self.inner.lock().unwrap();
        if let Some(s) = g.get_mut(&id) {
            if s.user == user {
                s.meta.summary = summary;
                s.meta.evicted_up_to = evicted_up_to;
            }
        }
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

    async fn get(&self, id: AudioId, user: UserId) -> Result<Option<CachedAudio>, KernelError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&id)
            .filter(|a| a.owner == user)
            .map(|a| CachedAudio {
                pcm: a.pcm.clone(),
                words: a.words.clone(),
                format: a.format,
                sample_rate: a.sample_rate,
                owner: a.owner,
            }))
    }
}
