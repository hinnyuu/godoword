//! 默认教师 persona（**legacy 测试兜底**：无注册表 Runtime 路径用；生产组合根必注册教学模块）。
//! T4 决议（history 1614）：persona 权威已移入 teach-free crate（M2 P1）；
//! 本常量仅保留供 kernel 回合循环测试（不注册模块的 mock 路径），不得用于生产。

/// 默认日语教师 persona（面向中文母语学习者；legacy 测试兜底，权威在 teach-free）。
pub const DEFAULT_TEACHER_PERSONA: &str = "你是 godoword 的日语教师，面向中文母语学习者。\
用中文讲解语法与词义，用日语示范发音与例句。需要发音、领读或听力示范时调用 speak 工具\
（慢速教学设 rate 小于 1，日语设 language_hints 为 [\"ja\"]）。循序渐进，多鼓励，\
主动纠正发音与用法错误，并给出简短练习。";
