//! 持久资产区（M2 P1，UX-A3 用户侧标准音频）：`/data/assets` **豁免 TTL/容量淘汰**
//!（「一次生成永久使用」与可丢弃缓存教义冲突的解法，与 `/data/cache` 回放缓存正交）。
//! 登记表 audio_assets（0006）：作用域 user+module+稳定 key+provider 身份（adapter_id+base_url）+voice+model。
//! 卷损毁→文件缺失视为未命中→按需懒再生兜底。写 tmp+rename 原子防半读。

use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use kernel_api::{fetch_optional_both, KernelError, Store};
use serde_json::Value;
use uuid::Uuid;

/// 资产登记行（命中直供用）。
pub struct AssetRow {
    /// 资产音频文件标识。
    pub audio_id: Uuid,
    /// 词级时间戳 JSON。
    pub words: Option<Value>,
    /// 编码（lowercase）。
    pub format: String,
    /// 采样率。
    pub sample_rate: i32,
}

/// 持久资产存储（目录 + 登记表）。
pub struct AssetStore {
    dir: PathBuf,
    store: Arc<Store>,
}

fn storage_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Storage(e.to_string())
}

impl AssetStore {
    /// 新建（目录如 `/data/assets`）。
    pub fn new(dir: PathBuf, store: Arc<Store>) -> Self {
        Self { dir, store }
    }

    /// 按完整作用域查登记行。
    #[allow(clippy::too_many_arguments)]
    pub async fn lookup(
        &self,
        user: Uuid,
        module: &str,
        key: &str,
        adapter_id: &str,
        base_url: &str,
        voice: &str,
        model: &str,
    ) -> Result<Option<AssetRow>, KernelError> {
        let row: Option<(Uuid, Option<Value>, String, i32)> = fetch_optional_both!(
            &*self.store,
            "SELECT audio_id, words, format, sample_rate FROM audio_assets
             WHERE user_id = $1 AND module_id = $2 AND stable_key = $3 AND adapter_id = $4
               AND base_url = $5 AND voice = $6 AND model = $7",
            (Uuid, Option<Value>, String, i32),
            user,
            module,
            key,
            adapter_id,
            base_url,
            voice,
            model
        )
        .map_err(storage_err)?;
        Ok(row.map(|(audio_id, words, format, sample_rate)| AssetRow {
            audio_id,
            words,
            format,
            sample_rate,
        }))
    }

    /// 读资产 PCM（缺失/卷损毁 → None，调用方懒再生）。
    pub async fn read_pcm(&self, audio_id: Uuid) -> Result<Option<Bytes>, KernelError> {
        let path = self.dir.join(format!("{audio_id}.pcm"));
        match tokio::fs::read(&path).await {
            Ok(b) => Ok(Some(Bytes::from(b))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(storage_err(format!("资产读取失败：{e}"))),
        }
    }

    /// 原子写资产 PCM（tmp+rename）。
    pub async fn write_pcm(&self, audio_id: Uuid, pcm: &[u8]) -> Result<(), KernelError> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(storage_err)?;
        let path = self.dir.join(format!("{audio_id}.pcm"));
        let tmp = self.dir.join(format!("{audio_id}.tmp-{}", Uuid::new_v4()));
        tokio::fs::write(&tmp, pcm).await.map_err(storage_err)?;
        tokio::fs::rename(&tmp, &path).await.map_err(storage_err)?;
        Ok(())
    }

    /// 登记 upsert（并发合成 last-write-wins）；RETURNING 胜出 audio_id（败者清理孤儿文件用）。
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(
        &self,
        user: Uuid,
        module: &str,
        key: &str,
        adapter_id: &str,
        base_url: &str,
        voice: &str,
        model: &str,
        audio_id: Uuid,
        format: &str,
        sample_rate: i32,
        words: &Value,
    ) -> Result<Uuid, KernelError> {
        let now = Utc::now();
        let row: (Uuid,) = kernel_api::fetch_one_both!(
            &*self.store,
            "INSERT INTO audio_assets (id, user_id, module_id, stable_key, adapter_id, base_url, voice, model,
                audio_id, format, sample_rate, words, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
             ON CONFLICT (user_id, module_id, stable_key, adapter_id, base_url, voice, model)
             DO UPDATE SET audio_id = $9, format = $10, sample_rate = $11, words = $12, created_at = $13
             RETURNING audio_id",
            (Uuid,),
            Uuid::new_v4(), user, module, key, adapter_id, base_url, voice, model,
            audio_id, format, sample_rate, words, now
        )
        .map_err(storage_err)?;
        Ok(row.0)
    }

    /// 删孤儿文件（并发合成败者；best-effort）。
    pub async fn remove_file(&self, audio_id: Uuid) {
        let path = self.dir.join(format!("{audio_id}.pcm"));
        let _ = tokio::fs::remove_file(path).await;
    }
}
