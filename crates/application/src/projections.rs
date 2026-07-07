//! Read-model projections (ADR-0040): the write side appends business events to `domain_events`; a
//! projector folds each event into a materialized read-model row. This module hosts the hand-written glue
//! — the [`Envelope`] a projector receives — and re-exports the GENERATED row types (`generated::rows`)
//! and projector wiring (`generated::projectors`: a `<Table>Handlers` trait + a `project_<table>`
//! dispatch per table). Projection LOGIC is the hand-written `…Handlers` impl (tested app code), never in
//! generated code or SQL — see ADR-0040.

use domain::generated::events::DomainEvent;

/// One event as delivered to a projector: the typed business event plus the log metadata a fold needs.
/// The technical envelope lives in infrastructure (`domain_events`); this is its in-memory projection.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    /// The aggregate-instance stream this event belongs to (`domain_events.stream_name`).
    pub stream_name: String,
    /// Global total order / projection checkpoint (`domain_events.position`).
    pub position: i64,
    /// When the event occurred — the row-write time stamped onto `updated_at` by the dispatch.
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    /// The typed business event.
    pub event: DomainEvent,
}

pub use crate::generated::projectors::*;
pub use crate::generated::rows::*;

#[cfg(test)]
mod projector_dispatch_tests {
    //! Prove the GENERATED projector wiring is usable end-to-end: a hand-written `Handlers` impl folds
    //! events into rows, the dispatch routes each event to the right method, stamps `updated_at` from the
    //! envelope, and leaves the row untouched for events the table is not fed.
    use super::*;
    use domain::generated::events::{
        CartCheckedOut, CartLineAdded, CartLineQuantityChanged, CartLineRemoved, CartStarted,
        CustomerIdentified, DomainEvent, RestaurantAccountDeleted,
    };
    use domain::generated::scalars::{
        CartId, CartStatus, CurrencyCode, MoneyCents, RestaurantAccountId, RestaurantId,
    };

    const NIL: &str = "00000000-0000-0000-0000-000000000000";
    fn ts(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(secs, 0).unwrap()
    }
    fn sample_row() -> CartRow {
        CartRow {
            cart_id: CartId(NIL.parse().unwrap()),
            restaurant_id: RestaurantId(NIL.parse().unwrap()),
            customer_id: None,
            status: CartStatus::OPEN,
            lines: serde_json::json!([]),
            total_amount_cents: MoneyCents(0),
            currency: CurrencyCode("EUR".into()),
            estimated_breakdown: None,
            uber_comparison: None,
            created_at: ts(0), // placeholder — the dispatch manages the technical timestamps
            updated_at: ts(0),
        }
    }
    fn env(event: DomainEvent, at: i64) -> Envelope {
        Envelope { stream_name: "cart-1".into(), position: 1, occurred_at: ts(at), event }
    }

    // A minimal hand-written fold: build the row on CartStarted, pass through otherwise.
    struct Handlers;
    impl CartHandlers for Handlers {
        fn on_cart_started(&self, _s: Option<CartRow>, _e: &CartStarted, _env: &Envelope) -> Option<CartRow> {
            Some(sample_row())
        }
        fn on_cart_line_added(&self, s: Option<CartRow>, _e: &CartLineAdded, _env: &Envelope) -> Option<CartRow> { s }
        fn on_cart_line_quantity_changed(&self, s: Option<CartRow>, _e: &CartLineQuantityChanged, _env: &Envelope) -> Option<CartRow> { s }
        fn on_cart_line_removed(&self, s: Option<CartRow>, _e: &CartLineRemoved, _env: &Envelope) -> Option<CartRow> { s }
        fn on_cart_checked_out(&self, s: Option<CartRow>, _e: &CartCheckedOut, _env: &Envelope) -> Option<CartRow> { s }
        fn on_customer_identified(&self, s: Option<CartRow>, _e: &CustomerIdentified, _env: &Envelope) -> Option<CartRow> { s }
    }

    #[test]
    fn dispatch_routes_to_handler_and_stamps_updated_at() {
        let started = DomainEvent::CartStarted(CartStarted {
            cart_id: CartId(NIL.parse().unwrap()),
            restaurant_id: RestaurantId(NIL.parse().unwrap()),
            customer_id: None,
        });
        let out = project_cart(&Handlers, None, &env(started, 1_700_000_000)).unwrap();
        // routed to on_cart_started (row built); both technical timestamps stamped from the envelope
        // (first event → created_at = updated_at = occurred_at), not the placeholder.
        assert_eq!(out.updated_at, ts(1_700_000_000));
        assert_eq!(out.created_at, ts(1_700_000_000));
    }

    #[test]
    fn unrelated_event_passes_through_untouched() {
        // RestaurantAccountDeleted is not fed to Cart → the incoming row is returned as-is, NOT stamped.
        let unrelated = DomainEvent::RestaurantAccountDeleted(RestaurantAccountDeleted {
            restaurant_account_id: RestaurantAccountId(NIL.parse().unwrap()),
            reason: None,
        });
        let mut r = sample_row();
        r.updated_at = ts(42);
        let out = project_cart(&Handlers, Some(r), &env(unrelated, 1_700_000_000)).unwrap();
        assert_eq!(out.updated_at, ts(42)); // unchanged — the `_ => state` arm skips stamping
    }
}
