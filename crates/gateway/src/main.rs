use pairspan_gateway::{hub::Hub, router, spawn_sweeper, Config, VERSION};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// `pairspan-gateway healthcheck` exits 0 when /health answers 200 (used by Docker, which has no shell or curl).
async fn healthcheck(port: u16) -> ! {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let ok = async {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.ok()?;
        s.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.ok()?;
        let mut buf = [0u8; 16];
        s.read_exact(&mut buf).await.ok()?;
        Some(buf.starts_with(b"HTTP/1.1 200"))
    };
    let healthy = tokio::time::timeout(std::time::Duration::from_secs(3), ok).await.ok().flatten().unwrap_or(false);
    std::process::exit(if healthy { 0 } else { 1 })
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        healthcheck(port).await;
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    let cfg = Config::from_env();
    tracing::info!(
        version = VERSION,
        port,
        token = cfg.token.is_some(),
        trust_proxy = cfg.trust_proxy,
        docs = ?cfg.docs_root,
        "starting pairspan gateway"
    );

    let hub = Arc::new(Hub::new());
    spawn_sweeper(hub.clone());
    let app = router(hub, cfg).into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
        .await
        .expect("bind port");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            {
                let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("install SIGTERM handler");
                tokio::select! { _ = ctrl_c => {}, _ = term.recv() => {} }
            }
            #[cfg(not(unix))]
            let _ = ctrl_c.await;
            tracing::info!("shutting down");
        })
        .await
        .expect("server error");
}
