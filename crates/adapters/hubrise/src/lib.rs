//! HubRise partner adapter (ADR-20260718-213352) — a self-contained vertical slice.
//!
//! - [`acl`] — framework-free Anti-Corruption Layer: `X-HubRise-Hmac-SHA256` verification + callback
//!   envelope parsing. Domain translation (→ `OfferStockUpdated`/`ImportCatalog`) is a deliberate
//!   follow-up: HubRise catalog/inventory callbacks carry no state and need an OAuth API pull + a
//!   ref-mapping (which will add `application`/`infrastructure` deps).
//! - `http` — the thin axum shell exposing `POST /webhooks/hubrise`; mount [`routes`] into the monolith
//!   server, or run the standalone `hubrise-webhook` binary (see `main.rs`) as its own web service.

pub mod acl;
mod http;

pub use http::routes;
