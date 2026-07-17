//! The Restaurant projection worker (ADR-0040): polls `domain_events` past the `'Restaurant'`
//! checkpoint, folds each Restaurant-stream event through the generated `project_restaurant` dispatch +
//! the hand-written `RestaurantProjector` compute hooks, upserts the `restaurant` row, and advances
//! `projection_checkpoint`. Idempotent on restart: replaying an event over the current row state is a
//! deterministic fold (`*Updated` events carry replace semantics).
//!
//! Scope note: only `Restaurant-%` streams are folded, so the cross-stream `default_currency` hole
//! (owning account's currency, set on the account stream) stays preserved by `RestaurantProjector` —
//! exactly the documented TODO(runtime) in `application::projectors::restaurant`.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use application::projections::{project_restaurant, Envelope};
use application::projectors::restaurant::RestaurantProjector;
use chrono::Utc;
use domain::generated::events::DomainEvent;
use domain::generated::scalars::RestaurantId;
use domain::shared::errors::DomainError;
use sqlx::{PgPool, Row};

use crate::persistence::{db_err, restaurant_store};
use crate::projection::ProjectionStatus;

const PROJECTOR: &str = "Restaurant";
const STREAM_PREFIX: &str = "Restaurant-";
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

pub struct ProjectionWorker {
    pool: PgPool,
    status: Arc<Mutex<ProjectionStatus>>,
}

impl ProjectionWorker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, status: Arc::new(Mutex::new(ProjectionStatus::default())) }
    }

    /// Shared status handle — the server reads this for its `/projector` health endpoint.
    pub fn status(&self) -> Arc<Mutex<ProjectionStatus>> {
        Arc::clone(&self.status)
    }

    fn status_mut(&self) -> MutexGuard<'_, ProjectionStatus> {
        // A poisoned lock only means a reader panicked mid-inspection; the snapshot stays usable.
        self.status.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drain all pending Restaurant-stream events once, updating the checkpoint and the status snapshot.
    pub async fn run_once(&self) -> Result<(), DomainError> {
        let outcome = self.tick().await;
        let mut st = self.status_mut();
        st.last_tick_at = Some(Utc::now());
        match &outcome {
            Ok((checkpoint, head)) => {
                st.checkpoint = *checkpoint;
                st.head = *head;
                st.lag = (*head - *checkpoint).max(0);
                st.last_error = None;
            }
            Err(e) => st.last_error = Some(e.to_string()),
        }
        outcome.map(|_| ())
    }

    /// Poll forever: `run_once` then sleep ~1.5s. Consumes the worker (spawn it as a task); the shared
    /// [`ProjectionStatus`] handle stays readable through [`Self::status`] clones taken before spawning.
    pub async fn run_loop(self) {
        self.status_mut().running = true;
        loop {
            // Errors are recorded on the status snapshot by run_once; the loop keeps polling.
            let _ = self.run_once().await;
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// One drain pass. Returns `(checkpoint, head)` after the pass.
    async fn tick(&self) -> Result<(i64, i64), DomainError> {
        let mut checkpoint: i64 =
            sqlx::query_scalar("SELECT position FROM projection_checkpoint WHERE projector = $1")
                .bind(PROJECTOR)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?
                .unwrap_or(0);
        let head: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) FROM domain_events")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;

        let pending = sqlx::query(
            "SELECT position, stream_name, event_type, payload, occurred_at FROM domain_events \
             WHERE position > $1 AND stream_name LIKE $2 ORDER BY position",
        )
        .bind(checkpoint)
        .bind(format!("{STREAM_PREFIX}%"))
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        for record in pending {
            let position: i64 = record.try_get("position").map_err(db_err)?;
            let stream_name: String = record.try_get("stream_name").map_err(db_err)?;
            let event_type: String = record.try_get("event_type").map_err(db_err)?;
            let payload: serde_json::Value = record.try_get("payload").map_err(db_err)?;
            let occurred_at: chrono::DateTime<Utc> = record.try_get("occurred_at").map_err(db_err)?;

            // Rebuild the typed event from the (event_type, payload) columns via the adjacent tag.
            let event: DomainEvent = serde_json::from_value(serde_json::json!({
                "eventType": event_type,
                "payload": payload,
            }))
            .map_err(|e| db_err(format!("position {position} ({event_type}): {e}")))?;

            let restaurant_id = restaurant_id_of(&stream_name, &event)?;
            let state = restaurant_store::load(&self.pool, restaurant_id).await?;
            let env = Envelope { stream_name, position, occurred_at, event };
            if let Some(next) = project_restaurant(&RestaurantProjector, state, &env) {
                restaurant_store::upsert(&self.pool, &next).await?;
            }

            self.commit_checkpoint(position).await?;
            checkpoint = position;
            self.status_mut().checkpoint = position;
        }

        // The checkpoint only advances on folded events, so foreign-stream positions between checkpoint
        // and head re-scan as no-ops next tick (cheap: the LIKE filter excludes them) and `lag` counts
        // GLOBAL log positions, not just Restaurant ones — a deliberate, simple V0 high-water mark.
        Ok((checkpoint, head.max(checkpoint)))
    }

    async fn commit_checkpoint(&self, position: i64) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO projection_checkpoint (projector, position, updated_at) VALUES ($1, $2, now()) \
             ON CONFLICT (projector) DO UPDATE SET position = EXCLUDED.position, updated_at = now()",
        )
        .bind(PROJECTOR)
        .bind(position)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}

/// The aggregate id an event belongs to: parsed from the `Restaurant-<uuid>` stream name, falling back
/// to the payload's `restaurantId` (every Restaurant-stream event carries it).
fn restaurant_id_of(stream_name: &str, event: &DomainEvent) -> Result<RestaurantId, DomainError> {
    if let Some(suffix) = stream_name.strip_prefix(STREAM_PREFIX) {
        if let Ok(id) = uuid::Uuid::parse_str(suffix) {
            return Ok(RestaurantId(id));
        }
    }
    serde_json::to_value(event)
        .ok()
        .and_then(|v| {
            v.get("payload")
                .and_then(|p| p.get("restaurantId"))
                .and_then(|id| id.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
        })
        .map(RestaurantId)
        .ok_or_else(|| {
            DomainError::Repository(format!("cannot resolve restaurant id for stream {stream_name}"))
        })
}
