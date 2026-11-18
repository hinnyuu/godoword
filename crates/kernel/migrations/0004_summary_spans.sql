-- M1c 二审收尾 R4：summarize 计费幂等键（span 消息 ID 集哈希）。
-- 淘汰 span 摘要 fragment 持久化：LLM 调用前查 span_hash 命中复用免重复计费；调用后落 fragment，
-- 再与 outbox + 水位同事务。evict 失败下回合复用 fragment 不重计费。双兼容子集。
-- session_id 外键 CASCADE：会话删除时其摘要 span 一并清理。

CREATE TABLE IF NOT EXISTS summary_spans (
    span_hash TEXT PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    fragment TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_summary_spans_session ON summary_spans (session_id);