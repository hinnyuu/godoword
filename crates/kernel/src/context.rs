//! 上下文装配与 token 预算（M1c）。字符启发式估算（CJK≈1/字、ASCII≈len/4，roadmap 远期换 tokenizer）。
//!
//! truncate 淘汰：系统常驻 + 新消息必留，超预算则从最旧历史逐条移出（evicted span 供 outbox 提取）。
//! summarize 淘汰由 runtime 编排（需 Summarizer role 绑定 + LLM 调用），失败回退 truncate。

use kernel_api::ChatMessage;

use crate::seam::StoredMessage;

/// 是否 CJK 字符（含假名/汉字/全角，日语学习场景）。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FA1F)
}

/// 字符启发式 token 估算：CJK 每字≈1 token，其余≈4 字符/token（向上取整）。
pub fn estimate_tokens(text: &str) -> u32 {
    let mut cjk: u32 = 0;
    let mut other: u32 = 0;
    for c in text.chars() {
        if is_cjk(c) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other.div_ceil(4)
}

/// 估算单消息 token（content + reasoning + tool_calls 序列化）。
pub fn estimate_message_tokens(msg: &ChatMessage) -> u32 {
    let mut t = estimate_tokens(&msg.content);
    if let Some(r) = &msg.reasoning {
        t += estimate_tokens(r);
    }
    if let Some(tc) = &msg.tool_calls {
        t += estimate_tokens(&tc.to_string());
    }
    t
}

/// 估算消息序列 token。
pub fn estimate_messages_tokens(msgs: &[ChatMessage]) -> u32 {
    msgs.iter().map(estimate_message_tokens).sum()
}

fn stored_tokens(msgs: &[StoredMessage]) -> u32 {
    msgs.iter()
        .map(|s| estimate_message_tokens(&s.message))
        .sum()
}

/// 装配结果：送 LLM 的消息序列 + 被淘汰 span（供 outbox SessionEvicted）。
pub struct AssembledContext {
    /// 系统 + 保留历史 + 新消息。
    pub messages: Vec<ChatMessage>,
    /// 被淘汰的历史消息（含 id/seq）。
    pub evicted: Vec<StoredMessage>,
}

/// truncate 装配：系统 + 新消息必留，超预算从最旧历史逐条淘汰至达标。
pub fn assemble_truncate(
    system: ChatMessage,
    history: Vec<StoredMessage>,
    user_msg: ChatMessage,
    budget: u32,
) -> AssembledContext {
    let fixed = estimate_message_tokens(&system) + estimate_message_tokens(&user_msg);
    let mut retained = history;
    let mut evicted: Vec<StoredMessage> = Vec::new();
    let mut total = fixed + stored_tokens(&retained);
    while total > budget && !retained.is_empty() {
        let old = retained.remove(0);
        total = total.saturating_sub(estimate_message_tokens(&old.message));
        evicted.push(old);
    }
    let mut messages = vec![system];
    messages.extend(retained.into_iter().map(|s| s.message));
    messages.push(user_msg);
    AssembledContext { messages, evicted }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_cjk_and_ascii() {
        assert_eq!(estimate_tokens("こんにちは"), 5); // 5 假名
        assert_eq!(estimate_tokens("abcd"), 1); // 4 ASCII → 1
        assert_eq!(estimate_tokens("abcde"), 2); // 5 ASCII → ceil(5/4)=2
        assert_eq!(estimate_tokens("你好abcd"), 3); // 2 CJK + 1
        assert_eq!(estimate_tokens(""), 0);
    }

    fn stored(seq: i32, content: &str) -> StoredMessage {
        StoredMessage {
            id: uuid::Uuid::new_v4(),
            seq,
            message: ChatMessage {
                role: "user".to_string(),
                content: content.to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            },
        }
    }

    #[test]
    fn truncate_evicts_oldest_over_budget() {
        let system = ChatMessage {
            role: "system".to_string(),
            content: "系统".to_string(), // 2 token
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let user = ChatMessage {
            role: "user".to_string(),
            content: "新消息".to_string(), // 3 token
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        };
        // 历史 3 条各 4 token（"啊啊啊啊"），fixed=5，total=17。budget=12 → 淘汰最旧 2 条（17→13→9）。
        let history = vec![
            stored(1, "啊啊啊啊"),
            stored(2, "呃呃呃呃"),
            stored(3, "哦哦哦哦"),
        ];
        let ctx = assemble_truncate(system, history, user, 12);
        assert_eq!(ctx.evicted.len(), 2, "淘汰最旧 2 条");
        assert_eq!(ctx.evicted[0].seq, 1);
        assert_eq!(ctx.evicted[1].seq, 2);
        // messages = system + 保留 1 条 + user = 3。
        assert_eq!(ctx.messages.len(), 3);
        assert_eq!(ctx.messages[0].role, "system");
        assert_eq!(ctx.messages[1].content, "哦哦哦哦");
        assert_eq!(ctx.messages[2].content, "新消息");
    }

    #[test]
    fn truncate_under_budget_keeps_all() {
        let system = ChatMessage {
            role: "system".to_string(),
            content: "s".to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let user = ChatMessage {
            role: "user".to_string(),
            content: "u".to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let history = vec![stored(1, "你好")];
        let ctx = assemble_truncate(system, history, user, 8000);
        assert!(ctx.evicted.is_empty());
        assert_eq!(ctx.messages.len(), 3);
    }
}
