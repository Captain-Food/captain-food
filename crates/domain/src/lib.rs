//! Captain.Food domain — the inner core (ADR-0035).
//!
//! Pure DDD: aggregates, commands, events, policies and value objects. This crate depends on **no other
//! workspace crate** — the dependency rule's innermost ring. Per decision 1 (ADR-0035) domain events and
//! value objects MAY derive `serde` (they are serialized into the append-only `domain_events` log and
//! cross the Crux/UniFFI boundary); serialization *logic* (wire formats, HubRise `"9.80 EUR"` parsing)
//! belongs in the infrastructure ACL, never here.
//!
//! Per-aggregate modules (`restaurant`, `order`, `customer`, `cart`, `review`, …) land here as the domain
//! model is generated/implemented from the specs. Only the `shared` vocabulary is scaffolded for now.

pub mod shared;
