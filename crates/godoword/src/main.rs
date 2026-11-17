//! godoword 组合根。M1a：注册 llm-openai/tts-qwen-audio 适配器 + kernel 回合循环 + 会话 REST/SSE。
//! dev-bypass feature 启用 env key 旁路（DEV-DEBT T1，release 永不启用）；持久化/认证 M1b/M1c。

#[cfg(feature = "dev-bypass")]
mod dev;
mod rest;

use axum::{
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "dev-bypass")]
use kernel::Runtime;
use kernel::{MemAudio, MemSessions};
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
        "milestone": "M1a",
        "dev_bypass": cfg!(feature = "dev-bypass"),
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let sessions = Arc::new(MemSessions::new());
    let audio = Arc::new(MemAudio::new());

    #[cfg(feature = "dev-bypass")]
    let (runtime, user) = {
        let rt = Runtime::new(
            Arc::new(llm_openai::OpenAiProvider::new()),
            Arc::new(tts_qwen_audio::QwenTtsProvider::new()),
            Arc::new(dev::EnvResolver),
            sessions.clone(),
            audio.clone(),
        );
        tracing::warn!("dev-bypass 启用：env key 旁路 + 固定 dev user（仅开发测试，release 不含）");
        (Some(Arc::new(rt)), dev::dev_user())
    };
    #[cfg(not(feature = "dev-bypass"))]
    let (runtime, user) = (None, kernel_api::UserId(uuid::Uuid::nil()));

    let state = Arc::new(AppState {
        sessions,
        audio,
        runtime,
        user,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(healthz))
        .route("/api/v1/meta", get(meta))
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

    // 默认双栈监听 [::]（兼容 IPv4），IPv6 不可用回退 0.0.0.0；GODOWORD_LISTEN 可覆盖。
    let listen = std::env::var("GODOWORD_LISTEN").unwrap_or_else(|_| "[::]:8080".to_string());
    let addr: std::net::SocketAddr = listen
        .parse()
        .expect("GODOWORD_LISTEN 须为合法 socket 地址");
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
    tracing::info!("godoword M1a 监听 {local}");

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
