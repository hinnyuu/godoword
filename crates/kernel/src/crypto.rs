//! 主密钥静态加密（AES-256-GCM，每记录独立 nonce）。
//!
//! 威胁模型：防静态泄露（DB 备份/卷快照外流），不防运行中服务器被攻破（见 history 0500）。
//! 主密钥：env `GODOWORD_MASTER_KEY`（hex 64 字符）覆盖，否则自动生成 `/data/keys/master.key`
//! （raw 32 字节，0600）。密文 blob 布局：`nonce(12) + 密文 + tag(16)`（tag 由 aes-gcm 附于密文末）。
//! 丢卷=密钥永久不可解（备份卷=备份一切含密钥，文档须明示）。

use std::io::Write;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use kernel_api::KernelError;
use rand::RngCore;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const ENV_MASTER_KEY: &str = "GODOWORD_MASTER_KEY";

/// 主密钥（32 字节）。Debug 脱敏，绝不入日志。
pub struct MasterKey([u8; KEY_LEN]);

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey([REDACTED])")
    }
}

impl MasterKey {
    /// 加载主密钥：env hex 覆盖优先，否则读 `/data/keys/master.key`，缺失则自动生成（0600）。
    pub fn load(data_dir: &Path) -> Result<Self, KernelError> {
        if let Ok(hex) = std::env::var(ENV_MASTER_KEY) {
            if !hex.is_empty() {
                return Self::parse_env_hex(&hex);
            }
        }
        let key_path = data_dir.join("keys").join("master.key");
        match std::fs::read(&key_path) {
            Ok(bytes) => Self::from_raw_bytes(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut bytes = [0u8; KEY_LEN];
                rand::rngs::OsRng.fill_bytes(&mut bytes);
                write_key_file(&key_path, &bytes)?;
                tracing::info!("已自动生成主密钥 → {}", key_path.display());
                Ok(Self(bytes))
            }
            Err(e) => Err(KernelError::Internal(format!("读主密钥失败：{e}"))),
        }
    }

    /// 从 env hex（64 字符）解析主密钥；长度/格式错误 fail-fast。
    fn parse_env_hex(hex: &str) -> Result<Self, KernelError> {
        let bytes = decode_hex(hex).map_err(|e| {
            KernelError::Internal(format!("{ENV_MASTER_KEY} 须为 64 位 hex（32 字节）：{e}"))
        })?;
        Self::from_raw_bytes(&bytes)
    }

    fn from_raw_bytes(bytes: &[u8]) -> Result<Self, KernelError> {
        if bytes.len() != KEY_LEN {
            return Err(KernelError::Internal(format!(
                "主密钥须为 {KEY_LEN} 字节，实得 {} 字节",
                bytes.len()
            )));
        }
        let mut arr = [0u8; KEY_LEN];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    /// 加密：随机 12 字节 nonce + AES-256-GCM，返回 `nonce + 密文 + tag`。
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, KernelError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.0));
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
            .map_err(|_| KernelError::Internal("AES-256-GCM 加密失败".to_string()))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// 解密：拆 nonce + 密文/tag；密钥错误或密文被篡改 → 错误（GCM 认证失败）。
    pub fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>, KernelError> {
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err(KernelError::Internal(
                "密文过短（缺 nonce/tag）".to_string(),
            ));
        }
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.0));
        cipher
            .decrypt(Nonce::from_slice(&blob[..NONCE_LEN]), &blob[NONCE_LEN..])
            .map_err(|_| KernelError::Internal("解密失败（主密钥错误或密文被篡改）".to_string()))
    }
}

/// 写主密钥文件：create_new + 0600（unix 原子建_mode，避免世界可读窗口）。
fn write_key_file(path: &Path, bytes: &[u8; KEY_LEN]) -> Result<(), KernelError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| KernelError::Internal(format!("建 keys 目录失败：{e}")))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(path)
        .map_err(|e| KernelError::Internal(format!("建主密钥文件失败：{e}")))?;
    file.write_all(bytes)
        .map_err(|e| KernelError::Internal(format!("写主密钥失败：{e}")))?;
    Ok(())
}

/// hex 解码（须 64 字符 → 32 字节）。
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() != KEY_LEN * 2 {
        return Err(format!("长度 {} 非 64", s.len()));
    }
    (0..KEY_LEN)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> MasterKey {
        MasterKey([7u8; KEY_LEN])
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let k = key();
        let secret = b"sk-my-qwen-api-key-1234567890";
        let blob = k.encrypt(secret).unwrap();
        assert_eq!(blob.len(), NONCE_LEN + secret.len() + TAG_LEN);
        assert_eq!(k.decrypt(&blob).unwrap(), secret);
    }

    #[test]
    fn nonce_unique_per_record() {
        let k = key();
        let a = k.encrypt(b"same").unwrap();
        let b = k.encrypt(b"same").unwrap();
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN], "每记录独立 nonce");
        assert_ne!(a, b, "同明文密文不同");
    }

    #[test]
    fn tamper_detected() {
        let k = key();
        let mut blob = k.encrypt(b"secret").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(k.decrypt(&blob).is_err(), "篡改 tag 须认证失败");
    }

    #[test]
    fn wrong_key_fails() {
        let blob = key().encrypt(b"secret").unwrap();
        let other = MasterKey([9u8; KEY_LEN]);
        assert!(other.decrypt(&blob).is_err(), "错误主密钥须解密失败");
    }

    #[test]
    fn short_blob_fails() {
        assert!(key().decrypt(&[0u8; 8]).is_err());
    }

    #[test]
    fn debug_redacted() {
        assert_eq!(format!("{:?}", key()), "MasterKey([REDACTED])");
    }

    #[test]
    fn env_hex_valid() {
        let hex = "07".repeat(KEY_LEN);
        let k = MasterKey::parse_env_hex(&hex).unwrap();
        assert_eq!(k.0, [7u8; KEY_LEN]);
    }

    #[test]
    fn env_hex_bad_length() {
        assert!(MasterKey::parse_env_hex("0707").is_err());
        assert!(MasterKey::parse_env_hex(&"zz".repeat(KEY_LEN)).is_err());
    }

    #[test]
    fn raw_bytes_bad_length() {
        assert!(MasterKey::from_raw_bytes(&[0u8; 16]).is_err());
    }
}
