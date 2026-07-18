//! Integrations — the Anti-Corruption Layer (ADR-0035). External systems NEVER talk to the domain
//! directly: each integration translates the partner's shapes/vocabulary into ordinary domain
//! commands (or records inbound facts), keeping HubRise/Stripe/SIRENE idioms out of `domain`.
//!
//! - [`sirene`] — the SIRENE ACL: raw INSEE établissements → `RegisterRestaurant` prospects
//!   (ADR-0019/0020/0027); the HTTP client/wire types live in the `sirene_ingest` crate (ADR-0045).
//! - [`sync_sirene_worker`] — the on-app worker draining the `external_sirene_restaurants` staging
//!   table through the ACL into the ordinary write path (register/close, ADR-0045).
//! - [`google`] — Google Business Profile seams (ownership proof + order-link probe, ADR-0019/0021);
//!   fail-closed stand-ins until the real Google adapters land.
//! - Later: HubRise (catalog import, inventory), Stripe (payment facts), delivery partner.

pub mod google;
pub mod sirene;
pub mod sync_sirene_worker;
