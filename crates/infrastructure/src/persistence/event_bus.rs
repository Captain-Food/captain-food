//! In-process domain-event bus: a `tokio::sync::broadcast` fan-out of events APPENDED to
//! `domain_events`, published by [`super::event_store::PgEventStore`] after (and only after) a
//! successful commit. GraphQL subscription resolvers subscribe to it to push read-side updates over
//! WebSockets without polling the log.
//!
//! Scope and guarantees:
//! - **Notification, not source of truth.** The envelope is deliberately lightweight (stream, type,
//!   correlation, position) — subscribers re-resolve current state from the read models, so a missed
//!   or lagged message degrades to "the next event catches you up", never to wrong data.
//! - **Best effort.** `publish` ignores send errors (no subscribers / all receivers dropped is a
//!   no-op); a slow subscriber past the channel capacity sees `Lagged` and simply re-resolves.
//! - **Single process.** The bus reaches only subscriptions served by THIS instance — fine for the
//!   V0 single-instance deployment (ADR-0042). Free-tier caveat: a WebSocket lives only while the
//!   app is warm (the uptimerobot ping keeps it so); a restart drops both bus and sockets, and
//!   clients resubscribe + re-sync via the pull queries.

use tokio::sync::broadcast;

/// The lightweight envelope broadcast for every event row appended to `domain_events`. Technical
/// metadata only (ADR-0041) — the business payload stays in the log and in the read models.
#[derive(Debug, Clone)]
pub struct AppendedEvent {
    /// The aggregate stream the event was appended to (e.g. `Order-<uuid>`).
    pub stream_name: String,
    /// The events.yaml event type (e.g. `OrderPlaced`).
    pub event_type: String,
    /// The command correlation stamped on the envelope — what subscription filters match on.
    pub correlation_id: uuid::Uuid,
    /// The event's version within its stream (its position in the aggregate's history).
    pub position: i64,
}

/// Cloneable handle over the broadcast channel: publishers (`PgEventStore`) and subscribers (the
/// GraphQL `SubscriptionRoot`, via schema `.data(...)`) share clones of the same bus.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppendedEvent>,
}

impl EventBus {
    /// A bus retaining up to `capacity` in-flight messages per subscriber before it lags.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Broadcast an appended-event envelope. Best effort: with no live subscribers (or a closed
    /// channel) this is a no-op — the append itself has already committed.
    pub fn publish(&self, event: AppendedEvent) {
        let _ = self.tx.send(event);
    }

    /// A fresh receiver seeing every envelope published from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<AppendedEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    /// Default capacity generously above any realistic V0 burst (envelopes are ~100 bytes).
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_without_subscribers_is_a_noop() {
        // No receiver exists — send() errs internally and publish must swallow it.
        EventBus::default().publish(AppendedEvent {
            stream_name: "Order-00000000-0000-0000-0000-000000000001".into(),
            event_type: "OrderPlaced".into(),
            correlation_id: uuid::Uuid::new_v4(),
            position: 1,
        });
    }

    #[tokio::test]
    async fn subscriber_receives_published_envelope() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let correlation = uuid::Uuid::new_v4();
        bus.publish(AppendedEvent {
            stream_name: "Order-x".into(),
            event_type: "OrderAccepted".into(),
            correlation_id: correlation,
            position: 2,
        });
        let got = rx.recv().await.expect("envelope");
        assert_eq!(got.stream_name, "Order-x");
        assert_eq!(got.event_type, "OrderAccepted");
        assert_eq!(got.correlation_id, correlation);
        assert_eq!(got.position, 2);
    }
}
