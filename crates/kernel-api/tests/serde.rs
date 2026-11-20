//! 协议类型 serde 黄金测试（R2：前后端线上协议锚点）。
//!
//! SessionEvent（tag=event, content=data, kebab-case）round-trip + 黄金 JSON；
//! ModuleManifest 反序列化 module.yaml。M1 写 Svelte 前端时即契约锚点。

use kernel_api::llm::FinishReason;
use kernel_api::types::{AudioFormat, AudioId, Capability, ModuleKind, RunId, WordTimestamp};
use kernel_api::{ModuleManifest, SessionEvent};
use serde_json::json;

fn roundtrip(e: &SessionEvent, tag: &str) -> serde_json::Value {
    let v = serde_json::to_value(e).unwrap();
    assert_eq!(v["event"], tag, "event 标签");
    assert!(v.get("data").is_some(), "须有 data 载荷");
    let back: SessionEvent = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(back).unwrap(), v, "round-trip 一致");
    v
}

#[test]
fn text_delta_golden() {
    let v = roundtrip(
        &SessionEvent::TextDelta {
            text: "こんにちは".into(),
        },
        "text-delta",
    );
    assert_eq!(
        v,
        json!({"event": "text-delta", "data": {"text": "こんにちは"}})
    );
}

#[test]
fn reasoning_delta_golden() {
    let v = roundtrip(
        &SessionEvent::ReasoningDelta {
            text: "thinking".into(),
        },
        "reasoning-delta",
    );
    assert_eq!(v["data"]["text"], "thinking");
}

#[test]
fn run_start_golden() {
    let e = SessionEvent::RunStart {
        run_id: RunId::new(),
        plan_version: Some(3),
        model: "qwen4-max-preview".into(),
    };
    let v = roundtrip(&e, "run-start");
    assert_eq!(v["data"]["plan_version"], 3);
    assert_eq!(v["data"]["model"], "qwen4-max-preview");
}

#[test]
fn audio_chunk_golden() {
    let e = SessionEvent::AudioChunk {
        audio_id: AudioId::new(),
        seq: 7,
        data: "AAEC".into(),
    };
    let v = roundtrip(&e, "audio-chunk");
    assert_eq!(v["data"]["seq"], 7);
    assert_eq!(v["data"]["data"], "AAEC");
}

#[test]
fn audio_start_golden() {
    let e = SessionEvent::AudioStart {
        audio_id: AudioId::new(),
        format: AudioFormat::Pcm,
        sample_rate: 24000,
    };
    let v = roundtrip(&e, "audio-start");
    assert_eq!(v["data"]["format"], "pcm");
    assert_eq!(v["data"]["sample_rate"], 24000);
}

#[test]
fn word_timestamps_golden() {
    let e = SessionEvent::WordTimestamps {
        audio_id: AudioId::new(),
        words: vec![WordTimestamp {
            text: "こんにちは".into(),
            begin_index: 0,
            end_index: 1,
            begin_time: 160,
            end_time: 640,
        }],
    };
    let v = roundtrip(&e, "word-timestamps");
    assert_eq!(v["data"]["words"][0]["begin_time"], 160);
    assert_eq!(v["data"]["words"][0]["end_time"], 640);
}

#[test]
fn error_golden() {
    let e = SessionEvent::Error {
        code: "auth".into(),
        message: "key 无效".into(),
        retryable: false,
        action: Some("open_settings".into()),
    };
    let v = roundtrip(&e, "error");
    assert_eq!(v["data"]["code"], "auth");
    assert_eq!(v["data"]["retryable"], false);
    assert_eq!(v["data"]["action"], "open_settings");
}

#[test]
fn run_end_golden() {
    let e = SessionEvent::RunEnd {
        run_id: RunId::new(),
        finish_reason: FinishReason::Stop,
    };
    let v = roundtrip(&e, "run-end");
    assert_eq!(v["data"]["finish_reason"], "stop");
}

#[test]
fn usage_golden() {
    let e = SessionEvent::Usage {
        capability: Capability::Llm,
        model: "qwen4-max-preview".into(),
        usage: json!({"total_tokens": 55}),
    };
    let v = roundtrip(&e, "usage");
    assert_eq!(v["data"]["capability"], "llm");
    assert_eq!(v["data"]["usage"]["total_tokens"], 55);
}

// ModuleManifest 的 Deserialize 契约锚点（与格式无关；module.yaml 的 YAML 解析为 noyalib，
// B7 定案 history 2130，kernel 注册表测试锚定真实 YAML；此处用 JSON 验证 serde 属性与默认值）。
const TEACHING_JSON: &str = r#"{
  "id": "teach-gojuon",
  "name": "五十音图",
  "version": "0.1.0",
  "kind": "teaching",
  "description": "五十音图自适应教学",
  "requires": { "capabilities": ["llm", "tts"] },
  "session_params": { "context_budget_tokens": 8000, "eviction": "truncate", "tool_iteration_cap": 8 },
  "frontend": { "entry": "web/modules/teach-gojuon/dist/index.js", "routes": ["/gojuon"] }
}"#;

#[test]
fn manifest_teaching() {
    let m: ModuleManifest = serde_json::from_str(TEACHING_JSON).unwrap();
    assert_eq!(m.id, "teach-gojuon");
    assert_eq!(m.kind, ModuleKind::Teaching);
    let req = m.requires.unwrap();
    assert_eq!(req.capabilities, vec![Capability::Llm, Capability::Tts]);
    assert_eq!(m.session_params.context_budget_tokens, 8000);
    assert_eq!(m.frontend.unwrap().routes, vec!["/gojuon"]);
}

#[test]
fn manifest_adapter() {
    let json = r#"{
      "id": "llm-openai",
      "name": "OpenAI 兼容 LLM",
      "version": "0.1.0",
      "kind": "adapter",
      "description": "OpenAI 兼容协议适配器",
      "provides": { "capability": "llm", "adapter_id": "llm-openai" }
    }"#;
    let m: ModuleManifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.kind, ModuleKind::Adapter);
    let p = m.provides.unwrap();
    assert_eq!(p.capability, Capability::Llm);
    assert_eq!(p.adapter_id, "llm-openai");
    // session_params 缺省取默认
    assert_eq!(m.session_params.tool_iteration_cap, 8);
}

#[test]
fn manifest_kernel_api_kind_kebab() {
    let json = r#"{
      "id": "kernel-api", "name": "契约", "version": "0.0.0",
      "kind": "kernel-api", "description": "trait 与类型"
    }"#;
    let m: ModuleManifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.kind, ModuleKind::KernelApi);
}

#[test]
fn manifest_kernel_kind_kebab() {
    let json = r#"{
      "id": "kernel", "name": "内核", "version": "0.0.0",
      "kind": "kernel", "description": "回合循环与内核服务"
    }"#;
    let m: ModuleManifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.kind, ModuleKind::Kernel);
}

// ProgressEvent 信封黄金（M2 P1：difficulty 契约形状新增，审计 B1；serde default 旧信封兼容）。
#[test]
fn progress_event_difficulty_golden() {
    let ev = kernel_api::ProgressEvent {
        id: kernel_api::EventId::new(),
        user_id: kernel_api::UserId::new(),
        module_id: kernel_api::ModuleId("teach-gojuon".to_string()),
        session_id: None,
        ts: chrono::Utc::now(),
        kind: kernel_api::EventKind::ExerciseOutcome,
        skill_tags: vec!["kana:あ".to_string()],
        outcome: Some(kernel_api::Outcome::Correct),
        item_ref: Some("あ".to_string()),
        difficulty: Some(0.25),
        detail: serde_json::json!({ "mode": "identify" }),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["kind"], "exercise-outcome", "kind kebab-case");
    assert_eq!(v["outcome"], "correct", "outcome lowercase");
    assert_eq!(v["difficulty"], 0.25, "difficulty 形状新增");
    assert_eq!(
        v["session_id"],
        serde_json::Value::Null,
        "for_user 事件无会话"
    );
    let back: kernel_api::ProgressEvent = serde_json::from_value(v).unwrap();
    assert_eq!(back.difficulty, Some(0.25), "round-trip");
    // 旧信封（无 difficulty 字段）serde default 兼容。
    let old = r#"{
        "id": "00000000-0000-0000-0000-000000000001",
        "user_id": "00000000-0000-0000-0000-000000000002",
        "module_id": "kernel",
        "session_id": null,
        "ts": "2026-11-20T00:00:00Z",
        "kind": "session-ended",
        "skill_tags": [],
        "outcome": null,
        "item_ref": null,
        "detail": {}
    }"#;
    let ev: kernel_api::ProgressEvent = serde_json::from_str(old).unwrap();
    assert_eq!(ev.difficulty, None, "旧信封 default 兼容");
}
