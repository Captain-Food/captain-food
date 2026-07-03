//! Ports — traits the infrastructure implements (Ports & Adapters, ADR-0035). A use case that needs I/O
//! depends on one of these, never on a concrete adapter. Referencing `domain` here proves the
//! application → domain edge at compile time.

use async_trait::async_trait;
use domain::shared::{errors::DomainError, identifiers::RestaurantId};

/// Read-side port: the query handlers resolve restaurants through this. In V0 the adapter reads the
/// `View_Restaurant` SQL view over `domain_events` (ADR-0035, decision 2).
#[async_trait]
pub trait RestaurantRepository: Send + Sync {
    /// Whether a restaurant with this id is visible in the read model.
    async fn exists(&self, id: RestaurantId) -> Result<bool, DomainError>;
}
