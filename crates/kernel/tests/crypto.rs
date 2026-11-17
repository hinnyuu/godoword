//! 主密钥加载集成测试（文件生成路径）。env 覆盖路径的 hex 解析逻辑见 src/crypto.rs 单元测试。
//! 用临时目录，不依赖 GODOWORD_MASTER_KEY（CI/本机默认未设）。

use kernel::MasterKey;
use uuid::Uuid;

fn temp_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("gw-crypto-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn load_generates_key_file_0600() {
    let dir = temp_dir();
    let k = MasterKey::load(&dir).unwrap();
    let key_path = dir.join("keys").join("master.key");
    assert!(key_path.exists(), "主密钥文件应生成");
    let bytes = std::fs::read(&key_path).unwrap();
    assert_eq!(bytes.len(), 32, "raw 32 字节");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "主密钥文件权限须 0600");
    }
    // 生成的密钥可加解密。
    let blob = k.encrypt(b"api-key").unwrap();
    assert_eq!(k.decrypt(&blob).unwrap(), b"api-key");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_idempotent_reads_existing() {
    let dir = temp_dir();
    let k1 = MasterKey::load(&dir).unwrap();
    let blob = k1.encrypt(b"secret").unwrap();
    // 二次加载读同一文件 → 同一密钥 → 可解。
    let k2 = MasterKey::load(&dir).unwrap();
    assert_eq!(k2.decrypt(&blob).unwrap(), b"secret", "幂等加载同一密钥");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_bad_length_file_fails() {
    let dir = temp_dir();
    let keys = dir.join("keys");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(keys.join("master.key"), [0u8; 16]).unwrap();
    assert!(MasterKey::load(&dir).is_err(), "非 32 字节主密钥须拒绝");
    std::fs::remove_dir_all(&dir).unwrap();
}
