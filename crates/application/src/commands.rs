//! CQRS command handlers (write side, ADR-0035). Thin by design: rehydrate the aggregate state by
//! folding its stream (loaded through the [`EventStore`] port), enforce the invariants declared for
//! that message in `specs/actors.yaml` (`throws` → `specs/errors.yaml`), then append the declared
//! `emits` event(s) at the expected version. Ids are client/ACL-generated (ADR-0034), so creation
//! commands are idempotent: replaying one hits the UNIQUE(stream_name, version) guard and is absorbed
//! as an already-registered no-op instead of duplicating the fact.
//!
//! Rejections carry the errors.yaml CODE: [`DomainError::Invariant`] only models a string, so the
//! canonical shape is `"<Code>: <context>"` (see [`reject`] / [`rejection_code`]) until a structured
//! error type lands.
//!
//! Cross-aggregate invariants that still lack a read port are explicit `TODO(invariant)` markers —
//! they are NOT silently skipped semantics, they are the documented gap.

use domain::generated::commands::{
    ActivateRestaurant, ChangeOrderAcceptanceMode, ChangeRestaurantListingStatus,
    ClaimRestaurantListing, ConfigureGoogleBusinessProfileOrderLink, DeactivateRestaurant,
    MarkRestaurantClosed, OptOutRestaurantListing, RegisterRestaurant, RegisterRestaurantAccount,
    RemoveRestaurant, UpdateRestaurant, UpdateRestaurantGoogleBusinessProfile,
    VerifyGoogleBusinessProfileOrderLink,
};
use domain::generated::events::{
    DomainEvent, RestaurantAcceptanceModeChanged, RestaurantAccountRegistered, RestaurantActivated,
    RestaurantDeactivated, RestaurantGoogleBusinessProfileOrderLinkConfigured,
    RestaurantGoogleBusinessProfileOrderLinkVerified, RestaurantGoogleBusinessProfileUpdated,
    RestaurantListingClaimed, RestaurantListingOptedOut, RestaurantListingStatusChanged,
    RestaurantMarkedClosed, RestaurantRegistered, RestaurantRemoved, RestaurantUpdated,
};
use domain::generated::scalars::{RestaurantId, RestaurantListingStatus, RestaurantStatus};
use domain::restaurant::RestaurantState;
use domain::shared::errors::DomainError;

use crate::ports::{is_version_conflict, Actor, EventStore, GbpOrderLinkProbe, GoogleOwnershipVerifier};
use crate::queries::RestaurantReadRepository;

/// Absorb the optimistic-concurrency clash of a CREATION command (expected_version = 0) as success:
/// the aggregate already exists under this client-generated id, so re-running the command is a no-op.
fn idempotent_on_existing(result: Result<i64, DomainError>) -> Result<(), DomainError> {
    match result {
        Ok(_) => Ok(()),
        Err(e) if is_version_conflict(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Build the canonical rejection for an `errors.yaml` invariant: the CODE, then the context detail —
/// `"<Code>: <detail>"`. [`rejection_code`] is the matching reader.
fn reject(code: &str, detail: impl std::fmt::Display) -> DomainError {
    DomainError::Invariant(format!("{code}: {detail}"))
}

/// The errors.yaml code a command rejection carries (`"<Code>: <detail>"`), if this is one.
pub fn rejection_code(err: &DomainError) -> Option<&str> {
    match err {
        DomainError::Invariant(msg) => msg.split(':').next().map(str::trim),
        DomainError::Repository(_) => None,
    }
}

/// The stream a Restaurant aggregate lives on.
fn restaurant_stream(id: &RestaurantId) -> String {
    format!("Restaurant-{}", id.0)
}

/// Rehydrate the Restaurant aggregate: fold its stream into the minimal write-side state and return it
/// with the stream's current version (the expected version for the next append).
async fn load_restaurant(
    store: &dyn EventStore,
    id: &RestaurantId,
) -> Result<(Option<RestaurantState>, i64), DomainError> {
    let (events, version) = store.load(&restaurant_stream(id)).await?;
    Ok((domain::restaurant::fold(&events), version))
}

/// Rehydrate and require existence, or reject with `errors.yaml#/RestaurantNotFound`.
async fn require_restaurant(
    store: &dyn EventStore,
    id: &RestaurantId,
) -> Result<(RestaurantState, i64), DomainError> {
    let (state, version) = load_restaurant(store, id).await?;
    match state {
        Some(state) => Ok((state, version)),
        None => Err(reject("RestaurantNotFound", format!("restaurantId={}", id.0))),
    }
}

/// Handle `commands.yaml#/RegisterRestaurantAccount` → emit `events.yaml#/RestaurantAccountRegistered`
/// on the new `RestaurantAccount-<id>` stream (actors.yaml, RestaurantAccount aggregate).
pub async fn register_restaurant_account(
    store: &dyn EventStore,
    cmd: RegisterRestaurantAccount,
    actor: &Actor,
) -> Result<(), DomainError> {
    // TODO(invariant): RefAlreadyUsed — reject when cmd.ref is already owned by another aggregate
    //                  (needs an external-reference read-model lookup).
    // TODO(invariant): InvalidCurrency — reject when cmd.default_currency is not a valid ISO 4217 code.
    let stream_name = format!("RestaurantAccount-{}", cmd.restaurant_account_id.0);
    let event = DomainEvent::RestaurantAccountRegistered(RestaurantAccountRegistered {
        restaurant_account_id: cmd.restaurant_account_id,
        r#ref: cmd.r#ref,
        legal_name: cmd.legal_name,
        contact: cmd.contact,
        default_currency: cmd.default_currency,
        default_tax_rate: cmd.default_tax_rate,
        timezone: cmd.timezone,
    });
    idempotent_on_existing(store.append(&stream_name, 0, &[event], actor).await)
}

/// Handle `commands.yaml#/RegisterRestaurant` → emit `events.yaml#/RestaurantRegistered` on the new
/// `Restaurant-<id>` stream (actors.yaml, Restaurant aggregate). `listingStatus` defaults to
/// NON_PARTNER when omitted (e.g. a Sirene/Google sync-seeded listing), per the command spec.
///
/// `restaurants` backs the `SlugAlreadyTaken` uniqueness check (the Restaurant projection is the only
/// slug index we have). A row already owning the slug under the SAME restaurant id is the idempotent
/// replay of this very registration and is not a conflict.
pub async fn register_restaurant(
    store: &dyn EventStore,
    restaurants: &dyn RestaurantReadRepository,
    cmd: RegisterRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    // TODO(invariant): RestaurantAccountNotFound — when cmd.account_id is set, reject if the owning
    //                  RestaurantAccount does not exist (needs an account read-model lookup port).
    // TODO(invariant): RefAlreadyUsed — reject when cmd.ref is already owned by another aggregate
    //                  (needs an external-reference read-model lookup port).
    if let Some(existing) = restaurants.by_slug(cmd.slug.clone()).await? {
        if existing.restaurant_id != cmd.restaurant_id {
            return Err(reject("SlugAlreadyTaken", format!("slug={}", cmd.slug.0)));
        }
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantRegistered(RestaurantRegistered {
        mode: cmd.mode,
        restaurant_id: cmd.restaurant_id,
        account_id: cmd.account_id,
        listing_status: cmd.listing_status.unwrap_or(RestaurantListingStatus::NON_PARTNER),
        r#ref: cmd.r#ref,
        external_identifiers: cmd.external_identifiers,
        slug: cmd.slug,
        display_name: cmd.display_name,
        contact: cmd.contact,
        website: cmd.website,
        tags: cmd.tags,
        margin_rate: cmd.margin_rate,
        cuisine_category: cmd.cuisine_category,
        uber_prices_opt_in: cmd.uber_prices_opt_in,
        address: cmd.address,
        location: cmd.location,
        timezone: cmd.timezone,
        preparation_time_minutes: cmd.preparation_time_minutes,
        opening_hours: cmd.opening_hours,
    });
    idempotent_on_existing(store.append(&stream_name, 0, &[event], actor).await)
}

/// Handle `commands.yaml#/ActivateRestaurant` → emit `events.yaml#/RestaurantActivated`. Idempotent
/// per actors.yaml: activating an already-ACTIVE restaurant is a no-op (no event, no error) — the
/// command ensures the ACTIVE state, it is not a toggle.
pub async fn activate_restaurant(
    store: &dyn EventStore,
    cmd: ActivateRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    // TODO(invariant): RestaurantNotReadyForActivation — "at least one catalog with one orderable
    //                  offer" is a cross-aggregate (Catalog) check; needs a catalog read-model port.
    if state.status == RestaurantStatus::ACTIVE {
        return Ok(());
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantActivated(RestaurantActivated {
        restaurant_id: cmd.restaurant_id,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/UpdateRestaurant` → emit `events.yaml#/RestaurantUpdated` (full replace of
/// the provided location fields). An update carrying nothing editable is rejected
/// (`errors.yaml#/NoEditableFieldProvided`).
pub async fn update_restaurant(
    store: &dyn EventStore,
    cmd: UpdateRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let has_editable_field = cmd.display_name.is_some()
        || cmd.contact.is_some()
        || cmd.website.is_some()
        || !cmd.tags.is_empty()
        || cmd.margin_rate.is_some()
        || cmd.cuisine_category.is_some()
        || cmd.uber_prices_opt_in.is_some()
        || cmd.address.is_some()
        || cmd.location.is_some()
        || cmd.timezone.is_some()
        || cmd.preparation_time_minutes.is_some()
        || !cmd.opening_hours.is_empty();
    if !has_editable_field {
        return Err(reject("NoEditableFieldProvided", "update carried no editable field"));
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantUpdated(RestaurantUpdated {
        restaurant_id: cmd.restaurant_id,
        display_name: cmd.display_name,
        contact: cmd.contact,
        website: cmd.website,
        tags: cmd.tags,
        margin_rate: cmd.margin_rate,
        cuisine_category: cmd.cuisine_category,
        uber_prices_opt_in: cmd.uber_prices_opt_in,
        address: cmd.address,
        location: cmd.location,
        timezone: cmd.timezone,
        preparation_time_minutes: cmd.preparation_time_minutes,
        opening_hours: cmd.opening_hours,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/DeactivateRestaurant` → emit `events.yaml#/RestaurantDeactivated`.
/// Idempotent per actors.yaml: deactivating an already-INACTIVE restaurant is a no-op (no event, no
/// error) — the command ensures the INACTIVE state, it is not a toggle.
pub async fn deactivate_restaurant(
    store: &dyn EventStore,
    cmd: DeactivateRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    if state.status == RestaurantStatus::INACTIVE {
        return Ok(());
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantDeactivated(RestaurantDeactivated {
        restaurant_id: cmd.restaurant_id,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/ChangeOrderAcceptanceMode` → emit
/// `events.yaml#/RestaurantAcceptanceModeChanged`. Only an ACTIVE restaurant toggles its live mode
/// (`RestaurantNotActive`), and re-requesting the current mode is rejected
/// (`AcceptanceModeUnchanged`).
pub async fn change_order_acceptance_mode(
    store: &dyn EventStore,
    cmd: ChangeOrderAcceptanceMode,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    if state.status != RestaurantStatus::ACTIVE {
        return Err(reject(
            "RestaurantNotActive",
            format!("restaurantId={} restaurantName={}", cmd.restaurant_id.0, state.display_name.0),
        ));
    }
    if state.order_acceptance == cmd.mode {
        return Err(reject(
            "AcceptanceModeUnchanged",
            format!(
                "restaurantId={} restaurantName={} mode={:?}",
                cmd.restaurant_id.0, state.display_name.0, cmd.mode
            ),
        ));
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantAcceptanceModeChanged(RestaurantAcceptanceModeChanged {
        restaurant_id: cmd.restaurant_id,
        mode: cmd.mode,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/RemoveRestaurant` → emit `events.yaml#/RestaurantRemoved` (the location is
/// delisted from its account; the stream and its history remain).
pub async fn remove_restaurant(
    store: &dyn EventStore,
    cmd: RemoveRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantRemoved(RestaurantRemoved {
        restaurant_id: cmd.restaurant_id,
        account_id: cmd.account_id,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/UpdateRestaurantGoogleBusinessProfile` → emit
/// `events.yaml#/RestaurantGoogleBusinessProfileUpdated` (GBP-specific metrics only; issued by the
/// Sirene/Google sync ACL or admin).
pub async fn update_restaurant_google_business_profile(
    store: &dyn EventStore,
    cmd: UpdateRestaurantGoogleBusinessProfile,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event =
        DomainEvent::RestaurantGoogleBusinessProfileUpdated(RestaurantGoogleBusinessProfileUpdated {
            restaurant_id: cmd.restaurant_id,
            google_place_id: cmd.google_place_id,
            rating: cmd.rating,
            reviews_count: cmd.reviews_count,
        });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/MarkRestaurantClosed` → emit `events.yaml#/RestaurantMarkedClosed` (e.g. a
/// Sirene closure reported through the sync ACL).
pub async fn mark_restaurant_closed(
    store: &dyn EventStore,
    cmd: MarkRestaurantClosed,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantMarkedClosed(RestaurantMarkedClosed {
        restaurant_id: cmd.restaurant_id,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/ClaimRestaurantListing` → emit `events.yaml#/RestaurantListingClaimed`.
/// A listing can be claimed once (`ListingAlreadyClaimed`), and only with a Google Business Profile
/// ownership proof the verifier accepts (`ListingOwnershipNotVerified`, ADR-0019).
pub async fn claim_restaurant_listing(
    store: &dyn EventStore,
    ownership: &dyn GoogleOwnershipVerifier,
    cmd: ClaimRestaurantListing,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    if state.listing_claimed {
        return Err(reject(
            "ListingAlreadyClaimed",
            format!("restaurantId={}", cmd.restaurant_id.0),
        ));
    }
    if !ownership.verify(cmd.restaurant_id, &cmd.google_ownership_proof).await? {
        return Err(reject(
            "ListingOwnershipNotVerified",
            format!("restaurantId={}", cmd.restaurant_id.0),
        ));
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantListingClaimed(RestaurantListingClaimed {
        restaurant_id: cmd.restaurant_id,
        account_id: cmd.account_id,
        proof: Some(cmd.google_ownership_proof),
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/OptOutRestaurantListing` → emit `events.yaml#/RestaurantListingOptedOut`.
/// Requires the same verified GBP ownership proof as a claim (`ListingOwnershipNotVerified`).
pub async fn opt_out_restaurant_listing(
    store: &dyn EventStore,
    ownership: &dyn GoogleOwnershipVerifier,
    cmd: OptOutRestaurantListing,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    if !ownership.verify(cmd.restaurant_id, &cmd.google_ownership_proof).await? {
        return Err(reject(
            "ListingOwnershipNotVerified",
            format!("restaurantId={}", cmd.restaurant_id.0),
        ));
    }
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantListingOptedOut(RestaurantListingOptedOut {
        restaurant_id: cmd.restaurant_id,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/ChangeRestaurantListingStatus` → emit
/// `events.yaml#/RestaurantListingStatusChanged` (admin moves a listing along the partnership funnel).
pub async fn change_restaurant_listing_status(
    store: &dyn EventStore,
    cmd: ChangeRestaurantListingStatus,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantListingStatusChanged(RestaurantListingStatusChanged {
        restaurant_id: cmd.restaurant_id,
        listing_status: cmd.listing_status,
        reason: cmd.reason,
    });
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/ConfigureGoogleBusinessProfileOrderLink` → emit
/// `events.yaml#/RestaurantGoogleBusinessProfileOrderLinkConfigured` (ADR-0021; V1).
pub async fn configure_gbp_order_link(
    store: &dyn EventStore,
    cmd: ConfigureGoogleBusinessProfileOrderLink,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (_state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantGoogleBusinessProfileOrderLinkConfigured(
        RestaurantGoogleBusinessProfileOrderLinkConfigured {
            restaurant_id: cmd.restaurant_id,
            gbp_order_url: cmd.gbp_order_url,
        },
    );
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}

/// Handle `commands.yaml#/VerifyGoogleBusinessProfileOrderLink` → emit
/// `events.yaml#/RestaurantGoogleBusinessProfileOrderLinkVerified` (ADR-0021; V1). Requires a
/// configured link (`GbpOrderLinkNotConfigured`); the probe port pings it and the handler records the
/// observed status.
pub async fn verify_gbp_order_link(
    store: &dyn EventStore,
    probe: &dyn GbpOrderLinkProbe,
    cmd: VerifyGoogleBusinessProfileOrderLink,
    actor: &Actor,
) -> Result<(), DomainError> {
    let (state, version) = require_restaurant(store, &cmd.restaurant_id).await?;
    let Some(url) = state.gbp_order_url else {
        return Err(reject(
            "GbpOrderLinkNotConfigured",
            format!("restaurantId={}", cmd.restaurant_id.0),
        ));
    };
    let status = probe.probe(&url).await?;
    let stream_name = restaurant_stream(&cmd.restaurant_id);
    let event = DomainEvent::RestaurantGoogleBusinessProfileOrderLinkVerified(
        RestaurantGoogleBusinessProfileOrderLinkVerified {
            restaurant_id: cmd.restaurant_id,
            status,
        },
    );
    store.append(&stream_name, version, &[event], actor).await.map(|_| ())
}
