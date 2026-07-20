//! Raw HubRise callback mirror (`external_hubrise_callbacks`,
//! `specs/database/tables/integration_staging.yaml`, ADR-20260720-015400): adapter-OWNED staging —
//! one verbatim row per verified callback, UPSERTed before enrichment. HubRise callbacks yield
//! COMMANDS (ImportCatalog / stock), not `inbound_events` rows, so `processed_at` marks the
//! definitive enrichment outcome (the replay/backfill high-water mark).

use async_trait::async_trait;
use domain::shared::errors::DomainError;
use sqlx::PgPool;

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}

/// The mirror's answer to an UPSERT — lets the endpoint absorb redelivery of an already-enriched
/// callback without re-running the pull.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawCallbackState {
    /// This delivery was the first sighting of the callback id.
    pub newly_mirrored: bool,
    /// The callback was already enriched to a definitive outcome (redelivery → ACK, skip enrich).
    pub already_processed: bool,
}

/// Adapter-owned raw mirror port; trait so the endpoint flow is unit-testable in memory.
#[async_trait]
pub trait RawHubRiseCallbacks: Send + Sync {
    /// UPSERT the verified raw callback (verbatim body); reports mirror + processed state.
    async fn upsert(
        &self,
        callback_id: &str,
        resource_type: &str,
        event_type: &str,
        location_id: Option<&str>,
        payload: &serde_json::Value,
    ) -> Result<RawCallbackState, DomainError>;

    /// Stamp the enrichment high-water mark once the callback reached a definitive outcome.
    async fn mark_processed(&self, callback_id: &str) -> Result<(), DomainError>;
}

/// Postgres [`RawHubRiseCallbacks`] over `external_hubrise_callbacks`.
pub struct PgRawHubRiseCallbacks {
    pool: PgPool,
}

impl PgRawHubRiseCallbacks {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RawHubRiseCallbacks for PgRawHubRiseCallbacks {
    async fn upsert(
        &self,
        callback_id: &str,
        resource_type: &str,
        event_type: &str,
        location_id: Option<&str>,
        payload: &serde_json::Value,
    ) -> Result<RawCallbackState, DomainError> {
        // Keep the FIRST mirrored payload; report whether the callback already ran to completion.
        let row = sqlx::query_as::<_, (bool, Option<chrono::DateTime<chrono::Utc>>)>(
            "INSERT INTO external_hubrise_callbacks \
               (callback_id, resource_type, event_type, location_id, payload, received_at, processed_at) \
             VALUES ($1, $2, $3, $4, $5, now(), NULL) \
             ON CONFLICT (callback_id) DO UPDATE SET callback_id = EXCLUDED.callback_id \
             RETURNING (xmax = 0) AS newly_mirrored, processed_at",
        )
        .bind(callback_id)
        .bind(resource_type)
        .bind(event_type)
        .bind(location_id)
        .bind(payload)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(RawCallbackState { newly_mirrored: row.0, already_processed: row.1.is_some() })
    }

    async fn mark_processed(&self, callback_id: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE external_hubrise_callbacks SET processed_at = now() WHERE callback_id = $1")
            .bind(callback_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
