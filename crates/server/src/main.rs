//! Captain.Food server binary (ADR-0035): bind `$PORT`, serve the Axum router, drain on SIGTERM.
//!
//! Migrations are applied out-of-band by **sqlx-cli in CI** (ADR-0043) — the server never runs them; it
//! only checks the schema version via `/health`. Render (ADR-0042) injects `$PORT` and sends SIGTERM on
//! deploy/scale-down; honouring it gives the graceful-drain half of the health/probe contract.

#[tokio::main]
async fn main() {
    // Print the build identity FIRST — before any fallible startup (router build, port bind, DB probe) — so
    // a boot that panics or never binds still names its version in the logs, exactly the case where /health
    // never comes up and cannot help (ADR-20260721-175411). The deployed image tag (`sha-<commit>`, pinned
    // by the deploy hook) is the platform-side source of truth for a container that never execs at all.
    println!("captain-food server starting — version {}", server::build_version());

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");

    let app = server::router();

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    println!("captain-food server listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

/// Resolve on Ctrl-C or SIGTERM (Render sends SIGTERM) so in-flight requests can drain.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        signal(SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    println!("shutdown signal received — draining");
}
