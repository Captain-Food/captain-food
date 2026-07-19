//! BEHAVIOUR tests for the Rider aggregate — the executable form of the `specs/tests.yaml`
//! Given/When/Then cases whose `when` is a Rider command (ADR-0032: each test cites the
//! `specs/rules.yaml` rule it asserts). Given = pre-seeded stream events (in-memory event store),
//! When = the command handler, Then = the emitted event(s) / the errors.yaml rejection code.
//!
//! Pure and offline: an in-memory [`EventStore`] only — the Rider fold and the availability machine
//! (`domain::rider::can_transition`) are the whole decision surface.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use application::commands::{change_rider_status, register_rider, rejection_code, update_rider_info};
use application::ports::{version_conflict, Actor, EventStore};
use domain::generated::commands::{ChangeRiderStatus, RegisterRider, UpdateRiderInfo};
use domain::generated::events::{DomainEvent, RiderRegistered};
use domain::generated::scalars::*;
use domain::shared::errors::DomainError;

// ------------------------------------------------------------------------------------------------
// Test doubles
// ------------------------------------------------------------------------------------------------

/// In-memory [`EventStore`]: version = number of events on the stream, same optimistic-concurrency
/// semantics as `PgEventStore` (a clash → the canonical `version_conflict`).
#[derive(Default)]
struct MemStore {
    streams: Mutex<HashMap<String, Vec<DomainEvent>>>,
}

impl MemStore {
    /// GIVEN: pre-seed a stream with already-recorded facts.
    fn seed(&self, stream: &str, events: Vec<DomainEvent>) {
        self.streams.lock().unwrap().insert(stream.to_string(), events);
    }

    /// THEN: the full stream after the command ran.
    fn stream(&self, stream: &str) -> Vec<DomainEvent> {
        self.streams.lock().unwrap().get(stream).cloned().unwrap_or_default()
    }
}

#[async_trait]
impl EventStore for MemStore {
    async fn append(
        &self,
        stream_name: &str,
        expected_version: i64,
        events: &[DomainEvent],
        _actor: &Actor,
    ) -> Result<i64, DomainError> {
        let mut streams = self.streams.lock().unwrap();
        let stream = streams.entry(stream_name.to_string()).or_default();
        if stream.len() as i64 != expected_version {
            return Err(version_conflict(stream_name, expected_version));
        }
        stream.extend(events.iter().cloned());
        Ok(stream.len() as i64)
    }

    async fn load(&self, stream_name: &str) -> Result<(Vec<DomainEvent>, i64), DomainError> {
        let events = self.stream(stream_name);
        let version = events.len() as i64;
        Ok((events, version))
    }
}

// ------------------------------------------------------------------------------------------------
// Fixtures (tests.yaml `fixtures`, with UUIDs instead of the sample string ids)
// ------------------------------------------------------------------------------------------------

fn actor() -> Actor {
    Actor {
        user_id: uuid::Uuid::new_v4(),
        user_type: 4, // UserType::RIDER ordinal
        correlation_id: uuid::Uuid::new_v4(),
        cause_id: None,
    }
}

fn stream(id: RiderId) -> String {
    format!("Rider-{}", id.0)
}

fn rid() -> RiderId {
    RiderId(uuid::Uuid::new_v4())
}

/// Fixture `riderRegistered` — the rider is born OFFLINE.
fn registered(id: RiderId) -> DomainEvent {
    DomainEvent::RiderRegistered(RiderRegistered {
        rider_id: id,
        auth_ref: ExternalReference("auth-supabase-9".into()),
        display_name: "Léa".into(),
        phone: PhoneNumber("+33611223344".into()),
        status: RiderStatus::OFFLINE,
    })
}

/// The `RegisterRider` fixture command (tests.yaml TestRiderRegistered data).
fn register_cmd(id: RiderId) -> RegisterRider {
    RegisterRider {
        rider_id: id,
        auth_ref: ExternalReference("auth-supabase-9".into()),
        display_name: "Léa".into(),
        phone: PhoneNumber("+33611223344".into()),
    }
}

// ------------------------------------------------------------------------------------------------
// Registration (rules.yaml#/RiderLifecycle)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRiderRegistered — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn a_rider_registers_linked_to_the_auth_provider_user() {
    let store = MemStore::default();
    let rider = rid();

    register_rider(&store, register_cmd(rider), &actor()).await.expect("register");

    let events = store.stream(&stream(rider));
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        DomainEvent::RiderRegistered(e)
            if e.rider_id == rider
                && e.auth_ref == ExternalReference("auth-supabase-9".into())
                && e.display_name == "Léa"
                && e.phone == PhoneNumber("+33611223344".into())
                && e.status == RiderStatus::OFFLINE // born OFFLINE; goes AVAILABLE explicitly
    ));
}

/// tests.yaml#/cases/TestRiderRegisterAgainIsRejected — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn registering_the_same_rider_twice_is_rejected() {
    let store = MemStore::default();
    let rider = rid();
    store.seed(&stream(rider), vec![registered(rider)]);

    let err = register_rider(&store, register_cmd(rider), &actor()).await.expect_err("re-register");
    assert_eq!(rejection_code(&err), Some("RiderAlreadyRegistered"));
    assert_eq!(store.stream(&stream(rider)).len(), 1, "no event on rejection");
}

// ------------------------------------------------------------------------------------------------
// Profile updates (rules.yaml#/RiderLifecycle)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRiderInfoUpdated — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn a_rider_updates_editable_profile_fields() {
    let store = MemStore::default();
    let rider = rid();
    store.seed(&stream(rider), vec![registered(rider)]);

    update_rider_info(
        &store,
        UpdateRiderInfo { rider_id: rider, display_name: Some("Léa B.".into()), phone: None },
        &actor(),
    )
    .await
    .expect("update");

    let events = store.stream(&stream(rider));
    assert!(matches!(
        &events[1],
        DomainEvent::RiderInfoUpdated(e)
            if e.display_name.as_deref() == Some("Léa B.") && e.phone.is_none()
    ));

    // An update carrying nothing editable is rejected (declared throw NoEditableFieldProvided).
    let err = update_rider_info(
        &store,
        UpdateRiderInfo { rider_id: rider, display_name: None, phone: None },
        &actor(),
    )
    .await
    .expect_err("nothing editable");
    assert_eq!(rejection_code(&err), Some("NoEditableFieldProvided"));
    assert_eq!(store.stream(&stream(rider)).len(), 2, "no event on rejection");
}

/// tests.yaml#/cases/TestRiderUpdateIsRejectedWhenNotFound — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn updating_an_unknown_rider_is_rejected() {
    let store = MemStore::default();

    let err = update_rider_info(
        &store,
        UpdateRiderInfo { rider_id: rid(), display_name: Some("Nobody".into()), phone: None },
        &actor(),
    )
    .await
    .expect_err("missing rider");
    assert_eq!(rejection_code(&err), Some("RiderNotFound"));
}

// ------------------------------------------------------------------------------------------------
// Availability machine (rules.yaml#/RiderLifecycle)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRiderStatusChanged — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn a_rider_goes_available_from_offline() {
    let store = MemStore::default();
    let rider = rid();
    store.seed(&stream(rider), vec![registered(rider)]);

    change_rider_status(
        &store,
        ChangeRiderStatus { rider_id: rider, status: RiderStatus::AVAILABLE },
        &actor(),
    )
    .await
    .expect("go available");

    let events = store.stream(&stream(rider));
    assert!(matches!(
        &events[1],
        DomainEvent::RiderStatusChanged(e) if e.status == RiderStatus::AVAILABLE
    ));
}

/// tests.yaml#/cases/TestRiderStatusChangeIsRejected — rules.yaml#/RiderLifecycle
#[tokio::test]
async fn an_offline_rider_cannot_jump_straight_to_on_delivery() {
    let store = MemStore::default();
    let rider = rid();
    store.seed(&stream(rider), vec![registered(rider)]);

    let err = change_rider_status(
        &store,
        ChangeRiderStatus { rider_id: rider, status: RiderStatus::ON_DELIVERY },
        &actor(),
    )
    .await
    .expect_err("invalid transition");
    assert_eq!(rejection_code(&err), Some("InvalidRiderStatusTransition"));
    assert_eq!(store.stream(&stream(rider)).len(), 1, "no event on rejection");

    // ChangeRiderStatus on an unknown rider → RiderNotFound (the other declared throw).
    let err = change_rider_status(
        &store,
        ChangeRiderStatus { rider_id: rid(), status: RiderStatus::AVAILABLE },
        &actor(),
    )
    .await
    .expect_err("missing rider");
    assert_eq!(rejection_code(&err), Some("RiderNotFound"));
}
