//! godoword 组合根。M1c：幂等 entrypoint（配置→自适应降权→主密钥→迁移→启动）+ 存储基底 +
//! DB 绑定解析 + 会话 REST/SSE + provider/binding CRUD + 认证/管理 REST + 配置只读 + OpenAPI。
//! 用户级端点经 cookie 会话认证（auth-local）；dev-bypass 已删除（DEV-DEBT T1 回归）。

mod auth;
mod folders;
mod init;
mod openapi;
mod prefs;
mod rest;
mod settings;
#[cfg(test)]
mod testutil;

use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use kernel::{
    run_migrations, AudioCache, Config, DbBindingResolver, DbSessions, FsAudio, MasterKey, Runtime,
    SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{LlmProvider, TtsProvider};
use std::sync::Arc;
use tower_http::services::ServeDir;

use rest::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    // 幂等 entrypoint：配置 → 自适应降权 + /data 目录 → 主密钥 → 存储 + 迁移。
    let cfg = Config::load().expect("配置加载失败");
    let listen = cfg.listen.clone();
    init::init_data_dir(&cfg.data_dir).expect("/data 初始化失败");
    let master = Arc::new(MasterKey::load(&cfg.data_dir).expect("主密钥加载失败"));
    let store = Arc::new(match &cfg.database_url {
        Some(url) => kernel::open_pg(url).await.expect("PostgreSQL 连接失败"),
        None => kernel::open_sqlite(&cfg.sqlite_path())
            .await
            .expect("SQLite 打开失败"),
    });
    let applied = run_migrations(&store, KERNEL_MIGRATIONS, Some(&cfg.backup_dir()))
        .await
        .expect("数据库迁移失败");
    tracing::info!("存储就绪（{}），本次应用迁移 {applied} 条", store.backend());

    let sessions: Arc<dyn SessionRepo> = Arc::new(DbSessions::new(store.clone()));
    let fs_audio = Arc::new(FsAudio::new(cfg.cache_dir()));
    let audio: Arc<dyn AudioCache> = fs_audio.clone();

    // 音频缓存周期清理（TTL + 容量上限，防 PCM 撑满磁盘；缓存可再生，淘汰只损失旧回放）。
    {
        let cache = fs_audio.clone();
        let ttl = cfg.cache_ttl();
        let max_bytes = cfg.cache_max_bytes;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                let c = cache.clone();
                match tokio::task::spawn_blocking(move || c.cleanup(ttl, max_bytes)).await {
                    Ok(Ok(n)) if n > 0 => tracing::info!("音频缓存清理 {n} 条"),
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => tracing::warn!("音频缓存清理失败：{e}"),
                    Err(e) => tracing::warn!("音频缓存清理任务失败：{e}"),
                }
            }
        });
    }

    // outbox dispatcher 骨架（M1c 无教学模块消费者；M2 注册模块 handle_event 消费 progress_events）。
    {
        let store = store.clone();
        let handlers: std::collections::HashMap<String, Arc<dyn kernel::OutboxHandler>> =
            std::collections::HashMap::new();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = kernel::dispatch_once(&store, &handlers, 100).await {
                    tracing::warn!("outbox 投递失败：{e}");
                }
            }
        });
    }

    // 适配器 + DB 绑定解析 + 运行时（M1c 无条件构建，release 含完整回合能力）。
    let llm: Arc<dyn LlmProvider> = Arc::new(llm_openai::OpenAiProvider::new());
    let tts: Arc<dyn TtsProvider> = Arc::new(tts_qwen_audio::QwenTtsProvider::new());
    let resolver = Arc::new(DbBindingResolver::new(store.clone(), master.clone()));

    // 模块注册表（M2 P1）：feature 注册教学模块 + ctx 工厂注入（module_id 构造期绑定）。
    // 持久资产区 /data/assets（UX-A3 用户侧标准音频，豁免淘汰）。
    let assets = Arc::new(kernel::AssetStore::new(cfg.assets_dir(), store.clone()));
    let mut registry = kernel::ModuleRegistry::new();
    #[cfg(feature = "teach-free")]
    {
        let factory = Arc::new(kernel::KernelCtxFactory::new(
            kernel_api::ModuleId(teach_free::TEACH_FREE_ID.to_string()),
            store.clone(),
            resolver.clone(),
            llm.clone(),
            tts.clone(),
            assets.clone(),
            None, // PlanProvider：teach-assess P3 注册
        ));
        registry
            .register_yaml(teach_free::module_yaml(), teach_free::module(), factory)
            .expect("teach-free 注册失败");
    }
    let registry = Arc::new(registry);
    tracing::info!("模块注册表就绪（{} 个教学模块）", registry.list().len());

    let runtime = Arc::new(
        Runtime::new(
            llm.clone(),
            tts.clone(),
            resolver,
            sessions.clone(),
            audio.clone(),
            store.clone(),
        )
        .with_detach_cap(cfg.max_detached_turns as usize)
        .with_modules(registry.clone()),
    );

    let shutdown_rt = runtime.clone();
    let shutdown_store = store.clone();
    // 可信代理解析（审计 R9.4）：非法项告警跳过；空则不信任 XFF（直连语义）。
    let trusted_proxies: Vec<ipnet::IpNet> = cfg
        .trusted_proxies
        .iter()
        .filter_map(|s| {
            let net = auth::parse_trusted(s);
            if net.is_none() {
                tracing::warn!("忽略非法 trusted_proxies 项：{s}");
            }
            net
        })
        .collect();
    let state = Arc::new(AppState {
        sessions,
        audio,
        runtime,
        store,
        cfg,
        master,
        llm,
        tts,
        registry,
        rate_limiter: Arc::new(auth::RateLimiter::new()),
        trusted_proxies,
    });

    let app = Router::new()
        .route("/healthz", get(rest::healthz))
        .route("/readyz", get(rest::readyz))
        .route("/api/v1/meta", get(rest::meta))
        .route("/api/v1/modules", get(rest::list_modules))
        .route("/api/v1/config", get(rest::config))
        .route("/api/v1/openapi.json", get(openapi::openapi_json))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/reset-password", post(auth::reset_password))
        .route("/api/v1/auth/change-password", post(auth::change_password))
        .route("/api/v1/auth/me", get(auth::me).put(auth::update_me))
        .route("/api/v1/admin/reset", post(auth::admin_reset))
        .route("/api/v1/admin/invites", post(auth::admin_invite))
        .route("/api/v1/admin/users", get(auth::admin_users))
        .route(
            "/api/v1/admin/users/{id}/status",
            post(auth::admin_set_status),
        )
        .route(
            "/api/v1/sessions",
            post(rest::create_session).get(rest::list_sessions),
        )
        .route(
            "/api/v1/sessions/{id}",
            get(rest::get_session).delete(rest::delete_session),
        )
        .route("/api/v1/sessions/{id}/messages", post(rest::messages))
        .route("/api/v1/sessions/{id}/cancel", post(rest::cancel))
        .route(
            "/api/v1/sessions/{id}/complete",
            post(rest::complete_session),
        )
        .route("/api/v1/sessions/{id}/path", put(rest::move_session_path))
        .route("/api/v1/audio/{id}", get(rest::get_audio))
        .route("/api/v1/audio/{id}/meta", get(rest::audio_meta))
        .route(
            "/api/v1/preferences",
            get(prefs::get_preferences).put(prefs::put_preferences),
        )
        .route(
            "/api/v1/folders",
            get(folders::list_folders).post(folders::create_folder),
        )
        .route(
            "/api/v1/folders/{id}",
            put(folders::rename_folder).delete(folders::delete_folder),
        )
        .route(
            "/api/v1/providers",
            post(settings::create_provider).get(settings::list_providers),
        )
        .route(
            "/api/v1/providers/{id}",
            get(settings::get_provider)
                .put(settings::update_provider)
                .delete(settings::delete_provider),
        )
        .route(
            "/api/v1/providers/{id}/models",
            get(settings::provider_models),
        )
        .route(
            "/api/v1/providers/{id}/voices",
            get(settings::provider_voices),
        )
        .route(
            "/api/v1/providers/{id}/preview",
            post(settings::provider_preview),
        )
        .route(
            "/api/v1/bindings",
            post(settings::create_binding).get(settings::list_bindings),
        )
        .route(
            "/api/v1/bindings/{id}",
            get(settings::get_binding)
                .put(settings::update_binding)
                .delete(settings::delete_binding),
        );

    // 模块路由挂载（M2 P1）：/api/v1/m/{id}/（TeachingModule.routes，签名不动；
    // 模块路由自带 ctx 工厂注入，不依赖 AppState）。先 with_state 再 nest（Router<()> 同型），
    // CSRF 层最后包裹（覆盖模块路由）。
    let mut app = app
        .fallback_service(ServeDir::new("web/shell/public"))
        .with_state(state.clone());
    for r in state.registry.list() {
        app = app.nest(&format!("/api/v1/m/{}", r.manifest.id), r.module.routes());
    }
    let app = app.layer(middleware::from_fn(auth::csrf_guard));

    // 默认双栈监听 [::]（兼容 IPv4），IPv6 不可用回退 0.0.0.0；配置 listen 可覆盖。
    let addr: std::net::SocketAddr = listen.parse().expect("listen 须为合法 socket 地址");
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if addr.is_ipv6() => {
            let fallback: std::net::SocketAddr = format!("0.0.0.0:{}", addr.port())
                .parse()
                .expect("回退地址合法");
            tracing::warn!("IPv6 绑定失败（{e}），回退 {fallback}");
            tokio::net::TcpListener::bind(fallback)
                .await
                .expect("绑定端口失败")
        }
        Err(e) => panic!("绑定端口失败：{e}"),
    };
    let local = listener.local_addr().expect("读取本地地址");
    tracing::info!("godoword 监听 {local}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_rt, shutdown_store))
    .await
    .expect("服务运行失败");
}

async fn shutdown_signal(runtime: Arc<Runtime>, store: Arc<kernel_api::Store>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("安装 Ctrl-C 处理失败");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("安装 SIGTERM 处理失败")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到退出信号，优雅关闭");
    // 退出屏障（审计 C6）：cancel + 有界等待 detached 落盘 + WAL checkpoint。
    runtime.shutdown(std::time::Duration::from_secs(5)).await;
    if let Err(e) = kernel::checkpoint(&store).await {
        tracing::warn!("退出 WAL checkpoint 失败：{e}");
    }
}
