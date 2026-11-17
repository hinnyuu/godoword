//! godoword 组合根。M1b：幂等 entrypoint（配置→自适应降权→主密钥→迁移→启动）+ 存储基底 +
//! DB 绑定解析 + 会话 REST/SSE + 配置只读。dev-bypass feature 启用固定 dev user（DEV-DEBT T1，release 永不启用）；
//! 认证 M1c。

#[cfg(feature = "dev-bypass")]
mod dev;
mod init;
mod rest;

use axum::{
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "dev-bypass")]
use kernel::DbBindingResolver;
#[cfg(feature = "dev-bypass")]
use kernel::Runtime;
use kernel::{run_migrations, Config, MasterKey, MemAudio, MemSessions, KERNEL_MIGRATIONS};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::services::ServeDir;

use rest::AppState;

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn meta() -> Json<Value> {
    Json(json!({
        "name": "godoword",
        "version": env!("CARGO_PKG_VERSION"),
        "milestone": "M1b",
        "dev_bypass": cfg!(feature = "dev-bypass"),
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    // 幂等 entrypoint：配置 → 自适应降权 + /data 目录 → 主密钥 → 存储 + 迁移。
    let cfg = Config::load().expect("配置加载失败");
    let listen = cfg.listen.clone();
    init::init_data_dir(&cfg.data_dir).expect("/data 初始化失败");
    let master = MasterKey::load(&cfg.data_dir).expect("主密钥加载失败");
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

    let sessions = Arc::new(MemSessions::new());
    let audio = Arc::new(MemAudio::new());

    #[cfg(feature = "dev-bypass")]
    let (runtime, user) = {
        let resolver = Arc::new(DbBindingResolver::new(store.clone(), Arc::new(master)));
        let rt = Runtime::new(
            Arc::new(llm_openai::OpenAiProvider::new()),
            Arc::new(tts_qwen_audio::QwenTtsProvider::new()),
            resolver,
            sessions.clone(),
            audio.clone(),
        );
        tracing::warn!("dev-bypass 启用：固定 dev user + DB 绑定解析（仅开发测试，release 不含）");
        (Some(Arc::new(rt)), dev::dev_user())
    };
    #[cfg(not(feature = "dev-bypass"))]
    let (runtime, user) = {
        // release M1b 已生成主密钥文件；用户级 REST 待 M1c 认证开放，主密钥值此时不入状态。
        let _ = master;
        (None, kernel_api::UserId(uuid::Uuid::nil()))
    };

    let state = Arc::new(AppState {
        sessions,
        audio,
        runtime,
        user,
        store,
        cfg,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(rest::readyz))
        .route("/api/v1/meta", get(meta))
        .route("/api/v1/config", get(rest::config))
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
        .route("/api/v1/audio/{id}", get(rest::get_audio))
        .fallback_service(ServeDir::new("web/shell/public"))
        .with_state(state);

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
    tracing::info!("godoword M1b 监听 {local}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("服务运行失败");
}

async fn shutdown_signal() {
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
}
