//! 文件系统音频缓存测试：put/get round-trip + missing None + 越权 None + TTL/容量有界清理 + 原子写。

use std::time::Duration;

use bytes::Bytes;
use kernel::{AudioCache, CachedAudio, FsAudio};
use kernel_api::{AudioFormat, AudioId, UserId, WordTimestamp};
use uuid::Uuid;

fn sample_audio(pcm_len: usize, owner: UserId) -> CachedAudio {
    CachedAudio {
        pcm: Bytes::from(vec![7u8; pcm_len]),
        words: vec![WordTimestamp {
            text: "こんにちは".to_string(),
            begin_index: 0,
            end_index: 1,
            begin_time: 400,
            end_time: 800,
        }],
        format: AudioFormat::Pcm,
        sample_rate: 24000,
        owner,
    }
}

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gw-fsaudio-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn put_get_roundtrip() {
    let dir = temp_dir();
    let cache = FsAudio::new(dir.clone());
    let user = UserId::new();
    let id = AudioId::new();
    cache.put(id, sample_audio(4800, user)).await.unwrap();

    let got = cache.get(id, user).await.unwrap().expect("缓存命中");
    assert_eq!(got.pcm.len(), 4800);
    assert_eq!(got.pcm, Bytes::from(vec![7u8; 4800]));
    assert_eq!(got.words.len(), 1);
    assert_eq!(got.words[0].text, "こんにちは");
    assert_eq!(got.format, AudioFormat::Pcm);
    assert_eq!(got.sample_rate, 24000);
    assert_eq!(got.owner, user);

    // 越权负例（审计 C1）：他人读取 → None，不泄露存在性。
    assert!(
        cache.get(id, UserId::new()).await.unwrap().is_none(),
        "越权 get None"
    );

    // 原子写无残留 tmp。
    let tmp_count = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(tmp_count, 0, "无残留 tmp 文件");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn get_missing_none() {
    let dir = temp_dir();
    let cache = FsAudio::new(dir.clone());
    assert!(cache
        .get(AudioId::new(), UserId::new())
        .await
        .unwrap()
        .is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn cleanup_ttl_and_capacity() {
    let dir = temp_dir();
    let cache = FsAudio::new(dir.clone());
    let user = UserId::new();
    let a = AudioId::new();
    let b = AudioId::new();
    cache.put(a, sample_audio(100, user)).await.unwrap();
    cache.put(b, sample_audio(200, user)).await.unwrap();

    // 大 TTL + 大容量：不清理。
    let removed = cache.cleanup(Duration::from_secs(3600), 1_000_000).unwrap();
    assert_eq!(removed, 0);
    assert!(cache.get(a, user).await.unwrap().is_some());

    // TTL=0：全部过期清理（pcm + sidecar）。
    let removed = cache.cleanup(Duration::ZERO, 1_000_000).unwrap();
    assert_eq!(removed, 2);
    assert!(cache.get(a, user).await.unwrap().is_none());
    assert!(cache.get(b, user).await.unwrap().is_none());
    let json_left = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count();
    assert_eq!(json_left, 0, "sidecar 一并清理");

    // 容量上限=0：全部超容量清理。
    cache.put(a, sample_audio(100, user)).await.unwrap();
    let removed = cache.cleanup(Duration::from_secs(3600), 0).unwrap();
    assert_eq!(removed, 1);
    assert!(cache.get(a, user).await.unwrap().is_none());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn cleanup_missing_dir_ok() {
    let cache = FsAudio::new(temp_dir());
    assert_eq!(cache.cleanup(Duration::ZERO, 0).unwrap(), 0);
}

#[tokio::test]
async fn put_sidecar_failure_cleans_orphan_pcm() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let cache = FsAudio::new(dir.clone());
    let user = UserId::new();
    let id = AudioId::new();
    // 故障注入（审计 A3）：sidecar 路径占为目录 → write_atomic rename 失败。
    let json_path = dir.join(format!("{}.json", id.0));
    std::fs::create_dir(&json_path).unwrap();
    let res = cache.put(id, sample_audio(4800, user)).await;
    assert!(res.is_err(), "A3：sidecar 写失败 put 返回 Err");
    // 孤儿 PCM 已清理（无半写残留，避免 get 读缺 sidecar 致 500）。
    let pcm_path = dir.join(format!("{}.pcm", id.0));
    assert!(!pcm_path.exists(), "A3：sidecar 失败后孤儿 PCM 已清理");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn get_legacy_or_corrupt_sidecar_none() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let cache = FsAudio::new(dir.clone());
    let user = UserId::new();
    // 旧版 sidecar（无 owner 字段，审计 R9.1）→ 不可授权 → None（404 可再生），不 500。
    let id = AudioId::new();
    std::fs::write(dir.join(format!("{}.pcm", id.0)), vec![7u8; 100]).unwrap();
    let legacy = serde_json::json!({
        "words": [], "format": "pcm", "sample_rate": 24000, "created_at": "2026-01-01T00:00:00Z"
    });
    std::fs::write(dir.join(format!("{}.json", id.0)), legacy.to_string()).unwrap();
    assert!(
        cache.get(id, user).await.unwrap().is_none(),
        "R9.1：旧 sidecar 无 owner → None"
    );
    // 损坏 sidecar（非法 JSON）→ None，不 500。
    let id2 = AudioId::new();
    std::fs::write(dir.join(format!("{}.pcm", id2.0)), vec![7u8; 100]).unwrap();
    std::fs::write(dir.join(format!("{}.json", id2.0)), "{not json").unwrap();
    assert!(
        cache.get(id2, user).await.unwrap().is_none(),
        "R9.1：损坏 sidecar → None"
    );
    // PCM-only 崩溃孤儿（审计 N7）：只有 PCM 无 sidecar → None（404 可再生），不 500。
    let id3 = AudioId::new();
    std::fs::write(dir.join(format!("{}.pcm", id3.0)), vec![7u8; 100]).unwrap();
    assert!(
        cache.get(id3, user).await.unwrap().is_none(),
        "N7：PCM-only 孤儿 → None"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
