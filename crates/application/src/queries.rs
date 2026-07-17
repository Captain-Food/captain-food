//! Query-side use-case ports (the read side, ADR-0035). Resolvers/handlers depend on these traits;
//! concrete adapters live in `infrastructure` and are injected at the `server` composition root. Read
//! ports return the generated `…Row` DTOs (what the projector writes and the query side returns).

use async_trait::async_trait;

use domain::generated::scalars::Slug;
use domain::shared::errors::DomainError;

pub use crate::generated::rows::RestaurantRow;

/// Optional filters for public restaurant discovery — mirrors the `restaurants` query args in api.yaml.
/// V0 applies a subset (the rest are accepted and ignored until the read model backs them).
#[derive(Debug, Clone, Default)]
pub struct RestaurantFilter {
    pub search: Option<String>,
    pub orderable_only: Option<bool>,
}

/// Read port over the `Restaurant` projection table (ADR-0040). Backs the `restaurants`/`restaurant`
/// GraphQL queries.
#[async_trait]
pub trait RestaurantReadRepository: Send + Sync {
    /// Discovery list (public), newest-first, honouring the filter.
    async fn list(&self, filter: RestaurantFilter) -> Result<Vec<RestaurantRow>, DomainError>;
    /// A single restaurant by its slug (the per-restaurant storefront), or `None` if absent.
    async fn by_slug(&self, slug: Slug) -> Result<Option<RestaurantRow>, DomainError>;
}
