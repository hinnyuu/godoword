//! 绑定解析 DB impl + providers/bindings 仓储 CRUD 双后端测试（纪律➀）。
//!
//! schema 表名固定，CRUD/解析集中于单测试函数内顺序遍历两后端避并行竞态；PG 跨本地运行持久，
//! 测试专属重置 public schema（CI service 每次新建）。验证 5 级 fallback + 解密 + 未配置 + CRUD + 用户隔离。

use std::sync::Arc;

use chrono::Utc;
use kernel::{
    repo, run_migrations, BindingResolver, DbBindingResolver, MasterKey, KERNEL_MIGRATIONS,
};
use kernel_api::{execute_both, Capability, ModuleId, Role, Store, UserId};
use secrecy::ExposeSecret;
use serde_json::json;
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

async fn backends() -> Vec<Store> {
    let mut v = Vec::new();
    let sqlite = SqlitePool::connect("sqlite::memory:").await.unwrap();
    // 启用外键（与生产 open_sqlite 一致），使 ON DELETE CASCADE 在两后端皆生效。
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&sqlite)
        .await
        .unwrap();
    v.push(Store::Sqlite(sqlite));
    if let Ok(url) = std::env::var("TEST_PG_URL") {
        let pg = PgPool::connect(&url).await.unwrap();
        v.push(Store::Pg(pg));
    } else {
        eprintln!("[skip] TEST_PG_URL 未设，PostgreSQL 侧跳过——仅测 SQLite（CI storage job 必设此变量跑双后端）");
    }
    v
}

async fn insert_user(store: &Store, id: Uuid) {
    let now = Utc::now();
    execute_both!(
        store,
        "INSERT INTO users (id, email, display_name, is_admin, status, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        id,
        Option::<String>::None,
        "u",
        false,
        "active",
        now,
        now
    )
    .unwrap();
}

#[tokio::test]
async fn resolver_and_repo_crud() {
    let key_dir = std::env::temp_dir().join(format!("gw-binding-key-{}", Uuid::new_v4()));
    let master = Arc::new(MasterKey::load(&key_dir).unwrap());
    let api_key = "sk-test-secret-key-abcdef";

    for store in backends().await {
        let b = store.backend();
        if let Store::Pg(p) = &store {
            sqlx::query("DROP SCHEMA public CASCADE")
                .execute(p)
                .await
                .unwrap();
            sqlx::query("CREATE SCHEMA public")
                .execute(p)
                .await
                .unwrap();
        }
        run_migrations(&store, KERNEL_MIGRATIONS, None)
            .await
            .unwrap();
        let store = Arc::new(store);
        let s: &Store = &store;
        let resolver = DbBindingResolver::new(store.clone(), master.clone());

        let user = Uuid::new_v4();
        let other = Uuid::new_v4();
        insert_user(s, user).await;
        insert_user(s, other).await;

        // 未配置 → Unconfigured。
        let err = resolver
            .resolve(UserId(user), None, Capability::Llm, Role::Default)
            .await;
        assert!(err.is_err(), "{b} 未配置须报错");

        // provider CRUD（key 加密）。
        let prov = repo::create_provider(
            s,
            &master,
            user,
            "llm-openai",
            "我的 Qwen",
            "https://example.com/compatible-mode/v1",
            api_key,
            json!({"region": "cn"}),
        )
        .await
        .unwrap();
        assert_eq!(
            repo::list_providers(s, user).await.unwrap().len(),
            1,
            "{b} list"
        );
        assert!(
            repo::get_provider(s, user, prov.id)
                .await
                .unwrap()
                .is_some(),
            "{b} get"
        );
        assert!(
            repo::get_provider(s, other, prov.id)
                .await
                .unwrap()
                .is_none(),
            "{b} 用户隔离 get"
        );
        assert!(
            repo::list_providers(s, other).await.unwrap().is_empty(),
            "{b} 用户隔离 list"
        );
        // Provider 域类型不含 key（序列化无 key_enc/key 字段）。
        let serialized = serde_json::to_string(&prov).unwrap();
        assert!(!serialized.contains("key_enc"), "{b} Provider 不暴露密文");
        assert!(!serialized.contains(api_key), "{b} Provider 不暴露明文 key");

        // 全模块默认绑定（哨兵 *）。
        let bind_default = repo::create_binding(
            s,
            user,
            "*",
            "llm",
            "default",
            prov.id,
            "model-A",
            json!({"voice": "longanlingxin"}),
        )
        .await
        .unwrap();
        let r = resolver
            .resolve(UserId(user), None, Capability::Llm, Role::Default)
            .await
            .unwrap();
        assert_eq!(r.model, "model-A", "{b} 默认绑定解析");
        assert_eq!(r.conn.api_key.expose_secret(), api_key, "{b} 解密 key 一致");
        assert_eq!(
            r.conn.base_url, "https://example.com/compatible-mode/v1",
            "{b} base_url"
        );
        assert_eq!(r.params["voice"], "longanlingxin", "{b} params 整取");

        // UNIQUE(user,module,cap,role)：重复默认绑定拒绝。
        let dup =
            repo::create_binding(s, user, "*", "llm", "default", prov.id, "dup", json!({})).await;
        assert!(dup.is_err(), "{b} 重复默认绑定拒绝");

        // role 专属 + fallback 到 default。
        repo::create_binding(
            s,
            user,
            "*",
            "llm",
            "summarizer",
            prov.id,
            "model-S",
            json!({}),
        )
        .await
        .unwrap();
        let r = resolver
            .resolve(UserId(user), None, Capability::Llm, Role::Summarizer)
            .await
            .unwrap();
        assert_eq!(r.model, "model-S", "{b} role 专属");
        let r = resolver
            .resolve(UserId(user), None, Capability::Llm, Role::Extractor)
            .await
            .unwrap();
        assert_eq!(r.model, "model-A", "{b} extractor fallback 到 default");

        // 模块覆盖 > 全模块默认。
        let gojuon = ModuleId("teach-gojuon".to_string());
        repo::create_binding(
            s,
            user,
            "teach-gojuon",
            "llm",
            "default",
            prov.id,
            "model-M",
            json!({}),
        )
        .await
        .unwrap();
        let r = resolver
            .resolve(UserId(user), Some(&gojuon), Capability::Llm, Role::Default)
            .await
            .unwrap();
        assert_eq!(r.model, "model-M", "{b} 模块覆盖");
        let other_mod = ModuleId("teach-dialogue".to_string());
        let r = resolver
            .resolve(
                UserId(user),
                Some(&other_mod),
                Capability::Llm,
                Role::Default,
            )
            .await
            .unwrap();
        assert_eq!(r.model, "model-A", "{b} 他模块回退全模块默认");

        // 模块 role 专属 > 模块 default > 全模块 role。
        repo::create_binding(
            s,
            user,
            "teach-gojuon",
            "llm",
            "assessor",
            prov.id,
            "model-MA",
            json!({}),
        )
        .await
        .unwrap();
        let r = resolver
            .resolve(UserId(user), Some(&gojuon), Capability::Llm, Role::Assessor)
            .await
            .unwrap();
        assert_eq!(r.model, "model-MA", "{b} 模块 role 专属");
        let r = resolver
            .resolve(
                UserId(user),
                Some(&gojuon),
                Capability::Llm,
                Role::Summarizer,
            )
            .await
            .unwrap();
        assert_eq!(
            r.model, "model-M",
            "{b} 模块 summarizer fallback 到模块 default"
        );

        // TTS 能力独立解析（未配置 tts → 错）。
        assert!(
            resolver
                .resolve(UserId(user), None, Capability::Tts, Role::Default)
                .await
                .is_err(),
            "{b} tts 未配置"
        );
        repo::create_binding(
            s,
            user,
            "*",
            "tts",
            "default",
            prov.id,
            "qwen-audio-3.0-tts-plus",
            json!({"voice": "longanlufeng"}),
        )
        .await
        .unwrap();
        let r = resolver
            .resolve(UserId(user), None, Capability::Tts, Role::Default)
            .await
            .unwrap();
        assert_eq!(r.model, "qwen-audio-3.0-tts-plus", "{b} tts 解析");

        // binding CRUD + update。
        assert!(
            repo::list_bindings(s, user).await.unwrap().len() >= 5,
            "{b} list bindings"
        );
        let upd = repo::update_binding(
            s,
            user,
            bind_default.id,
            prov.id,
            "model-A2",
            json!({"voice": "x"}),
        )
        .await
        .unwrap();
        assert_eq!(upd.unwrap().model, "model-A2", "{b} update binding");
        assert!(
            repo::update_binding(s, other, bind_default.id, prov.id, "hack", json!({}))
                .await
                .is_err(),
            "{b} 用户隔离 update（归属校验拒绝）"
        );
        assert!(
            !repo::delete_binding(s, other, bind_default.id)
                .await
                .unwrap(),
            "{b} 用户隔离 delete"
        );
        assert!(
            repo::delete_binding(s, user, bind_default.id)
                .await
                .unwrap(),
            "{b} delete binding"
        );

        // provider update（含 re-encrypt key）+ delete 级联。
        let upd = repo::update_provider(
            s,
            &master,
            user,
            prov.id,
            "新标签",
            "https://new.example.com/v1",
            Some("sk-new-key"),
            json!({}),
        )
        .await
        .unwrap();
        assert_eq!(upd.unwrap().label, "新标签", "{b} update provider");
        assert!(
            repo::update_provider(s, &master, other, prov.id, "x", "x", None, json!({}))
                .await
                .unwrap()
                .is_none(),
            "{b} 用户隔离 update provider"
        );
        // 跨用户 provider binding 负例（审计 C2）：不可绑定/改绑他人 provider。
        let other_prov = repo::create_provider(
            s,
            &master,
            other,
            "llm-openai",
            "他人 provider",
            "https://other.example.com/v1",
            "sk-other-secret",
            json!({}),
        )
        .await
        .unwrap();
        let cross = repo::create_binding(
            s,
            user,
            "*",
            "tts",
            "default",
            other_prov.id,
            "model-X",
            json!({}),
        )
        .await;
        assert!(cross.is_err(), "{b} 跨用户 create_binding 拒绝");
        repo::delete_provider(s, other, other_prov.id)
            .await
            .unwrap();

        // 删 provider 级联删 bindings（外键 CASCADE）。
        assert!(
            repo::delete_provider(s, user, prov.id).await.unwrap(),
            "{b} delete provider"
        );
        assert!(
            repo::list_bindings(s, user).await.unwrap().is_empty(),
            "{b} provider 删除级联 bindings"
        );

        // 清理 users。
        execute_both!(s, "DELETE FROM users WHERE id=$1", user).unwrap();
        execute_both!(s, "DELETE FROM users WHERE id=$1", other).unwrap();
    }
    std::fs::remove_dir_all(&key_dir).unwrap();
}
