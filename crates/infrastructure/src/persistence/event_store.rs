//! Postgres adapter for the `application::ports::EventStore` write port (ADR-0035): appends business
//! events to the append-only `domain_events` log. The technical envelope (event id, stream/version,
//! acting user, correlation, occurred_at) is stamped HERE — payloads stay business-only (ADR-0041).
//! Optimistic concurrency rides the UNIQUE(stream_name, version) constraint: a clash maps to the
//! canonical `version_conflict` DomainError so handlers can absorb idempotent replays.

use application::ports::{version_conflict, Actor, EventStore};
use async_trait::async_trait;
use domain::generated::events::DomainEvent;
use domain::shared::errors::DomainError;
use sqlx::PgPool;

use crate::persistence::db_err;

pub struct PgEventStore {
    pool: PgPool,
}

impl PgEventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventStore for PgEventStore {
    async fn append(
        &self,
        stream_name: &str,
        expected_version: i64,
        events: &[DomainEvent],
        actor: &Actor,
    ) -> Result<i64, DomainError> {
        // One transaction per append: a multi-event emission lands atomically or not at all, and a
        // version clash on ANY row rolls the whole batch back.
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        for (index, event) in events.iter().enumerate() {
            let version = expected_version + index as i64 + 1;
            let (event_type, payload) = split_event(event)?;

            let insert = sqlx::query(
                "INSERT INTO domain_events \
                 (id, stream_name, version, user_id, user_type, correlation_id, cause_id, \
                  event_type, payload, metadata, occurred_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, now())",
            )
            .bind(uuid::Uuid::new_v4())
            .bind(stream_name)
            .bind(i32::try_from(version).map_err(db_err)?)
            .bind(actor.user_id)
            .bind(actor.user_type)
            .bind(actor.correlation_id)
            .bind(actor.cause_id)
            .bind(&event_type)
            .bind(payload)
            .execute(&mut *tx)
            .await;

            if let Err(e) = insert {
                // The event id is a fresh v4 UUID, so a unique violation here is (in practice) the
                // (stream_name, version) guard — i.e. we lost the optimistic-concurrency race.
                if is_unique_violation(&e) {
                    return Err(version_conflict(stream_name, expected_version));
                }
                return Err(db_err(e));
            }
        }

        tx.commit().await.map_err(db_err)?;
        Ok(expected_version + events.len() as i64)
    }
}

/// Split the adjacently-tagged [`DomainEvent`] (`{"eventType": …, "payload": …}`) into the
/// `event_type` + `payload` columns of `domain_events`.
fn split_event(event: &DomainEvent) -> Result<(String, serde_json::Value), DomainError> {
    let tagged = serde_json::to_value(event).map_err(db_err)?;
    let event_type = tagged
        .get("eventType")
        .and_then(|t| t.as_str())
        .ok_or_else(|| db_err("DomainEvent serialized without an eventType tag"))?
        .to_owned();
    // Unit-payload variants would serialize without `payload`; store `{}` rather than SQL NULL
    // (the column is NOT NULL).
    let payload = tagged.get("payload").cloned().unwrap_or_else(|| serde_json::json!({}));
    Ok((event_type, payload))
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}
