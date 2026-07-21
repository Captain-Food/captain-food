//! Postgres adapter for the CoopCycle raw webhook mirror (`external_coopcycle_events`,
//! `specs/database/tables/integration_staging.yaml`, ADR-20260721-122910): adapter-OWNED staging —
//! one verbatim row per verified event, keyed by the namespaced `"{instance_id}:{event id}"` and
//! recording its originating `instance_id` (the federation dimension). `processed_at` is the
//! translation high-water mark (NULL ⇒ not yet staged into `inbound_events`).

use async_trait::async_trait;
use domain::shared::errors::DomainError;
use sqlx::PgPool;

use crate::acl::RawCoopCycleEvents;

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}

/// Postgres [`RawCoopCycleEvents`] over `external_coopcycle_events`.
pub struct PgRawCoopCycleEvents {
    pool: PgPool,
}

impl PgRawCoopCycleEvents {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RawCoopCycleEvents for PgRawCoopCycleEvents {
    async fn upsert(
        &self,
        coopcycle_event_id: &str,
        instance_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<bool, DomainError> {
        // A redelivery keeps the FIRST mirrored payload (facts don't change); only the receipt is new.
        let inserted = sqlx::query(
            "INSERT INTO external_coopcycle_events (coopcycle_event_id, instance_id, event_type, payload, received_at, processed_at) \
             VALUES ($1, $2, $3, $4, now(), NULL) \
             ON CONFLICT (coopcycle_event_id) DO NOTHING",
        )
        .bind(coopcycle_event_id)
        .bind(instance_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(inserted == 1)
    }

    async fn mark_processed(&self, coopcycle_event_id: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE external_coopcycle_events SET processed_at = now() WHERE coopcycle_event_id = $1")
            .bind(coopcycle_event_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
