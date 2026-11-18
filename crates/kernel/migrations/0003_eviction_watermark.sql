-- M1c-hardening C7：淘汰持久水位（审计 C7：杜绝重复摘要/重复 outbox）。
-- sessions 增滚动摘要 summary + 已淘汰水位 evicted_up_to（seq）。每回合仅处理 seq > 水位的新历史，
-- 淘汰 span 摘要并入滚动摘要、水位推进、outbox 发一次。双兼容子集（ALTER ADD COLUMN 两后端支持）。

ALTER TABLE sessions ADD COLUMN summary TEXT;

ALTER TABLE sessions ADD COLUMN evicted_up_to INTEGER;