//! CQRS command handlers (write side, ADR-0035). Thin by design: map the command onto the emitted
//! business event(s) declared in `specs/actors.yaml`, then append to the stream through the
//! [`EventStore`] port at the expected version. Ids are client/ACL-generated (ADR-0034), so creation
//! commands are idempotent: replaying one hits the UNIQUE(stream_name, version) guard and is absorbed
//! as an already-registered no-op instead of duplicating the fact.
//!
//! Cross-aggregate invariants (the `throws` of actors.yaml) need read-model lookups and are left as
//! explicit `TODO(invariant)` markers until the corresponding query ports exist — they are NOT
//! silently skipped semantics, they are the documented gap.

use domain::generated::commands::{RegisterRestaurant, RegisterRestaurantAccount};
use domain::generated::events::{
    DomainEvent, RestaurantAccountRegistered, RestaurantRegistered,
};
use domain::generated::scalars::RestaurantListingStatus;
use domain::shared::errors::DomainError;

use crate::ports::{is_version_conflict, Actor, EventStore};

/// Absorb the optimistic-concurrency clash of a CREATION command (expected_version = 0) as success:
/// the aggregate already exists under this client-generated id, so re-running the command is a no-op.
fn idempotent_on_existing(result: Result<i64, DomainError>) -> Result<(), DomainError> {
    match result {
        Ok(_) => Ok(()),
        Err(e) if is_version_conflict(&e) => Ok(()),
        Err(e) => Err(e),
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
pub async fn register_restaurant(
    store: &dyn EventStore,
    cmd: RegisterRestaurant,
    actor: &Actor,
) -> Result<(), DomainError> {
    // TODO(invariant): RestaurantAccountNotFound — when cmd.account_id is set, reject if the owning
    //                  RestaurantAccount does not exist (needs an account read-model lookup).
    // TODO(invariant): SlugAlreadyTaken — reject when another restaurant already uses cmd.slug
    //                  (needs a slug read-model lookup).
    // TODO(invariant): RefAlreadyUsed — reject when cmd.ref is already owned by another aggregate
    //                  (needs an external-reference read-model lookup).
    let stream_name = format!("Restaurant-{}", cmd.restaurant_id.0);
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
