//! Rider aggregate — the PURE write-side state fold (ADR-0035), mirroring `customer.rs`. A rider
//! identity linked to the auth provider user (`specs/actors.yaml#/Rider`); the fold tracks just what
//! the declared invariants read: existence (`RiderNotFound`), profile fields, and the availability
//! machine guarded by [`can_transition`] (`errors.yaml#/InvalidRiderStatusTransition`). No I/O, no
//! serialization logic (dependency rule).

use crate::generated::events::DomainEvent;
use crate::generated::scalars::{PhoneNumber, RiderStatus};

/// What the Rider command handlers need to know to accept or reject a command. `None` (from
/// [`fold`]) means no `RiderRegistered` yet on this stream.
#[derive(Debug, Clone, PartialEq)]
pub struct RiderState {
    /// Availability/lifecycle machine (OFFLINE/AVAILABLE/ON_DELIVERY/SUSPENDED) — see [`can_transition`].
    pub status: RiderStatus,
    /// Current display name (profile field, edited via UpdateRiderInfo).
    pub display_name: String,
    /// Current canonical E.164 phone (profile field, edited via UpdateRiderInfo).
    pub phone: PhoneNumber,
}

/// Whether `from` → `to` is a legal rider status transition (`ChangeRiderStatus`). SUSPENDED is
/// admin-imposed from anywhere and terminal until reinstated (back to OFFLINE only); a delivery can
/// only be entered from AVAILABLE — notably OFFLINE → ON_DELIVERY is invalid.
pub fn can_transition(from: RiderStatus, to: RiderStatus) -> bool {
    use RiderStatus::*;
    matches!(
        (from, to),
        (_, SUSPENDED)
            | (OFFLINE, AVAILABLE)
            | (AVAILABLE, OFFLINE)
            | (AVAILABLE, ON_DELIVERY)
            | (ON_DELIVERY, AVAILABLE)
            | (SUSPENDED, OFFLINE)
    )
}

/// Fold a Rider stream (events in version order) into its current state. `None` ⇔ the stream has no
/// `RiderRegistered` yet, i.e. the rider does not exist.
pub fn fold(events: &[DomainEvent]) -> Option<RiderState> {
    events.iter().fold(None, apply)
}

/// Apply one event — a pure transition, total over the whole event union.
fn apply(state: Option<RiderState>, event: &DomainEvent) -> Option<RiderState> {
    if let DomainEvent::RiderRegistered(e) = event {
        return Some(RiderState {
            status: e.status,
            display_name: e.display_name.clone(),
            phone: e.phone.clone(),
        });
    }
    let mut s = state?;
    match event {
        DomainEvent::RiderInfoUpdated(e) => {
            // Partial update: only the provided profile fields change; the status never does.
            if let Some(name) = &e.display_name {
                s.display_name = name.clone();
            }
            if let Some(phone) = &e.phone {
                s.phone = phone.clone();
            }
        }
        DomainEvent::RiderStatusChanged(e) => s.status = e.status,
        _ => {}
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::Aggregate;
    use crate::generated::events::{RiderInfoUpdated, RiderRegistered, RiderStatusChanged};
    use crate::generated::scalars::{ExternalReference, RiderId};

    fn registered(status: RiderStatus) -> DomainEvent {
        DomainEvent::RiderRegistered(RiderRegistered {
            rider_id: RiderId(uuid::Uuid::nil()),
            auth_ref: ExternalReference("auth_1".into()),
            display_name: "Sam".into(),
            phone: PhoneNumber("+33600000000".into()),
            status,
        })
    }
    fn status_changed(status: RiderStatus) -> DomainEvent {
        DomainEvent::RiderStatusChanged(RiderStatusChanged {
            rider_id: RiderId(uuid::Uuid::nil()),
            status,
        })
    }

    #[test]
    fn no_registration_means_no_rider() {
        assert_eq!(fold(&[]), None);
        assert_eq!(fold(&[status_changed(RiderStatus::AVAILABLE)]), None);
    }

    #[test]
    fn registration_births_the_rider_with_the_event_status() {
        let s = fold(&[registered(RiderStatus::OFFLINE)]).unwrap();
        assert_eq!(s.status, RiderStatus::OFFLINE);
        assert_eq!(s.display_name, "Sam");
        assert_eq!(s.phone, PhoneNumber("+33600000000".into()));
    }

    #[test]
    fn info_update_is_partial_and_never_touches_the_status() {
        let update = DomainEvent::RiderInfoUpdated(RiderInfoUpdated {
            rider_id: RiderId(uuid::Uuid::nil()),
            display_name: Some("Sam R.".into()),
            phone: None, // omitted → keeps the current phone
        });
        let s = fold(&[registered(RiderStatus::AVAILABLE), update]).unwrap();
        assert_eq!(s.display_name, "Sam R.");
        assert_eq!(s.phone, PhoneNumber("+33600000000".into()));
        assert_eq!(s.status, RiderStatus::AVAILABLE);
    }

    #[test]
    fn status_changes_fold_in_order() {
        let s = fold(&[
            registered(RiderStatus::OFFLINE),
            status_changed(RiderStatus::AVAILABLE),
            status_changed(RiderStatus::ON_DELIVERY),
        ])
        .unwrap();
        assert_eq!(s.status, RiderStatus::ON_DELIVERY);
    }

    #[test]
    fn transition_table_matches_the_spec() {
        use RiderStatus::*;
        // The legal moves.
        assert!(can_transition(OFFLINE, AVAILABLE));
        assert!(can_transition(AVAILABLE, OFFLINE));
        assert!(can_transition(AVAILABLE, ON_DELIVERY));
        assert!(can_transition(ON_DELIVERY, AVAILABLE));
        assert!(can_transition(SUSPENDED, OFFLINE)); // reinstate
        // Suspension is admin-imposed from anywhere.
        assert!(can_transition(OFFLINE, SUSPENDED));
        assert!(can_transition(AVAILABLE, SUSPENDED));
        assert!(can_transition(ON_DELIVERY, SUSPENDED));
        // The notable invalid jumps.
        assert!(!can_transition(OFFLINE, ON_DELIVERY));
        assert!(!can_transition(ON_DELIVERY, OFFLINE));
        assert!(!can_transition(SUSPENDED, AVAILABLE));
        assert!(!can_transition(SUSPENDED, ON_DELIVERY));
        assert!(!can_transition(OFFLINE, OFFLINE));
        assert!(!can_transition(AVAILABLE, AVAILABLE));
    }

    #[test]
    fn stream_name_matches_the_aggregate_format() {
        let id = uuid::Uuid::nil();
        assert_eq!(RiderState::stream(RiderId(id)), format!("Rider-{id}"));
    }
}
