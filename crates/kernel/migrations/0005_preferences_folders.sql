-- M1d 0005：用户偏好（跨设备随账户）+ 会话虚拟路径文件夹。双兼容 SQL 子集
-- （uuid 主键应用生成、时间戳应用侧、JSON 整取回 Rust、单一 PG 风格类型名靠 SQLite 亲和性）。

-- 用户偏好：prefs_json 整取（theme/auto_play/on_disconnect_default/密度/字号/custom_css + extra）。
CREATE TABLE IF NOT EXISTS user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    prefs_json JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- 会话文件夹登记表（虚拟路径即身份）：持久空文件夹；module_id 哨兵 'default' 表无模块命名空间（chroot）。
-- path 全 NOT NULL → UNIQUE(user_id,module_id,path) 两后端正常约束（避 NULL 互不相等陷阱，bindings 哨兵教训）。
CREATE TABLE IF NOT EXISTS session_folders (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    module_id TEXT NOT NULL,
    path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (user_id, module_id, path)
);

CREATE INDEX IF NOT EXISTS idx_session_folders_user ON session_folders (user_id);

-- 会话虚拟路径（文件夹链，NULL=未分类/根）。会话名不进 path（标题首条用户消息截断另行派生）。
ALTER TABLE sessions ADD COLUMN path TEXT;