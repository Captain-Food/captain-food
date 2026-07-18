//! Raw UPSERT into the `external_sirene_restaurants` staging table (ADR-0045,
//! `specs/database/tables/integration_staging.yaml` / `migrations/20260718100000_…`).
//!
//! One row per SIRET, verbatim payload. The ingestion NEVER touches `processed_at` (the worker's
//! high-water mark) or `first_seen_at`; it bumps `last_seen_at`/`sync_run_id` and refreshes
//! `payload`/`etat`/`naf`/`department`, which makes the row pending again
//! (`processed_at < last_seen_at ⇒ pending`) for the on-app `sync_sirene_worker`.

use crate::client::{SireneError, SireneRecord};

/// UPSERT one fetched établissement into the staging table. `department` is the partition the sweep
/// queried (commune codes are prefixed by it), stamped for worker batching and re-partitioned sweeps;
/// `sync_run_id` correlates every row one ingestion run touched.
pub async fn upsert_staging_row(
    pool: &sqlx::PgPool,
    record: &SireneRecord,
    department: &str,
    sync_run_id: uuid::Uuid,
) -> Result<(), SireneError> {
    let siret = record.etablissement.siret.trim();
    // The query is active-only, so a missing periode état defaults to 'A'; NAF has no meaningful
    // default (the column is NOT NULL) — an empty string marks "not stated".
    let etat = record.etablissement.etat().unwrap_or("A");
    let naf = record.etablissement.naf().unwrap_or("");
    sqlx::query(
        "INSERT INTO external_sirene_restaurants \
           (siret, payload, etat, naf, department, first_seen_at, last_seen_at, sync_run_id, processed_at) \
         VALUES ($1, $2, $3, $4, $5, now(), now(), $6, NULL) \
         ON CONFLICT (siret) DO UPDATE SET \
           payload = EXCLUDED.payload, \
           etat = EXCLUDED.etat, \
           naf = EXCLUDED.naf, \
           department = EXCLUDED.department, \
           last_seen_at = EXCLUDED.last_seen_at, \
           sync_run_id = EXCLUDED.sync_run_id",
    )
    .bind(siret)
    .bind(&record.raw)
    .bind(etat)
    .bind(naf)
    .bind(department)
    .bind(sync_run_id)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| SireneError(format!("staging upsert for siret {siret}: {e}")))
}
