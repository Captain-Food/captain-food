//! Captain.Food web frontend (Leptos → WASM) — skeleton (ADR-0035).
//!
//! Will hold the recursive SDUI `renderer`, the GENERATED component `registry` (from
//! `customer_screens.yaml`), the `action_dispatcher`, and the non-SDUI screens. Depends on `shared_types`
//! + `app_core` (the Crux core); referencing both proves the web → shared_types, core edges.

use app_core::health;
use shared_types::HealthDto;

/// Placeholder boot hook — proves the frontend can drive the shared core. Becomes the Leptos mount.
pub fn boot() -> HealthDto {
    health()
}
