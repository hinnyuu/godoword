//! 文件系统音频缓存测试：put/get round-trip + missing None + TTL/容量有界清理 + 原子写无残留 tmp。

use std::time::Duration;

use bytes::Bytes;
use kernel::{AudioCache, CachedAudio, FsAudio};
use kernel_api::{AudioFormat, AudioId, WordTimestamp};
use uuid::Uuid;

fn sample_audio(pcm_len: usize) -> CachedAudio {
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
    }
}

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gw-fsaudio-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn put_get_roundtrip() {
    let dir = temp_dir();
    let cache = FsAudio::new(dir.clone());
    let id = AudioId::new();
    cache.put(id, sample_audio(4800)).await.unwrap();

    let got = cache.get(id).await.unwrap().expect("缓存命中");
    assert_eq!(got.pcm.len(), 4800);
    assert_eq!(got.pcm, Bytes::from(vec![7u8; 4800]));
    assert_eq!(got.words.len(), 1);
    assert_eq!(got.words[0].text, "こんにちは");
    assert_eq!(got.format, AudioFormat::Pcm);
    assert_eq!(got.sample_rate, 24000);

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
    assert!(cache.get(AudioId::new()).await.unwrap().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn cleanup_ttl_and_capacity() {
    let dir = temp_dir();
    let cache = FsAudio::new(dir.clone());
    let a = AudioId::new();
    let b = AudioId::new();
    cache.put(a, sample_audio(100)).await.unwrap();
    cache.put(b, sample_audio(200)).await.unwrap();

    // 大 TTL + 大容量：不清理。
    let removed = cache.cleanup(Duration::from_secs(3600), 1_000_000).unwrap();
    assert_eq!(removed, 0);
    assert!(cache.get(a).await.unwrap().is_some());

    // TTL=0：全部过期清理（pcm + sidecar）。
    let removed = cache.cleanup(Duration::ZERO, 1_000_000).unwrap();
    assert_eq!(removed, 2);
    assert!(cache.get(a).await.unwrap().is_none());
    assert!(cache.get(b).await.unwrap().is_none());
    let json_left = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count();
    assert_eq!(json_left, 0, "sidecar 一并清理");

    // 容量上限=0：全部超容量清理。
    cache.put(a, sample_audio(100)).await.unwrap();
    let removed = cache.cleanup(Duration::from_secs(3600), 0).unwrap();
    assert_eq!(removed, 1);
    assert!(cache.get(a).await.unwrap().is_none());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn cleanup_missing_dir_ok() {
    let cache = FsAudio::new(temp_dir());
    assert_eq!(cache.cleanup(Duration::ZERO, 0).unwrap(), 0);
}
