//! BEHAVIOUR tests for the DeliveryJob aggregate (ADR-0031) — the executable form of the
//! `specs/tests.yaml` Given/When/Then cases whose `when` is a DeliveryJob command (ADR-0032: each test
//! cites the `specs/rules.yaml` rule it asserts). Given = pre-seeded stream events (in-memory event
//! store), When = the command handler, Then = the emitted event(s) / the errors.yaml rejection code.
//!
//! Pure and offline: an in-memory [`EventStore`]. `DeliveryRequested` in the GIVENs stands for the
//! DeliveryDispatchProcess outcome (that saga reacts to OrderMarkedReady and is a separate runtime leg).

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use application::commands::{
    accept_delivery, assign_delivery_to_partner, cancel_delivery, complete_delivery,
    confirm_pickup, decline_delivery, rejection_code, report_delivery_issue,
    resolve_delivery_issue, unassign_delivery_from_partner, update_delivery_partner_status,
    update_delivery_status,
};
use application::ports::{version_conflict, Actor, EventStore};
use domain::generated::commands::{
    AcceptDelivery, AssignDeliveryToPartner, CancelDelivery, CompleteDelivery, ConfirmPickup,
    DeclineDelivery, ReportDeliveryIssue, ResolveDeliveryIssue, UnassignDeliveryFromPartner,
    UpdateDeliveryPartnerStatus, UpdateDeliveryStatus,
};
use domain::generated::entities::{Address, Courier};
use domain::generated::events::{
    DeliveryAcceptedByPartner, DeliveryAcceptedByRider, DeliveryAssignedToPartner,
    DeliveryCancelled, DeliveryCompleted, DeliveryIssueReported, DeliveryPickedUp,
    DeliveryRequested, DomainEvent,
};
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

fn stream(id: DeliveryJobId) -> String {
    format!("DeliveryJob-{}", id.0)
}

fn address(line1: &str) -> Address {
    Address {
        line1: AddressLine(line1.into()),
        line2: None,
        postal_code: PostalCode("37000".into()),
        city: CityName("Tours".into()),
        country: CountryCode("FR".into()),
    }
}

/// Fixture `deliveryRequested` — the job is born PENDING.
fn requested(id: DeliveryJobId) -> DomainEvent {
    DomainEvent::DeliveryRequested(DeliveryRequested {
        mode: None,
        delivery_job_id: id,
        order_id: OrderId(uuid::Uuid::new_v4()),
        restaurant_id: RestaurantId(uuid::Uuid::new_v4()),
        pickup: address("1 Rue Nationale"),
        dropoff: address("9 Rue Colbert"),
        provider: None,
    })
}

/// Fixture `deliveryAcceptedByRider`.
fn accepted(id: DeliveryJobId, rider: RiderId) -> DomainEvent {
    DomainEvent::DeliveryAcceptedByRider(DeliveryAcceptedByRider { delivery_job_id: id, rider_id: rider })
}

/// Fixture `deliveryPickedUp`.
fn picked_up(id: DeliveryJobId, rider: RiderId) -> DomainEvent {
    DomainEvent::DeliveryPickedUp(DeliveryPickedUp { delivery_job_id: id, rider_id: rider, at: None })
}

/// Fixture `deliveryCompleted`.
fn completed(id: DeliveryJobId) -> DomainEvent {
    DomainEvent::DeliveryCompleted(DeliveryCompleted { delivery_job_id: id, at: None })
}

/// Fixture `deliveryCancelled`.
fn cancelled(id: DeliveryJobId) -> DomainEvent {
    DomainEvent::DeliveryCancelled(DeliveryCancelled { delivery_job_id: id, reason: None })
}

/// Fixture `deliveryAssignedToPartner`.
fn assigned_to_partner(id: DeliveryJobId, partner: &str) -> DomainEvent {
    DomainEvent::DeliveryAssignedToPartner(DeliveryAssignedToPartner {
        delivery_job_id: id,
        partner_ref: ExternalReference(partner.into()),
    })
}

/// Fixture `deliveryAcceptedByPartner`.
fn accepted_by_partner(id: DeliveryJobId, partner: &str) -> DomainEvent {
    DomainEvent::DeliveryAcceptedByPartner(DeliveryAcceptedByPartner {
        delivery_job_id: id,
        partner_ref: ExternalReference(partner.into()),
        courier: Courier {
            display_name: "Léa".into(),
            phone: Some(PhoneNumber("+33611223344".into())),
            rider_id: None,
        },
        estimated_pickup_at: None,
        estimated_dropoff_at: None,
    })
}

/// Fixture `deliveryIssueReported`.
fn issue_reported(id: DeliveryJobId, rider: RiderId) -> DomainEvent {
    DomainEvent::DeliveryIssueReported(DeliveryIssueReported {
        delivery_job_id: id,
        rider_id: Some(rider),
        issue: "Customer unreachable at the door".into(),
        reported_at: None,
    })
}

fn jid() -> DeliveryJobId {
    DeliveryJobId(uuid::Uuid::new_v4())
}
fn rider() -> RiderId {
    RiderId(uuid::Uuid::new_v4())
}

// ------------------------------------------------------------------------------------------------
// Acceptance (rules.yaml#/DeliveryAcceptedOnlyWhenPending)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestAcceptDelivery — rules.yaml#/DeliveryAcceptedOnlyWhenPending
#[tokio::test]
async fn an_independent_rider_accepts_a_pending_job() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job)]);

    accept_delivery(&store, AcceptDelivery { delivery_job_id: job, rider_id: r }, &actor())
        .await
        .expect("accept");

    let events = store.stream(&stream(job));
    assert!(matches!(&events[1], DomainEvent::DeliveryAcceptedByRider(e) if e.rider_id == r));
}

/// tests.yaml#/cases/TestAcceptDeliveryIsRejected (all three arms) —
/// rules.yaml#/DeliveryAcceptedOnlyWhenPending
#[tokio::test]
async fn rejects_accepting_a_missing_taken_or_cancelled_job() {
    let store = MemStore::default();

    // Missing job → DeliveryJobNotFound.
    let err = accept_delivery(&store, AcceptDelivery { delivery_job_id: jid(), rider_id: rider() }, &actor())
        .await
        .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));

    // Already taken by rider-1 → DeliveryAlreadyAssigned for rider-2.
    let (job, r1) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r1)]);
    let err = accept_delivery(&store, AcceptDelivery { delivery_job_id: job, rider_id: rider() }, &actor())
        .await
        .expect_err("already taken");
    assert_eq!(rejection_code(&err), Some("DeliveryAlreadyAssigned"));
    assert_eq!(store.stream(&stream(job)).len(), 2, "no event on rejection");

    // Cancelled job → InvalidDeliveryStatus (only a PENDING job can be accepted).
    let job = jid();
    store.seed(&stream(job), vec![requested(job), cancelled(job)]);
    let err = accept_delivery(&store, AcceptDelivery { delivery_job_id: job, rider_id: rider() }, &actor())
        .await
        .expect_err("cancelled");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
}

// ------------------------------------------------------------------------------------------------
// Pickup & completion (rules.yaml#/DeliveryPickupAndCompletionByRider)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestConfirmPickup — rules.yaml#/DeliveryPickupAndCompletionByRider
#[tokio::test]
async fn the_assigned_rider_confirms_pickup() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r)]);

    confirm_pickup(&store, ConfirmPickup { delivery_job_id: job, rider_id: r }, &actor())
        .await
        .expect("pickup");
    assert!(matches!(&store.stream(&stream(job))[2], DomainEvent::DeliveryPickedUp(e) if e.rider_id == r));

    // Another rider cannot confirm the pickup (must be ASSIGNED to this rider).
    let (job2, r2) = (jid(), rider());
    store.seed(&stream(job2), vec![requested(job2), accepted(job2, r2)]);
    let err = confirm_pickup(&store, ConfirmPickup { delivery_job_id: job2, rider_id: rider() }, &actor())
        .await
        .expect_err("wrong rider");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));

    // A PENDING (unassigned) job cannot be picked up.
    let job3 = jid();
    store.seed(&stream(job3), vec![requested(job3)]);
    let err = confirm_pickup(&store, ConfirmPickup { delivery_job_id: job3, rider_id: rider() }, &actor())
        .await
        .expect_err("not assigned");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
}

/// tests.yaml#/cases/TestCompleteDelivery — rules.yaml#/DeliveryPickupAndCompletionByRider
#[tokio::test]
async fn the_assigned_rider_records_hand_over() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r), picked_up(job, r)]);

    complete_delivery(&store, CompleteDelivery { delivery_job_id: job, rider_id: r }, &actor())
        .await
        .expect("complete");
    assert!(matches!(&store.stream(&stream(job))[3], DomainEvent::DeliveryCompleted(_)));

    // Completion before pickup is out of order (InvalidDeliveryStatus).
    let (job2, r2) = (jid(), rider());
    store.seed(&stream(job2), vec![requested(job2), accepted(job2, r2)]);
    let err = complete_delivery(&store, CompleteDelivery { delivery_job_id: job2, rider_id: r2 }, &actor())
        .await
        .expect_err("not picked up");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
}

// ------------------------------------------------------------------------------------------------
// Cancellation (rules.yaml#/DeliveryCancellableBeforeCompletion)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestCancelDelivery — rules.yaml#/DeliveryCancellableBeforeCompletion
#[tokio::test]
async fn the_restaurant_cancels_a_pending_job() {
    let store = MemStore::default();
    let job = jid();
    store.seed(&stream(job), vec![requested(job)]);

    cancel_delivery(
        &store,
        CancelDelivery { delivery_job_id: job, reason: Some("Restaurant closed".into()) },
        &actor(),
    )
    .await
    .expect("cancel");
    assert!(matches!(
        &store.stream(&stream(job))[1],
        DomainEvent::DeliveryCancelled(e) if e.reason.as_deref() == Some("Restaurant closed")
    ));

    // Re-cancelling an already-cancelled job is an idempotent no-op (the command ensures the state).
    cancel_delivery(&store, CancelDelivery { delivery_job_id: job, reason: None }, &actor())
        .await
        .expect("idempotent");
    assert_eq!(store.stream(&stream(job)).len(), 2, "no event emitted");
}

/// tests.yaml#/cases/TestCancelDeliveryIsRejected (both arms) —
/// rules.yaml#/DeliveryCancellableBeforeCompletion
#[tokio::test]
async fn rejects_cancelling_a_missing_or_delivered_job() {
    let store = MemStore::default();

    // Missing job → DeliveryJobNotFound.
    let err = cancel_delivery(&store, CancelDelivery { delivery_job_id: jid(), reason: None }, &actor())
        .await
        .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));

    // Delivered job → InvalidDeliveryStatus.
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r), picked_up(job, r), completed(job)]);
    let err = cancel_delivery(
        &store,
        CancelDelivery { delivery_job_id: job, reason: Some("Too late".into()) },
        &actor(),
    )
    .await
    .expect_err("delivered");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
    assert_eq!(store.stream(&stream(job)).len(), 4, "no event on rejection");
}

// ------------------------------------------------------------------------------------------------
// Decline (rules.yaml#/DeliveryDeclineKeepsJobPending)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestDeliveryDeclinedByRider — rules.yaml#/DeliveryDeclineKeepsJobPending
#[tokio::test]
async fn a_rider_declines_a_pending_job_and_it_stays_re_offerable() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job)]);

    decline_delivery(
        &store,
        DeclineDelivery { delivery_job_id: job, rider_id: r, reason: Some("Too far".into()) },
        &actor(),
    )
    .await
    .expect("decline");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[1],
        DomainEvent::DeliveryDeclinedByRider(e)
            if e.rider_id == r && e.reason.as_deref() == Some("Too far")
    ));
    // The decline folds to nothing: the job stays PENDING and unassigned, so ANOTHER rider can accept.
    accept_delivery(&store, AcceptDelivery { delivery_job_id: job, rider_id: rider() }, &actor())
        .await
        .expect("still re-offerable");

    // A job already taken rejects the decline with DeliveryAlreadyAssigned.
    let (job2, r2) = (jid(), rider());
    store.seed(&stream(job2), vec![requested(job2), accepted(job2, r2)]);
    let err = decline_delivery(
        &store,
        DeclineDelivery { delivery_job_id: job2, rider_id: rider(), reason: None },
        &actor(),
    )
    .await
    .expect_err("already taken");
    assert_eq!(rejection_code(&err), Some("DeliveryAlreadyAssigned"));

    // A missing job rejects with DeliveryJobNotFound; a cancelled one with InvalidDeliveryStatus.
    let err = decline_delivery(
        &store,
        DeclineDelivery { delivery_job_id: jid(), rider_id: rider(), reason: None },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
    let job3 = jid();
    store.seed(&stream(job3), vec![requested(job3), cancelled(job3)]);
    let err = decline_delivery(
        &store,
        DeclineDelivery { delivery_job_id: job3, rider_id: rider(), reason: None },
        &actor(),
    )
    .await
    .expect_err("cancelled");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
}

// ------------------------------------------------------------------------------------------------
// Issues (rules.yaml#/DeliveryIssueLifecycle)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestDeliveryIssueReported — rules.yaml#/DeliveryIssueLifecycle
#[tokio::test]
async fn an_issue_is_reported_on_a_non_delivered_job() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r)]);

    report_delivery_issue(
        &store,
        ReportDeliveryIssue {
            delivery_job_id: job,
            rider_id: Some(r),
            issue: "Customer unreachable at the door".into(),
        },
        &actor(),
    )
    .await
    .expect("report");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[2],
        DomainEvent::DeliveryIssueReported(e)
            if e.rider_id == Some(r)
                && e.issue == "Customer unreachable at the door"
                && e.reported_at.is_some() // stamped server-side by the handler
    ));

    // A DELIVERED job cannot report an issue; a missing one is DeliveryJobNotFound.
    let (job2, r2) = (jid(), rider());
    store.seed(
        &stream(job2),
        vec![requested(job2), accepted(job2, r2), picked_up(job2, r2), completed(job2)],
    );
    let err = report_delivery_issue(
        &store,
        ReportDeliveryIssue { delivery_job_id: job2, rider_id: Some(r2), issue: "late".into() },
        &actor(),
    )
    .await
    .expect_err("delivered");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
    let err = report_delivery_issue(
        &store,
        ReportDeliveryIssue { delivery_job_id: jid(), rider_id: None, issue: "?".into() },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
}

/// tests.yaml#/cases/TestDeliveryIssueResolved — rules.yaml#/DeliveryIssueLifecycle
#[tokio::test]
async fn a_previously_reported_issue_is_resolved() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r), issue_reported(job, r)]);

    resolve_delivery_issue(
        &store,
        ResolveDeliveryIssue {
            delivery_job_id: job,
            resolution: "Customer called back; order handed over".into(),
        },
        &actor(),
    )
    .await
    .expect("resolve");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[3],
        DomainEvent::DeliveryIssueResolved(e)
            if e.resolution == "Customer called back; order handed over"
                && e.resolved_at.is_some() // stamped server-side by the handler
    ));

    // No OPEN issue (just resolved) → InvalidDeliveryStatus.
    let err = resolve_delivery_issue(
        &store,
        ResolveDeliveryIssue { delivery_job_id: job, resolution: "again".into() },
        &actor(),
    )
    .await
    .expect_err("nothing open");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));

    // A missing job rejects with DeliveryJobNotFound.
    let err = resolve_delivery_issue(
        &store,
        ResolveDeliveryIssue { delivery_job_id: jid(), resolution: "?".into() },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
}

// ------------------------------------------------------------------------------------------------
// Status machine by command (rules.yaml#/DeliveryPickupAndCompletionByRider)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestDeliveryStatusUpdatedByCommand —
/// rules.yaml#/DeliveryPickupAndCompletionByRider
#[tokio::test]
async fn the_delivery_status_is_driven_through_valid_transitions_to_delivered() {
    let store = MemStore::default();
    let (job, r) = (jid(), rider());
    store.seed(&stream(job), vec![requested(job), accepted(job, r), picked_up(job, r)]);

    update_delivery_status(
        &store,
        UpdateDeliveryStatus { delivery_job_id: job, status: DeliveryStatus::DELIVERED },
        &actor(),
    )
    .await
    .expect("valid transition");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[3],
        DomainEvent::DeliveryStatusUpdated(e) if e.status == DeliveryStatus::DELIVERED
    ));

    // An invalid jump (PENDING → DELIVERED) rejects; a missing job is DeliveryJobNotFound.
    let job2 = jid();
    store.seed(&stream(job2), vec![requested(job2)]);
    let err = update_delivery_status(
        &store,
        UpdateDeliveryStatus { delivery_job_id: job2, status: DeliveryStatus::DELIVERED },
        &actor(),
    )
    .await
    .expect_err("invalid jump");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
    let err = update_delivery_status(
        &store,
        UpdateDeliveryStatus { delivery_job_id: jid(), status: DeliveryStatus::ASSIGNED },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
}

// ------------------------------------------------------------------------------------------------
// Partner assignment (rules.yaml#/DeliveryPartnerAssignmentLifecycle)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestDeliveryAssignedToPartner —
/// rules.yaml#/DeliveryPartnerAssignmentLifecycle
#[tokio::test]
async fn a_pending_job_is_assigned_to_a_delivery_partner() {
    let store = MemStore::default();
    let job = jid();
    store.seed(&stream(job), vec![requested(job)]);

    assign_delivery_to_partner(
        &store,
        AssignDeliveryToPartner {
            delivery_job_id: job,
            partner_ref: ExternalReference("avelo-77".into()),
        },
        &actor(),
    )
    .await
    .expect("assign");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[1],
        DomainEvent::DeliveryAssignedToPartner(e)
            if e.partner_ref == ExternalReference("avelo-77".into())
    ));

    // A job already taken (this very assignment) rejects a second assignment.
    let err = assign_delivery_to_partner(
        &store,
        AssignDeliveryToPartner {
            delivery_job_id: job,
            partner_ref: ExternalReference("other".into()),
        },
        &actor(),
    )
    .await
    .expect_err("already assigned");
    assert_eq!(rejection_code(&err), Some("DeliveryAlreadyAssigned"));

    // A missing job rejects with DeliveryJobNotFound; a cancelled one with InvalidDeliveryStatus.
    let err = assign_delivery_to_partner(
        &store,
        AssignDeliveryToPartner {
            delivery_job_id: jid(),
            partner_ref: ExternalReference("avelo-77".into()),
        },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
    let job2 = jid();
    store.seed(&stream(job2), vec![requested(job2), cancelled(job2)]);
    let err = assign_delivery_to_partner(
        &store,
        AssignDeliveryToPartner {
            delivery_job_id: job2,
            partner_ref: ExternalReference("avelo-77".into()),
        },
        &actor(),
    )
    .await
    .expect_err("cancelled");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
}

/// tests.yaml#/cases/TestDeliveryUnassignedFromPartner —
/// rules.yaml#/DeliveryPartnerAssignmentLifecycle
#[tokio::test]
async fn an_assigned_job_is_unassigned_from_its_partner_to_be_re_offered() {
    let store = MemStore::default();
    let job = jid();
    store.seed(&stream(job), vec![requested(job), assigned_to_partner(job, "avelo-77")]);

    unassign_delivery_from_partner(
        &store,
        UnassignDeliveryFromPartner {
            delivery_job_id: job,
            reason: Some("Re-offering to another channel".into()),
        },
        &actor(),
    )
    .await
    .expect("unassign");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[2],
        DomainEvent::DeliveryUnassignedFromPartner(e)
            if e.reason.as_deref() == Some("Re-offering to another channel")
    ));
    // The fold returns the job to PENDING: it is re-offerable (a rider can now accept it).
    accept_delivery(&store, AcceptDelivery { delivery_job_id: job, rider_id: rider() }, &actor())
        .await
        .expect("re-offerable");

    // A PENDING (never-assigned) job rejects; so does a RIDER-assigned job (no partner to unassign).
    let job2 = jid();
    store.seed(&stream(job2), vec![requested(job2)]);
    let err = unassign_delivery_from_partner(
        &store,
        UnassignDeliveryFromPartner { delivery_job_id: job2, reason: None },
        &actor(),
    )
    .await
    .expect_err("pending");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
    let (job3, r3) = (jid(), rider());
    store.seed(&stream(job3), vec![requested(job3), accepted(job3, r3)]);
    let err = unassign_delivery_from_partner(
        &store,
        UnassignDeliveryFromPartner { delivery_job_id: job3, reason: None },
        &actor(),
    )
    .await
    .expect_err("rider-assigned");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));

    // A missing job rejects with DeliveryJobNotFound.
    let err = unassign_delivery_from_partner(
        &store,
        UnassignDeliveryFromPartner { delivery_job_id: jid(), reason: None },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
}

/// tests.yaml#/cases/TestDeliveryPartnerStatusUpdated —
/// rules.yaml#/DeliveryPartnerAssignmentLifecycle
#[tokio::test]
async fn a_partner_reported_status_change_applies_as_a_valid_transition() {
    let store = MemStore::default();
    let job = jid();
    store.seed(&stream(job), vec![requested(job), accepted_by_partner(job, "avelo-77")]);

    update_delivery_partner_status(
        &store,
        UpdateDeliveryPartnerStatus {
            delivery_job_id: job,
            partner_ref: Some(ExternalReference("avelo-77".into())),
            status: DeliveryStatus::PICKED_UP,
        },
        &actor(),
    )
    .await
    .expect("valid partner transition");

    let events = store.stream(&stream(job));
    assert!(matches!(
        &events[2],
        DomainEvent::DeliveryPartnerStatusUpdated(e)
            if e.status == DeliveryStatus::PICKED_UP
                && e.partner_ref == Some(ExternalReference("avelo-77".into()))
    ));

    // An invalid transition (ASSIGNED → DELIVERED skipping pickup) rejects; a missing job too.
    let job2 = jid();
    store.seed(&stream(job2), vec![requested(job2), accepted_by_partner(job2, "avelo-77")]);
    let err = update_delivery_partner_status(
        &store,
        UpdateDeliveryPartnerStatus {
            delivery_job_id: job2,
            partner_ref: Some(ExternalReference("avelo-77".into())),
            status: DeliveryStatus::DELIVERED,
        },
        &actor(),
    )
    .await
    .expect_err("invalid transition");
    assert_eq!(rejection_code(&err), Some("InvalidDeliveryStatus"));
    let err = update_delivery_partner_status(
        &store,
        UpdateDeliveryPartnerStatus {
            delivery_job_id: jid(),
            partner_ref: None,
            status: DeliveryStatus::PICKED_UP,
        },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("DeliveryJobNotFound"));
}
