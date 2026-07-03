//! Captain.Food infrastructure — adapters (ADR-0035).
//!
//! Implements the traits declared in `application::ports` using real I/O: `persistence/` (the event store
//! appending to `domain_events`, and read-model repos querying the `View_*` SQL views), `integrations/`
//! (the Anti-Corruption Layer for HubRise/Stripe/delivery, incl. recording inbound facts), and later
//! `events/` (projectors, when a hot view graduates to a materialized table). Depends on `application` +
//! `domain`. Referencing both below proves the infrastructure → application, domain edges.

use application::ports::RestaurantRepository;
use async_trait::async_trait;
use domain::shared::{errors::DomainError, identifiers::RestaurantId};

/// Skeleton read-model adapter. The real implementation will query the `View_Restaurant` SQL view over
/// `domain_events` (ADR-0035, decision 2) via `sqlx`.
pub struct PgRestaurantRepository;

#[async_trait]
impl RestaurantRepository for PgRestaurantRepository {
    async fn exists(&self, _id: RestaurantId) -> Result<bool, DomainError> {
        Ok(false) // placeholder until the read-model view + sqlx query land
    }
}
