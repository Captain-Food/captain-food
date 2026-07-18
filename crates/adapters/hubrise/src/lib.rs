//! HubRise partner adapter (ADR-20260718-213352) — a self-contained vertical slice.
//!
//! - [`acl`] — framework-free Anti-Corruption Layer: `X-HubRise-Hmac-SHA256` verification + callback
//!   envelope parsing.
//! - [`api`] — the OUTBOUND OAuth2 client (domain enrichment): pull catalog/inventory from HubRise after a
//!   (stateless) callback, since those callbacks carry no state.
//! - `http` — the thin axum shell exposing `POST /webhooks/hubrise`; mount [`routes`] into the monolith
//!   server, or run the standalone `hubrise-webhook` binary (see `main.rs`) as its own web service.
//!
//! **Remaining seam (the domain wiring):** turning a pulled catalog/inventory into `ImportCatalog` /
//! `OfferStockUpdated` must match the **Catalog aggregate's** id + stream conventions so the emitted
//! events project correctly — the offer id in `OfferStockUpdated` has to equal the id `ImportCatalog`
//! assigned. The idiomatic strategy is a deterministic id from the HubRise `ref` (UUIDv5, like the SIRENE
//! ACL does with the SIRET), but it must be reconciled with the Catalog aggregate — deliberately NOT done
//! blind here. Once agreed, the flow is: callback → `api` pull → ACL map (deterministic ids) →
//! `ImportCatalog` handler / `OfferStockUpdated` append.

pub mod acl;
pub mod api;
mod http;

pub use http::routes;
