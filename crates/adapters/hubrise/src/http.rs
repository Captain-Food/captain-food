//! HTTP shell for the HubRise adapter: `POST /webhooks/hubrise`. Thin — reads the raw body + signature,
//! delegates verification/parsing to the (framework-free) [`crate::acl`], and ACKs. Verified ingress only:
//! domain translation (OAuth pull → `OfferStockUpdated`/`ImportCatalog`) is a deliberate follow-up.

use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};

use crate::acl::{
    verify_hubrise_signature, HubRiseCallback, HUBRISE_SIGNATURE_HEADER, HUBRISE_WEBHOOK_SECRET_ENV,
};

/// Mount `POST /webhooks/hubrise`. Stateless — the current ingress needs no DB.
pub fn routes() -> Router {
    Router::new().route("/webhooks/hubrise", post(hubrise_webhook))
}

async fn hubrise_webhook(headers: HeaderMap, body: Bytes) -> Response {
    // Fail closed: without the client secret nothing can be authenticated.
    let secret = match std::env::var(HUBRISE_WEBHOOK_SECRET_ENV) {
        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "hubrise webhooks not configured (HUBRISE_WEBHOOK_SECRET unset)",
            )
                .into_response()
        }
    };
    let Some(signature) = headers.get(HUBRISE_SIGNATURE_HEADER).and_then(|v| v.to_str().ok()) else {
        return (StatusCode::BAD_REQUEST, "missing X-HubRise-Hmac-SHA256 header").into_response();
    };
    if let Err(e) = verify_hubrise_signature(&secret, signature, &body) {
        return (StatusCode::BAD_REQUEST, format!("invalid HubRise signature: {e}")).into_response();
    }
    let callback: HubRiseCallback = match serde_json::from_slice(&body) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("unparsable HubRise callback: {e}"))
                .into_response()
        }
    };
    // Verified ingress only. Domain enrichment (OAuth pull → OfferStockUpdated / ImportCatalog) is the
    // next chapter; acknowledge so HubRise stops redelivering.
    println!(
        "hubrise webhook: verified {}.{} (id {}){}",
        callback.resource_type,
        callback.event_type,
        callback.id,
        if callback.needs_pull() { " [needs API pull to enrich]" } else { "" }
    );
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "received": true, "status": "verified_pending_enrichment" })),
    )
        .into_response()
}
