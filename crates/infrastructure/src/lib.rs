//! Captain.Food infrastructure — adapters (ADR-0035).
//!
//! Implements the traits declared in `application::ports` / `application::queries` using real I/O:
//! `persistence/` (sqlx read-model repos over the materialized projection tables, ADR-0040) and
//! `projection/` (the app-layer projection worker that folds `domain_events` into those tables via the
//! hand-written `…Compute` projectors). Later: `integrations/` (the Anti-Corruption Layer for
//! HubRise/Stripe/delivery, incl. recording inbound facts). Depends on `application` + `domain`;
//! referencing both proves the infrastructure → application, domain edges.

pub mod persistence;
pub mod projection;

pub use persistence::PgRestaurantRepository;
pub use projection::{ProjectionStatus, ProjectionWorker};
