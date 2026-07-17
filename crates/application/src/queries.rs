//! Query-side use-case ports (the read side, ADR-0035). Resolvers/handlers depend on these traits;
//! concrete adapters live in `infrastructure` and are injected at the `server` composition root. Read
//! ports return the generated `…Row` DTOs (what the projector writes and the query side returns).

use async_trait::async_trait;

use domain::generated::scalars::{CuisineCategory, CurrencyCode, ProspectPipelineStatus, Slug};
use domain::shared::errors::DomainError;

pub use crate::generated::rows::ProspectionPipelineRow;
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

/// Optional filters for the admin prospection pipeline — mirrors the `prospectionPipeline` query args
/// in api.yaml (`minScore` / `status`).
#[derive(Debug, Clone, Default)]
pub struct ProspectFilter {
    pub min_score: Option<i32>,
    pub status: Option<ProspectPipelineStatus>,
}

/// Read port over the `ProspectionPipeline` projection table (ADR-0020/0040). Backs the admin
/// `prospectionPipeline` GraphQL query.
#[async_trait]
pub trait ProspectionReadRepository: Send + Sync {
    /// Scored prospect list (admin), best-score-first, honouring the filter.
    async fn list(&self, filter: ProspectFilter) -> Result<Vec<ProspectionPipelineRow>, DomainError>;
}

/// One `pricingpolicy` referential row (ADR-0016/0017/0037) — hand-written: referential tables are
/// seeded configuration, not projections, so no `…Row` is generated for them.
#[derive(Debug, Clone)]
pub struct PricingPolicyRow {
    pub currency: CurrencyCode,
    pub fee_rate: f64,
    pub buyer_share: f64,
    pub margin_low: f64,
    pub margin_high: f64,
    pub effective_from: chrono::DateTime<chrono::Utc>,
}

/// Read port over the seeded `PricingPolicy` referential table. Backs the admin `pricingPolicy`
/// GraphQL query.
#[async_trait]
pub trait PricingPolicyReadRepository: Send + Sync {
    /// The active fee-policy rows (one per currency), stable order.
    async fn list(&self) -> Result<Vec<PricingPolicyRow>, DomainError>;
}

/// One `uberestimationpolicy` referential row (ADR-0024/0030/0037) — hand-written, like
/// [`PricingPolicyRow`].
#[derive(Debug, Clone)]
pub struct UberEstimationPolicyRow {
    pub cuisine_category: CuisineCategory,
    pub price_coefficient: f64,
    pub effective_from: chrono::DateTime<chrono::Utc>,
}

/// Read port over the seeded `UberEstimationPolicy` referential table. Backs the admin
/// `uberEstimationPolicy` GraphQL query.
#[async_trait]
pub trait UberEstimationPolicyReadRepository: Send + Sync {
    /// The per-cuisine mark-up coefficients (one per CuisineCategory), stable order.
    async fn list(&self) -> Result<Vec<UberEstimationPolicyRow>, DomainError>;
}

/// One `ubersplitpolicy` referential row (ADR-0024/0025/0037) — hand-written, like
/// [`PricingPolicyRow`].
#[derive(Debug, Clone)]
pub struct UberSplitPolicyRow {
    pub currency: CurrencyCode,
    pub uber_commission_pct: f64,
    pub rider_base_cents: i64,
    pub rider_per_km_cents: i64,
    pub avg_delivery_fee_cents: i64,
    pub platform_fee_pct: f64,
    pub effective_from: chrono::DateTime<chrono::Utc>,
}

/// Read port over the seeded `UberSplitPolicy` referential table. Backs the admin `uberSplitPolicy`
/// GraphQL query.
#[async_trait]
pub trait UberSplitPolicyReadRepository: Send + Sync {
    /// The active split/fee assumption rows (one per currency), stable order.
    async fn list(&self) -> Result<Vec<UberSplitPolicyRow>, DomainError>;
}
