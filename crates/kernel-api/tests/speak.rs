//! speak 契约黄金测试：锚定 `speak_tool_spec()` ↔ `SpeakArgs` 一致（防规格与类型分裂）。

use kernel_api::{speak_tool_spec, SpeakArgs, SPEAK_TOOL_NAME};
use serde_json::json;

#[test]
fn spec_name_and_required() {
    let spec = speak_tool_spec();
    assert_eq!(spec.name, SPEAK_TOOL_NAME);
    assert_eq!(spec.name, "speak");
    assert_eq!(spec.parameters["required"], json!(["text"]));
    assert_eq!(spec.parameters["type"], "object");
}

#[test]
fn spec_properties_match_speak_args_fields() {
    let spec = speak_tool_spec();
    let props = spec.parameters["properties"]
        .as_object()
        .expect("properties");
    let mut keys: Vec<&str> = props.keys().map(|s| s.as_str()).collect();
    keys.sort();
    // SpeakArgs 的 LLM 可控字段（model/format/sample_rate/word_timestamps 不暴露）
    let mut expected = vec![
        "text",
        "voice",
        "rate",
        "pitch",
        "instruction",
        "language_hints",
    ];
    expected.sort();
    assert_eq!(keys, expected, "规格 properties 须与 SpeakArgs 字段一致");
}

#[test]
fn speak_args_full_roundtrip() {
    let args = SpeakArgs {
        text: "こんにちは".to_string(),
        voice: Some("longanlingxin".to_string()),
        rate: Some(0.8),
        pitch: Some(1.0),
        instruction: Some("缓慢清晰地领读".to_string()),
        language_hints: vec!["ja".to_string()],
    };
    let v = serde_json::to_value(&args).unwrap();
    assert_eq!(v["text"], "こんにちは");
    assert_eq!(v["rate"].as_f64().unwrap() as f32, 0.8f32);
    assert_eq!(v["language_hints"], json!(["ja"]));
    let back: SpeakArgs = serde_json::from_value(v).unwrap();
    assert_eq!(back.text, args.text);
    assert_eq!(back.voice, args.voice);
    assert_eq!(back.rate, args.rate);
}

#[test]
fn speak_args_minimal_defaults() {
    let args: SpeakArgs = serde_json::from_value(json!({ "text": "你好" })).unwrap();
    assert_eq!(args.text, "你好");
    assert!(args.voice.is_none());
    assert!(args.rate.is_none());
    assert!(args.language_hints.is_empty());
    // 最小序列化只含 text（可选字段省略）
    let v = serde_json::to_value(&args).unwrap();
    assert_eq!(v, json!({ "text": "你好" }));
}

#[test]
fn spec_is_valid_tool_spec_shape() {
    let spec = speak_tool_spec();
    // 可被 OpenAI function 形包裹（适配器 build_request 用）
    let wrapped = json!({
        "type": "function",
        "function": { "name": spec.name, "description": spec.description, "parameters": spec.parameters }
    });
    assert_eq!(wrapped["function"]["name"], "speak");
    assert!(wrapped["function"]["parameters"]["properties"]["text"].is_object());
}
