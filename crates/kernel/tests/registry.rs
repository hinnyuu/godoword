//! 模块注册表测试（M2 P1a）：真实 module.yaml 反序列化（noyalib）+ 注册校验负例。

use std::sync::Arc;

use async_trait::async_trait;
use kernel::{parse_manifest, ModuleRegistry};
use kernel_api::{
    AssetAudio, ChatRequest, ChatStream, KernelError, LearningPlan, ModuleContext,
    ModuleContextFactory, ModuleKind, ModuleManifest, ModuleStore, PersistedEvent, ProgressInput,
    Role, SessionId, SessionParams, TeachingModule, ToolCall, ToolResult, TtsRequest, UserId,
};

/// 全部 crate 的真实 module.yaml（编译期内嵌；kind 覆盖 kernel-api/kernel/adapter/root/teaching 缺省）。
const MANIFESTS: &[(&str, &str, ModuleKind)] = &[
    (
        "kernel-api",
        include_str!("../../kernel-api/module.yaml"),
        ModuleKind::KernelApi,
    ),
    (
        "kernel",
        include_str!("../../kernel/module.yaml"),
        ModuleKind::Kernel,
    ),
    (
        "llm-openai",
        include_str!("../../llm-openai/module.yaml"),
        ModuleKind::Adapter,
    ),
    (
        "tts-qwen-audio",
        include_str!("../../tts-qwen-audio/module.yaml"),
        ModuleKind::Adapter,
    ),
    (
        "auth-local",
        include_str!("../../auth-local/module.yaml"),
        ModuleKind::Adapter,
    ),
    (
        "teach-free",
        include_str!("../../teach-free/module.yaml"),
        ModuleKind::Teaching,
    ),
    (
        "godoword",
        include_str!("../../godoword/module.yaml"),
        ModuleKind::Root,
    ),
];

#[test]
fn all_crate_manifests_parse() {
    for (id, yaml, kind) in MANIFESTS {
        let m = parse_manifest(yaml).unwrap_or_else(|e| panic!("{id} 解析失败：{e}"));
        assert_eq!(m.id, *id, "{id} 清单 id");
        assert_eq!(m.kind, *kind, "{id} kind");
        assert!(!m.name.is_empty(), "{id} name 非空");
    }
}

/// mock 教学模块（注册校验用）。
struct MockTeach {
    manifest: ModuleManifest,
}

impl MockTeach {
    fn new(id: &str) -> Self {
        Self {
            manifest: ModuleManifest {
                id: id.to_string(),
                name: id.to_string(),
                version: "0.0.0".to_string(),
                kind: ModuleKind::Teaching,
                description: "mock".to_string(),
                provides: None,
                requires: None,
                session_params: SessionParams::default(),
                frontend: None,
                consumes_events: vec![],
                migrations: None,
                tables_prefix: None,
            },
        }
    }
}

#[async_trait]
impl TeachingModule for MockTeach {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn persona(&self) -> String {
        "mock persona".to_string()
    }
    async fn execute_tool(
        &self,
        _ctx: &dyn ModuleContext,
        call: ToolCall,
    ) -> Result<ToolResult, kernel_api::ModuleError> {
        Ok(ToolResult {
            tool_call_id: call.id,
            ok: true,
            content: serde_json::json!({}),
        })
    }
    async fn handle_event(
        &self,
        _ctx: &dyn ModuleContext,
        _event: &PersistedEvent,
    ) -> Result<(), kernel_api::ModuleError> {
        Ok(())
    }
}

/// mock ctx（注册校验用，方法不被调用）。
struct MockCtx;

#[async_trait]
impl ModuleContext for MockCtx {
    fn user_id(&self) -> UserId {
        unimplemented!()
    }
    fn session_id(&self) -> Option<SessionId> {
        unimplemented!()
    }
    fn store(&self) -> &ModuleStore {
        unimplemented!()
    }
    async fn emit_progress(&self, _input: ProgressInput) -> Result<(), KernelError> {
        unimplemented!()
    }
    async fn current_plan(&self) -> Result<Option<LearningPlan>, KernelError> {
        unimplemented!()
    }
    async fn llm(&self, _role: Role, _req: ChatRequest) -> Result<ChatStream, KernelError> {
        unimplemented!()
    }
    async fn complete_session(&self) -> Result<bool, KernelError> {
        unimplemented!()
    }
    async fn get_or_synthesize(
        &self,
        _key: &str,
        _req: TtsRequest,
    ) -> Result<AssetAudio, KernelError> {
        unimplemented!()
    }
}

struct MockFactory;

impl ModuleContextFactory for MockFactory {
    fn for_session(&self, _user: UserId, _session: SessionId) -> Arc<dyn ModuleContext> {
        Arc::new(MockCtx)
    }
    fn for_user(&self, _user: UserId) -> Arc<dyn ModuleContext> {
        Arc::new(MockCtx)
    }
}

const MOCK_YAML: &str =
    "id: teach-mock\nname: mock\nversion: 0.0.0\nkind: teaching\ndescription: mock 教学模块\n";

#[test]
fn register_and_lookup() {
    let mut reg = ModuleRegistry::new();
    reg.register_yaml(
        MOCK_YAML,
        Arc::new(MockTeach::new("teach-mock")),
        Arc::new(MockFactory),
    )
    .unwrap();
    assert!(reg.contains("teach-mock"));
    let r = reg.get("teach-mock").expect("已注册");
    assert_eq!(r.manifest.id, "teach-mock");
    assert_eq!(r.module.persona(), "mock persona");
    assert_eq!(reg.list().len(), 1);
    assert!(reg.get("absent").is_none());
}

#[test]
fn duplicate_id_rejected() {
    let mut reg = ModuleRegistry::new();
    reg.register_yaml(
        MOCK_YAML,
        Arc::new(MockTeach::new("teach-mock")),
        Arc::new(MockFactory),
    )
    .unwrap();
    let r = reg.register_yaml(
        MOCK_YAML,
        Arc::new(MockTeach::new("teach-mock")),
        Arc::new(MockFactory),
    );
    assert!(matches!(r, Err(KernelError::Internal(m)) if m.contains("重复注册")));
    assert_eq!(reg.list().len(), 1, "拒绝后不污染注册表");
}

#[test]
fn non_teaching_kind_rejected() {
    let mut reg = ModuleRegistry::new();
    // 真实适配器清单（kind=adapter）不得注册为教学模块。
    let yaml = include_str!("../../llm-openai/module.yaml");
    let r = reg.register_yaml(
        yaml,
        Arc::new(MockTeach::new("llm-openai")),
        Arc::new(MockFactory),
    );
    assert!(matches!(r, Err(KernelError::Internal(m)) if m.contains("仅教学模块")));
}

#[test]
fn id_mismatch_rejected() {
    let mut reg = ModuleRegistry::new();
    let r = reg.register_yaml(
        MOCK_YAML,
        Arc::new(MockTeach::new("other-id")),
        Arc::new(MockFactory),
    );
    assert!(matches!(r, Err(KernelError::Internal(m)) if m.contains("不一致")));
}

#[test]
fn malformed_yaml_rejected() {
    let mut reg = ModuleRegistry::new();
    let r = reg.register_yaml(
        "id: [unclosed",
        Arc::new(MockTeach::new("x")),
        Arc::new(MockFactory),
    );
    assert!(matches!(r, Err(KernelError::Internal(m)) if m.contains("解析失败")));
    // 缺必填字段（kind）亦拒绝。
    let r = reg.register_yaml(
        "id: x\nname: x\nversion: 0.0.0\ndescription: d\n",
        Arc::new(MockTeach::new("x")),
        Arc::new(MockFactory),
    );
    assert!(r.is_err(), "缺 kind 拒绝");
}
