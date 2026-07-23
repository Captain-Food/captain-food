//! Captain.Food web frontend (Leptos → WASM) — renderer skeleton + data layer (ADR-0033/0035).
//!
//! Holds the GENERATED SDUI allowlists (`generated/registry.rs` components,
//! `generated/data_layer.rs` resolvers/actions — codegen roadmap item 6) and the hand-written
//! layers over them. Split 1/4 of #21 wired the registry + a single static screen; split 2/4 adds
//! the DATA LAYER: `session` (the persistent anonymous identity, #12), `graphql` (the transport
//! seam + `execute_resolver`, the only read entry point) and `actions` (the acceptance-first
//! two-step `dispatch`, #17). The Leptos SSR/hydration runtime that consumes them (live screens,
//! checkout, subscriptions) lands in later splits. Depends on `shared_types` + `app_core`.

use app_core::health;
use shared_types::HealthDto;

pub mod actions;
pub mod generated;
pub mod graphql;
pub mod renderer;
pub mod session;

/// Placeholder boot hook — proves the frontend can drive the shared core. Becomes the Leptos mount.
pub fn boot() -> HealthDto {
    health()
}
