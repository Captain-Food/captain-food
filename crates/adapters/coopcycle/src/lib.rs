//! CoopCycle delivery-partner adapter (issue #58, ADR-20260721-122910) — a self-contained vertical
//! slice and the THIRD `DeliveryProvider = PARTNER` implementation, mirroring `crates/adapters/avelo37`.
//!
//! - [`config`] — the FEDERATION registry: many self-hosted co-op instances (per-instance base URL,
//!   OAuth2 credentials, webhook secret, coverage area), parsed from `COOPCYCLE_INSTANCES`.
//! - [`acl`] — framework-free Anti-Corruption Layer: per-instance `CoopCycle-Signature` verification,
//!   co-op→domain event mapping (`task.accepted` / `task.declined` / `task.status_updated` →
//!   `DeliveryAcceptedByPartner` / `DeliveryRejectedByPartner` / `DeliveryStatusUpdated`), and the
//!   idempotent [`acl::CoopCycleWebhookIngestor`] over the two-layer inbox (ADR-20260720-015400).
//! - `http` — the thin axum shell exposing `POST /adapters/coopcycle/{instance}/webhooks`; mount
//!   [`routes`] into the monolith server, or run the standalone `coopcycle-webhook` binary (main.rs).
//! - [`outbound`] — the OUTBOUND client [`CoopCycleDeliveryGateway`], the real adapter behind the
//!   generated `DeliveryService` port: resolve the job to a co-op instance (by dropoff postal prefix),
//!   fetch that instance's OAuth2 token, and POST the offered job with our `deliveryJobId` reference.
//!
//! The co-op's answers NEVER come back through the outbound call — they arrive asynchronously as
//! verified webhooks, recorded as inbound facts (CLAUDE.md "Commands vs inbound events").

pub mod acl;
pub mod config;
mod http;
pub mod outbound;
pub mod raw;

pub use acl::{CoopCycleWebhookIngestor, RawCoopCycleEvents};
pub use config::{CoopCycleInstance, CoopCycleRegistry};
pub use http::{routes, CoopCycleWebhookState};
pub use outbound::CoopCycleDeliveryGateway;
pub use raw::PgRawCoopCycleEvents;
