//! Postgres adapter for the Stripe raw webhook mirror (`external_stripe_events`,
//! `specs/database/tables/integration_staging.yaml`, ADR-20260720-015400): adapter-OWNED staging —
//! one verbatim row per verified Stripe event id, UPSERTed before interpretation. `processed_at` is
//! the translation high-water mark (NULL ⇒ not yet staged into `inbound_events`).

use async_trait::async_trait;
use domain::shared::errors::DomainError;
use sqlx::PgPool;

use crate::acl::RawStripeEvents;

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}

/// Postgres [`RawStripeEvents`] over `external_stripe_events`.
pub struct PgRawStripeEvents {
    pool: PgPool,
}

impl PgRawStripeEvents {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RawStripeEvents for PgRawStripeEvents {
    async fn upsert(
        &self,
        stripe_event_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<bool, DomainError> {
        // A redelivery keeps the FIRST mirrored payload (facts don't change); only the receipt is new.
        let inserted = sqlx::query(
            "INSERT INTO external_stripe_events (stripe_event_id, event_type, payload, received_at, processed_at) \
             VALUES ($1, $2, $3, now(), NULL) \
             ON CONFLICT (stripe_event_id) DO NOTHING",
        )
        .bind(stripe_event_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(inserted == 1)
    }

    async fn mark_processed(&self, stripe_event_id: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE external_stripe_events SET processed_at = now() WHERE stripe_event_id = $1")
            .bind(stripe_event_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
