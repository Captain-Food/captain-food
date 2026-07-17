//! sqlx read repositories over the seeded REFERENTIAL policy tables (ADR-0037) — `pricingpolicy`,
//! `uberestimationpolicy`, `ubersplitpolicy`. These are configuration rows applied by the seed
//! migration, NOT projections (no worker, no events). Back the admin `pricingPolicy` /
//! `uberEstimationPolicy` / `uberSplitPolicy` GraphQL queries via `application::queries`.
//!
//! Column conventions: NUMERIC columns are SELECTed cast to `::float8` (no decimal dependency,
//! policy rates/coefficients are display values); INTEGER cents widen to `i64` in the row;
//! `cuisine_category` is the enum's INTEGER ordinal (ADR-0037, see [`super::enum_sql`]).

use application::queries::{
    PricingPolicyReadRepository, PricingPolicyRow, UberEstimationPolicyReadRepository,
    UberEstimationPolicyRow, UberSplitPolicyReadRepository, UberSplitPolicyRow,
};
use async_trait::async_trait;
use domain::generated::scalars::CurrencyCode;
use domain::shared::errors::DomainError;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use super::db_err;
use super::enum_sql::EnumOrd;

/// Postgres adapter for the PricingPolicy referential table.
pub struct PgPricingPolicyRepository {
    pool: PgPool,
}

impl PgPricingPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn decode_pricing(row: &PgRow) -> Result<PricingPolicyRow, DomainError> {
    Ok(PricingPolicyRow {
        currency: CurrencyCode(row.try_get("currency").map_err(db_err)?),
        fee_rate: row.try_get::<f64, _>("fee_rate").map_err(db_err)?,
        buyer_share: row.try_get::<f64, _>("buyer_share").map_err(db_err)?,
        margin_low: row.try_get::<f64, _>("margin_low").map_err(db_err)?,
        margin_high: row.try_get::<f64, _>("margin_high").map_err(db_err)?,
        effective_from: row.try_get("effective_from").map_err(db_err)?,
    })
}

#[async_trait]
impl PricingPolicyReadRepository for PgPricingPolicyRepository {
    /// The active fee-policy rows, one per currency (stable order by currency).
    async fn list(&self) -> Result<Vec<PricingPolicyRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT currency, fee_rate::float8 AS fee_rate, buyer_share::float8 AS buyer_share, \
             margin_low::float8 AS margin_low, margin_high::float8 AS margin_high, effective_from \
             FROM pricingpolicy ORDER BY currency",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(decode_pricing).collect()
    }
}

/// Postgres adapter for the UberEstimationPolicy referential table.
pub struct PgUberEstimationPolicyRepository {
    pool: PgPool,
}

impl PgUberEstimationPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn decode_estimation(row: &PgRow) -> Result<UberEstimationPolicyRow, DomainError> {
    Ok(UberEstimationPolicyRow {
        cuisine_category: EnumOrd::from_ord(row.try_get::<i32, _>("cuisine_category").map_err(db_err)?)?,
        price_coefficient: row.try_get::<f64, _>("price_coefficient").map_err(db_err)?,
        effective_from: row.try_get("effective_from").map_err(db_err)?,
    })
}

#[async_trait]
impl UberEstimationPolicyReadRepository for PgUberEstimationPolicyRepository {
    /// The per-cuisine mark-up coefficients (stable order by cuisine_category ordinal).
    async fn list(&self) -> Result<Vec<UberEstimationPolicyRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT cuisine_category, price_coefficient::float8 AS price_coefficient, effective_from \
             FROM uberestimationpolicy ORDER BY cuisine_category",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(decode_estimation).collect()
    }
}

/// Postgres adapter for the UberSplitPolicy referential table.
pub struct PgUberSplitPolicyRepository {
    pool: PgPool,
}

impl PgUberSplitPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn decode_split(row: &PgRow) -> Result<UberSplitPolicyRow, DomainError> {
    Ok(UberSplitPolicyRow {
        currency: CurrencyCode(row.try_get("currency").map_err(db_err)?),
        uber_commission_pct: row.try_get::<f64, _>("uber_commission_pct").map_err(db_err)?,
        rider_base_cents: i64::from(row.try_get::<i32, _>("rider_base_cents").map_err(db_err)?),
        rider_per_km_cents: i64::from(row.try_get::<i32, _>("rider_per_km_cents").map_err(db_err)?),
        avg_delivery_fee_cents: i64::from(row.try_get::<i32, _>("avg_delivery_fee_cents").map_err(db_err)?),
        platform_fee_pct: row.try_get::<f64, _>("platform_fee_pct").map_err(db_err)?,
        effective_from: row.try_get("effective_from").map_err(db_err)?,
    })
}

#[async_trait]
impl UberSplitPolicyReadRepository for PgUberSplitPolicyRepository {
    /// The active split/fee assumption rows, one per currency (stable order by currency).
    async fn list(&self) -> Result<Vec<UberSplitPolicyRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT currency, uber_commission_pct::float8 AS uber_commission_pct, rider_base_cents, \
             rider_per_km_cents, avg_delivery_fee_cents, platform_fee_pct::float8 AS platform_fee_pct, \
             effective_from FROM ubersplitpolicy ORDER BY currency",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(decode_split).collect()
    }
}
