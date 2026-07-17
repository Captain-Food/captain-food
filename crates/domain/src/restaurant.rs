//! Restaurant aggregate — the PURE write-side state fold (ADR-0035). Command handlers rehydrate a
//! [`RestaurantState`] by folding the stream's events (loaded through the `EventStore` port) and then
//! enforce the invariants declared in `specs/actors.yaml`/`specs/errors.yaml` against it. Deliberately
//! MINIMAL: only the fields those invariants read are folded — the full read model lives in the
//! `Restaurant` projection (ADR-0040), not here. No I/O, no serialization logic (dependency rule).
//!
//! The lifecycle mapping mirrors the read-side `RestaurantProjector` so write-side decisions and the
//! projected `status` column can never disagree: `RestaurantRegistered` → DRAFT, `RestaurantActivated`
//! → ACTIVE, `RestaurantDeactivated`/`RestaurantRemoved`/`RestaurantMarkedClosed` → INACTIVE.

use crate::generated::events::DomainEvent;
use crate::generated::scalars::{
    ExternalReference, OrderAcceptanceMode, RestaurantDisplayName, RestaurantListingStatus,
    RestaurantStatus, Slug, WebUrl,
};

/// What the Restaurant command handlers need to know about the aggregate to accept or reject a
/// command. `None` (from [`fold`]) means the aggregate does not exist → `RestaurantNotFound`.
#[derive(Debug, Clone, PartialEq)]
pub struct RestaurantState {
    /// Operational lifecycle (DRAFT → ACTIVE ⇄ INACTIVE) — `RestaurantNotActive`, activate/deactivate
    /// idempotency.
    pub status: RestaurantStatus,
    /// Live acceptance mode (NORMAL until changed) — `AcceptanceModeUnchanged`.
    pub order_acceptance: OrderAcceptanceMode,
    /// Partnership funnel position (NON_PARTNER → PASSIVE_PARTNER → ACTIVE_PARTNER).
    pub listing_status: RestaurantListingStatus,
    /// Whether an owner already claimed this listing — `ListingAlreadyClaimed`.
    pub listing_claimed: bool,
    /// The configured GBP 'Order online' link, if any — `GbpOrderLinkNotConfigured` (ADR-0021).
    pub gbp_order_url: Option<WebUrl>,
    /// Current slug (registration value; identity of the storefront host).
    pub slug: Slug,
    /// Display name, carried into rejection contexts (errors.yaml `restaurantName`).
    pub display_name: RestaurantDisplayName,
    /// External idempotent import key, when seeded from an external source.
    pub r#ref: Option<ExternalReference>,
}

/// Fold a Restaurant stream (events in version order) into its current state. `None` ⇔ the stream has
/// no `RestaurantRegistered` yet, i.e. the aggregate does not exist.
pub fn fold(events: &[DomainEvent]) -> Option<RestaurantState> {
    events.iter().fold(None, apply)
}

/// Apply one event to the state — a pure transition, total over the whole event union (events not
/// touching the folded fields are no-ops, so a fatter stream never breaks rehydration).
fn apply(state: Option<RestaurantState>, event: &DomainEvent) -> Option<RestaurantState> {
    if let DomainEvent::RestaurantRegistered(e) = event {
        return Some(RestaurantState {
            status: RestaurantStatus::DRAFT,
            order_acceptance: OrderAcceptanceMode::NORMAL,
            listing_status: e.listing_status,
            listing_claimed: false,
            gbp_order_url: None,
            slug: e.slug.clone(),
            display_name: e.display_name.clone(),
            r#ref: e.r#ref.clone(),
        });
    }
    let mut s = state?;
    match event {
        DomainEvent::RestaurantActivated(_) => s.status = RestaurantStatus::ACTIVE,
        DomainEvent::RestaurantDeactivated(_)
        | DomainEvent::RestaurantRemoved(_)
        | DomainEvent::RestaurantMarkedClosed(_) => s.status = RestaurantStatus::INACTIVE,
        DomainEvent::RestaurantAcceptanceModeChanged(e) => s.order_acceptance = e.mode,
        DomainEvent::RestaurantUpdated(e) => {
            if let Some(name) = &e.display_name {
                s.display_name = name.clone();
            }
        }
        DomainEvent::RestaurantListingClaimed(_) => s.listing_claimed = true,
        DomainEvent::RestaurantListingStatusChanged(e) => s.listing_status = e.listing_status,
        DomainEvent::RestaurantGoogleBusinessProfileOrderLinkConfigured(e) => {
            s.gbp_order_url = Some(e.gbp_order_url.clone())
        }
        _ => {}
    }
    Some(s)
}
