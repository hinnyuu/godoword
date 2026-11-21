-- M2 P1（0006）：模块切片 schema 基底 + teach-free 迁移 + 持久资产登记表。
-- plan_version 类型对齐（B3b 定案 history 2130）：TEXT→INTEGER。该列自 0001 起从未被写入
--（生产/测试皆绑 NULL），DROP+ADD 零数据风险；LearningPlan.version/RunStart.plan_version 皆 u32。
ALTER TABLE sessions DROP COLUMN plan_version;
ALTER TABLE sessions ADD COLUMN plan_version INTEGER;
-- 完成语义（complete_session，完成≠删除；B2 DELETE 翻转归 P2）。
ALTER TABLE sessions ADD COLUMN completed_at TIMESTAMPTZ;
-- call_log 模块归因（B4：用量看板按模块聚合 + ctx.llm 全计量；会话删除后 join 丢归因故写入时填）。
ALTER TABLE call_log ADD COLUMN module_id TEXT;
-- 进度事件结构化列（B1：ProgressInput 早有 outcome/item_ref/difficulty，DB 对齐；
-- difficulty 为契约 ProgressEvent 形状新增，诚实标注）。REAL ↔ f32 双后端。
ALTER TABLE progress_events ADD COLUMN outcome TEXT;
ALTER TABLE progress_events ADD COLUMN item_ref TEXT;
ALTER TABLE progress_events ADD COLUMN difficulty REAL;
-- teach-free 迁移（T4 决议 history 1614：既有无模块会话与 default 文件夹迁入 teach-free；
-- bindings 哨兵 `*` 不动）。此后新会话创建缺省 module=teach-free（组合根）。
UPDATE sessions SET module_id = 'teach-free' WHERE module_id IS NULL;
UPDATE session_folders SET module_id = 'teach-free' WHERE module_id = 'default';
-- 白名单持久资产登记表（UX-A3 用户侧标准音频：一次生成永久使用，豁免 TTL/容量淘汰）。
-- 作用域：user+module+稳定 key+provider 身份（adapter_id+base_url）+voice+model（换音色/服务商懒再生新版旧版保留）。
-- PCM 文件存 /data/assets/<audio_id>.pcm + sidecar（与 /data/cache 回放缓存正交）。
CREATE TABLE IF NOT EXISTS audio_assets (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    module_id TEXT NOT NULL,
    stable_key TEXT NOT NULL,
    adapter_id TEXT NOT NULL,
    base_url TEXT NOT NULL,
    voice TEXT NOT NULL,
    model TEXT NOT NULL,
    audio_id UUID NOT NULL,
    format TEXT NOT NULL,
    sample_rate INTEGER NOT NULL,
    words JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE(user_id, module_id, stable_key, adapter_id, base_url, voice, model)
);
CREATE INDEX IF NOT EXISTS idx_audio_assets_user ON audio_assets(user_id);