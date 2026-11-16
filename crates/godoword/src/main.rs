//! godoword 组合根。M0 阶段为最小 hello 服务，验证部署链路；
//! 模块注册、卷初始化、自适应降权、认证等 M1 落地。

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::services::ServeDir;

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn meta() -> Json<Value> {
    let root = kernel_api::ModuleId("godoword".to_string());
    Json(json!({
        "name": "godoword",
        "version": env!("CARGO_PKG_VERSION"),
        "milestone": "M0",
        "root": root.0,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(healthz))
        .route("/api/v1/meta", get(meta))
        .fallback_service(ServeDir::new("web/shell/public"));

    // 默认双栈监听 [::]（Linux 默认 IPV6_V6ONLY=0，兼容 IPv4），localhost 的 IPv6/IPv4 皆可达；
    // IPv6 不可用时回退 IPv4。显式 GODOWORD_LISTEN 优先。
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
    tracing::info!("godoword M0 监听 {local}");

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
