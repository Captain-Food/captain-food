//! Standalone HubRise webhook web service (ADR-20260718-213352): binds `$PORT` and serves ONLY
//! `POST /webhooks/hubrise`. Verified ingress (HMAC) with no DB dependency yet, so it can deploy as its
//! own Render web service isolated from other partners — or be mounted into the monolith via
//! [`hubrise_adapter::routes`]. Domain enrichment (OAuth pull) will add its own dependencies later.

use hubrise_adapter::routes;

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    println!("hubrise-webhook adapter listening on {addr}");
    axum::serve(listener, routes()).await.expect("server error");
}
