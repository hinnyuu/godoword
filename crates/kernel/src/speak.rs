//! speak 工具执行准备：SpeakArgs → 解析 TTS 绑定 → TtsRequest。
//! 实际音频流迭代在回合循环内联（live 发 audio 事件）。

use kernel_api::{
    AudioFormat, AudioId, Capability, KernelError, Role, SpeakArgs, ToolCall, TtsRequest, UserId,
};
use serde_json::json;

use crate::seam::{BindingResolver, ResolvedBinding};

/// 默认音色（绑定未指定且 LLM 未选时）。
const DEFAULT_VOICE: &str = "longanlingxin";
/// speak 文本上限（审计 O9：防 LLM 巨量文本放大 TTS 内存/费用；远超正常领读需求）。
const SPEAK_TEXT_CAP: usize = 2000;
/// 语速合法区间（上游契约 [0.5, 2.0]；审计 O9：kernel 钳位不依赖上游拒绝）。
const RATE_MIN: f32 = 0.5;
const RATE_MAX: f32 = 2.0;

/// 准备 speak 执行：反序列化 SpeakArgs、解析 TTS 绑定、组装 TtsRequest。
/// model/format/sample_rate/word_timestamps 由 kernel 定（不暴露给 LLM）。
pub async fn prepare_speak(
    bindings: &dyn BindingResolver,
    user: UserId,
    call: &ToolCall,
) -> Result<(ResolvedBinding, TtsRequest, AudioId), KernelError> {
    let args: SpeakArgs = serde_json::from_value(call.arguments.clone())
        .map_err(|e| KernelError::Internal(format!("speak 参数解析失败：{e}")))?;
    if args.text.chars().count() > SPEAK_TEXT_CAP {
        return Err(KernelError::Internal(format!(
            "speak 文本超上限 {SPEAK_TEXT_CAP} 字符"
        )));
    }
    let binding = bindings
        .resolve(user, None, Capability::Tts, Role::Default)
        .await?;
    let voice = args
        .voice
        .clone()
        .or_else(|| {
            binding
                .params
                .get("voice")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| DEFAULT_VOICE.to_string());
    let req = TtsRequest {
        model: binding.model.clone(),
        text: args.text,
        voice,
        format: AudioFormat::Pcm,
        sample_rate: 24000,
        rate: args.rate.map(|r| r.clamp(RATE_MIN, RATE_MAX)),
        pitch: args.pitch,
        instruction: args.instruction,
        language_hints: args.language_hints,
        word_timestamps: true,
        extra: json!({}),
    };
    Ok((binding, req, AudioId::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seam::ResolvedBinding;
    use kernel_api::{Capability, ModuleId, ProviderConn, Role};
    use secrecy::SecretString;

    struct MockResolver;

    #[async_trait::async_trait]
    impl BindingResolver for MockResolver {
        async fn resolve(
            &self,
            _: UserId,
            _: Option<&ModuleId>,
            _: Capability,
            _: Role,
        ) -> Result<ResolvedBinding, KernelError> {
            Ok(ResolvedBinding {
                conn: ProviderConn {
                    base_url: "https://example.com".to_string(),
                    api_key: SecretString::from("k".to_string()),
                    extra: json!({}),
                },
                model: "m".to_string(),
                params: json!({ "voice": "longanlufeng" }),
            })
        }
    }

    fn call(args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "c1".to_string(),
            name: "speak".to_string(),
            arguments: args,
        }
    }

    #[tokio::test]
    async fn rate_clamped_to_contract_range() {
        let (_, req, _) = prepare_speak(
            &MockResolver,
            UserId::new(),
            &call(json!({"text":"あ","rate":99.0})),
        )
        .await
        .unwrap();
        assert_eq!(req.rate, Some(RATE_MAX), "超上限钳位");
        let (_, req, _) = prepare_speak(
            &MockResolver,
            UserId::new(),
            &call(json!({"text":"あ","rate":0.01})),
        )
        .await
        .unwrap();
        assert_eq!(req.rate, Some(RATE_MIN), "低于下限钳位");
        let (_, req, _) = prepare_speak(
            &MockResolver,
            UserId::new(),
            &call(json!({"text":"あ","rate":0.8})),
        )
        .await
        .unwrap();
        assert_eq!(req.rate, Some(0.8), "合法值不动");
    }

    #[tokio::test]
    async fn text_cap_rejected() {
        let long = "あ".repeat(SPEAK_TEXT_CAP + 1);
        let r = prepare_speak(&MockResolver, UserId::new(), &call(json!({"text": long}))).await;
        assert!(r.is_err(), "超上限拒绝（工具错误回喂 LLM）");
    }

    #[tokio::test]
    async fn voice_fallback_binding_then_default() {
        let (_, req, _) = prepare_speak(&MockResolver, UserId::new(), &call(json!({"text":"あ"})))
            .await
            .unwrap();
        assert_eq!(req.voice, "longanlufeng", "绑定音色优先于默认");
        let (_, req, _) = prepare_speak(
            &MockResolver,
            UserId::new(),
            &call(json!({"text":"あ","voice":"v"})),
        )
        .await
        .unwrap();
        assert_eq!(req.voice, "v", "LLM 选音色优先");
    }
}
