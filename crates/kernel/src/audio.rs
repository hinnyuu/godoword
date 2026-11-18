//! 文件系统音频缓存（M1c 生产路径，回放存储；替换 M1a 内存默认）。
//!
//! `/data/cache/<id>.pcm`（完整裸 PCM）+ `<id>.json` sidecar（words/format/sample_rate/created_at）。
//! 默认新鲜合成（随机 AudioId，每次合成存供 `GET /audio/{id}` 回放）；复用由模块显式白名单声明（M2）。
//! 缓存可再生可丢弃：写 tmp+rename 原子防半读；TTL + 容量上限有界（防 PCM 撑满磁盘），周期清理淘汰最旧。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use kernel_api::{AudioFormat, AudioId, KernelError, UserId, WordTimestamp};
use serde::{Deserialize, Serialize};

use crate::seam::{AudioCache, CachedAudio};

/// 文件系统音频缓存（缓存目录注入）。
pub struct FsAudio {
    dir: PathBuf,
}

/// sidecar 元数据（词级时间戳 + 编码 + 采样率 + 创建时间 + 归属用户）。
#[derive(Serialize, Deserialize)]
struct Sidecar {
    words: Vec<WordTimestamp>,
    format: AudioFormat,
    sample_rate: u32,
    created_at: DateTime<Utc>,
    owner: UserId,
}

impl FsAudio {
    /// 新建缓存（目录如 `/data/cache`）。
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn pcm_path(&self, id: AudioId) -> PathBuf {
        self.dir.join(format!("{}.pcm", id.0))
    }

    fn json_path(&self, id: AudioId) -> PathBuf {
        self.dir.join(format!("{}.json", id.0))
    }

    /// 清理过期 + 超容量缓存（TTL 按文件 mtime；超容量按 mtime 淘汰最旧）。返回删除的音频数。
    /// 缓存可再生，淘汰只损失旧音频回放可用性（需要时重新合成），不损失正确性。
    pub fn cleanup(&self, ttl: Duration, max_bytes: u64) -> std::io::Result<usize> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        // 收集 .pcm 文件（mtime, size, path）。
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("pcm") {
                continue;
            }
            let meta = entry.metadata()?;
            files.push((meta.modified()?, meta.len(), path));
        }
        let now = SystemTime::now();
        let mut removed = 0;
        // TTL 淘汰。
        files.retain(|(mtime, _, path)| {
            let expired = now.duration_since(*mtime).map(|d| d > ttl).unwrap_or(false);
            if expired {
                let _ = remove_pair(path);
                removed += 1;
                false
            } else {
                true
            }
        });
        // 容量上限：按 mtime 升序淘汰最旧至总大小达标。
        let total: u64 = files.iter().map(|(_, sz, _)| sz).sum();
        if total > max_bytes {
            files.sort_by_key(|(mtime, _, _)| *mtime);
            let mut acc = total;
            for (mtime, sz, path) in files {
                if acc <= max_bytes {
                    break;
                }
                let _ = (mtime, remove_pair(&path));
                acc = acc.saturating_sub(sz);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// 删除 .pcm 及其 .json sidecar。
fn remove_pair(pcm_path: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_file(pcm_path);
    let json = pcm_path.with_extension("json");
    let _ = std::fs::remove_file(json);
    Ok(())
}

fn io_err(e: std::io::Error) -> KernelError {
    KernelError::Storage(format!("音频缓存 IO 错误：{e}"))
}

/// 原子写：写唯一 tmp 文件 + rename（防并发半读）。
async fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(
        "{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("audio"),
        uuid::Uuid::new_v4()
    ));
    tokio::fs::write(&tmp, data).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

#[async_trait]
impl AudioCache for FsAudio {
    async fn put(&self, id: AudioId, audio: CachedAudio) -> Result<(), KernelError> {
        tokio::fs::create_dir_all(&self.dir).await.map_err(io_err)?;
        let sidecar = Sidecar {
            words: audio.words,
            format: audio.format,
            sample_rate: audio.sample_rate,
            created_at: Utc::now(),
            owner: audio.owner,
        };
        let json = serde_json::to_vec(&sidecar)
            .map_err(|e| KernelError::Internal(format!("音频 sidecar 序列化失败：{e}")))?;
        write_atomic(&self.pcm_path(id), &audio.pcm)
            .await
            .map_err(io_err)?;
        if let Err(e) = write_atomic(&self.json_path(id), &json).await {
            // 半写清理孤儿 PCM（审计 A3）：sidecar 失败时移除已写 PCM，避免 get 读缺 sidecar 致 500。
            let _ = tokio::fs::remove_file(self.pcm_path(id)).await;
            return Err(io_err(e));
        }
        Ok(())
    }

    async fn get(&self, id: AudioId, user: UserId) -> Result<Option<CachedAudio>, KernelError> {
        let pcm = match tokio::fs::read(self.pcm_path(id)).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_err(e)),
        };
        let json = tokio::fs::read(self.json_path(id)).await.map_err(io_err)?;
        let sidecar: Sidecar = serde_json::from_slice(&json)
            .map_err(|e| KernelError::Internal(format!("音频 sidecar 解析失败：{e}")))?;
        // 归属校验（审计 C1）：非本用户 → None，不泄露存在性。
        if sidecar.owner != user {
            return Ok(None);
        }
        Ok(Some(CachedAudio {
            pcm: Bytes::from(pcm),
            words: sidecar.words,
            format: sidecar.format,
            sample_rate: sidecar.sample_rate,
            owner: sidecar.owner,
        }))
    }
}
