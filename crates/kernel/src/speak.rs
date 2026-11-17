//! speak 工具执行准备：SpeakArgs → 解析 TTS 绑定 → TtsRequest。
//! 实际音频流迭代在回合循环内联（live 发 audio 事件）。

use kernel_api::{
    AudioFormat, AudioId, Capability, KernelError, Role, SpeakArgs, ToolCall, TtsRequest, UserId,
};
use serde_json::json;

use crate::seam::{BindingResolver, ResolvedBinding};

/// 默认音色（绑定未指定且 LLM 未选时）。
const DEFAULT_VOICE: &str = "longanlingxin";

/// 准备 speak 执行：反序列化 SpeakArgs、解析 TTS 绑定、组装 TtsRequest。
/// model/format/sample_rate/word_timestamps 由 kernel 定（不暴露给 LLM）。
pub async fn prepare_speak(
    bindings: &dyn BindingResolver,
    user: UserId,
    call: &ToolCall,
) -> Result<(ResolvedBinding, TtsRequest, AudioId), KernelError> {
    let args: SpeakArgs = serde_json::from_value(call.arguments.clone())
        .map_err(|e| KernelError::Internal(format!("speak 参数解析失败：{e}")))?;
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
        rate: args.rate,
        pitch: args.pitch,
        instruction: args.instruction,
        language_hints: args.language_hints,
        word_timestamps: true,
        extra: json!({}),
    };
    Ok((binding, req, AudioId::new()))
}
