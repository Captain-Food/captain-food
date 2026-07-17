//! App-layer projection runtime (ADR-0040): the worker that folds `domain_events` into the materialized
//! read-model tables using the hand-written `…Compute` projectors, checkpointed in
//! `projection_checkpoint`. V0 covers the Restaurant slice.

pub mod worker;

pub use worker::ProjectionWorker;

/// Live health snapshot of the projection worker, exposed by the server's `/projector` endpoint.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ProjectionStatus {
    /// Whether the polling loop is running.
    pub running: bool,
    /// Last `domain_events.position` folded and committed to `projection_checkpoint`.
    pub checkpoint: i64,
    /// Highest `domain_events.position` seen at the last tick.
    pub head: i64,
    /// `head - checkpoint`: how many log positions the read model is behind.
    pub lag: i64,
    /// When the worker last completed a tick (successful or not).
    pub last_tick_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The last tick's error, cleared on the next successful tick.
    pub last_error: Option<String>,
}

impl Default for ProjectionStatus {
    fn default() -> Self {
        Self { running: false, checkpoint: 0, head: 0, lag: 0, last_tick_at: None, last_error: None }
    }
}
