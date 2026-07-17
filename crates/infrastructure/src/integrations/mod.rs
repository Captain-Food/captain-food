//! Integrations — the Anti-Corruption Layer (ADR-0035). External systems NEVER talk to the domain
//! directly: each integration translates the partner's shapes/vocabulary into ordinary domain
//! commands (or records inbound facts), keeping HubRise/Stripe/SIRENE idioms out of `domain`.
//!
//! - [`sirene`] — INSEE Sirene pull sync: food-service établissements → `RegisterRestaurant`
//!   prospects (ADR-0019/0020/0027).
//! - [`google`] — Google Business Profile seams (ownership proof + order-link probe, ADR-0019/0021);
//!   fail-closed stand-ins until the real Google adapters land.
//! - Later: HubRise (catalog import, inventory), Stripe (payment facts), delivery partner.

pub mod google;
pub mod sirene;
