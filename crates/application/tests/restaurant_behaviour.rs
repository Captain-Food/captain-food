//! BEHAVIOUR tests for the Restaurant aggregate — the executable form of the `specs/tests.yaml`
//! Given/When/Then cases whose `when` is a Restaurant-aggregate command (ADR-0032: each test cites the
//! `specs/rules.yaml` rule it asserts). Given = pre-seeded stream events (in-memory event store),
//! When = the command handler, Then = the emitted event(s) / the errors.yaml rejection code.
//!
//! Pure and offline: an in-memory [`EventStore`] plus fakes for the read/verification ports
//! (`RestaurantReadRepository` for the slug index, `GoogleOwnershipVerifier`, `GbpOrderLinkProbe`).
//! Cross-aggregate invariants still lacking a port (RestaurantAccountNotFound, RefAlreadyUsed,
//! RestaurantNotReadyForActivation) are documented `TODO(invariant)`s in `application::commands` and
//! are NOT asserted here.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use application::commands::{
    activate_restaurant, change_order_acceptance_mode, change_restaurant_listing_status,
    claim_restaurant_listing, configure_gbp_order_link, deactivate_restaurant, mark_restaurant_closed,
    opt_out_restaurant_listing, register_restaurant, rejection_code, remove_restaurant,
    update_restaurant, update_restaurant_google_business_profile, verify_gbp_order_link,
};
use application::ports::{
    version_conflict, Actor, EventStore, GbpOrderLinkProbe, GoogleOwnershipVerifier,
};
use application::queries::{RestaurantFilter, RestaurantReadRepository, RestaurantRow};
use domain::generated::commands::{
    ActivateRestaurant, ChangeOrderAcceptanceMode, ChangeRestaurantListingStatus,
    ClaimRestaurantListing, ConfigureGoogleBusinessProfileOrderLink, DeactivateRestaurant,
    MarkRestaurantClosed, OptOutRestaurantListing, RegisterRestaurant, RemoveRestaurant,
    UpdateRestaurant, UpdateRestaurantGoogleBusinessProfile, VerifyGoogleBusinessProfileOrderLink,
};
use domain::generated::entities::{Address, ExternalIdentifier};
use domain::generated::events::{
    DomainEvent, RestaurantActivated, RestaurantGoogleBusinessProfileOrderLinkConfigured,
    RestaurantListingClaimed, RestaurantRegistered,
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

/// Fake slug index: `by_slug` answers from at most one configured row (the read side is a projection,
/// so seeding it directly mirrors "another restaurant already projected this slug").
#[derive(Default)]
struct FakeRestaurants {
    row: Option<RestaurantRow>,
}

#[async_trait]
impl RestaurantReadRepository for FakeRestaurants {
    async fn list(&self, _filter: RestaurantFilter) -> Result<Vec<RestaurantRow>, DomainError> {
        Ok(self.row.clone().into_iter().collect())
    }

    async fn by_slug(&self, slug: Slug) -> Result<Option<RestaurantRow>, DomainError> {
        Ok(self.row.clone().filter(|r| r.slug == slug))
    }

    async fn by_id(&self, id: RestaurantId) -> Result<Option<RestaurantRow>, DomainError> {
        Ok(self.row.clone().filter(|r| r.restaurant_id == id))
    }
}

/// Fake GBP ownership verifier: accepts exactly the fixture's valid token (tests.yaml uses
/// `"valid-gbp-token"` / `"bad-token"`).
struct FakeOwnership;

#[async_trait]
impl GoogleOwnershipVerifier for FakeOwnership {
    async fn verify(&self, _restaurant_id: RestaurantId, proof: &str) -> Result<bool, DomainError> {
        Ok(proof == "valid-gbp-token")
    }
}

/// Fake link probe: the configured link answers → VERIFIED (the fixture's expected status).
struct FakeProbe;

#[async_trait]
impl GbpOrderLinkProbe for FakeProbe {
    async fn probe(&self, _url: &WebUrl) -> Result<GbpLinkStatus, DomainError> {
        Ok(GbpLinkStatus::VERIFIED)
    }
}

// ------------------------------------------------------------------------------------------------
// Fixtures (tests.yaml `fixtures`, with UUIDs instead of the sample string ids)
// ------------------------------------------------------------------------------------------------

fn actor() -> Actor {
    Actor {
        user_id: uuid::Uuid::new_v4(),
        user_type: 5, // UserType::ADMIN ordinal
        correlation_id: uuid::Uuid::new_v4(),
        cause_id: None,
    }
}

fn address() -> Address {
    Address {
        line1: AddressLine("1 Rue Nationale".into()),
        line2: None,
        postal_code: PostalCode("37000".into()),
        city: CityName("Tours".into()),
        country: CountryCode("FR".into()),
    }
}

fn stream(id: RestaurantId) -> String {
    format!("Restaurant-{}", id.0)
}

/// Fixture `restaurantRegistered` (partner location under an account) /
/// `restaurantSeeded` (account-less NON_PARTNER listing) — parameterized.
fn registered_event(
    id: RestaurantId,
    account_id: Option<RestaurantAccountId>,
    listing_status: RestaurantListingStatus,
    slug: &str,
) -> DomainEvent {
    DomainEvent::RestaurantRegistered(RestaurantRegistered {
        mode: None,
        restaurant_id: id,
        account_id,
        listing_status,
        r#ref: None,
        external_identifiers: vec![],
        slug: Slug(slug.into()),
        display_name: RestaurantDisplayName("Chez Marco".into()),
        contact: None,
        website: None,
        tags: vec![],
        margin_rate: None,
        cuisine_category: None,
        uber_prices_opt_in: None,
        address: address(),
        location: None,
        timezone: Some(TimeZone("Europe/Paris".into())),
        preparation_time_minutes: None,
        opening_hours: vec![],
    })
}

fn activated_event(id: RestaurantId) -> DomainEvent {
    DomainEvent::RestaurantActivated(RestaurantActivated { restaurant_id: id, reason: None })
}

fn register_cmd(id: RestaurantId, account_id: Option<RestaurantAccountId>, slug: &str) -> RegisterRestaurant {
    RegisterRestaurant {
        mode: None,
        restaurant_id: id,
        account_id,
        listing_status: None, // → defaults to NON_PARTNER per the command spec
        slug: Slug(slug.into()),
        display_name: RestaurantDisplayName("Chez Marco".into()),
        contact: None,
        website: None,
        tags: vec![],
        margin_rate: None,
        cuisine_category: None,
        uber_prices_opt_in: None,
        address: address(),
        location: None,
        timezone: Some(TimeZone("Europe/Paris".into())),
        preparation_time_minutes: None,
        opening_hours: vec![],
        external_identifiers: vec![],
        r#ref: None,
    }
}

/// A projected `restaurant` row owning `slug` (what `by_slug` would return).
fn projected_row(id: RestaurantId, slug: &str) -> RestaurantRow {
    RestaurantRow {
        restaurant_id: id,
        restaurant_account_id: None,
        listing_status: RestaurantListingStatus::NON_PARTNER,
        external_identifiers: None,
        google_place_id: None,
        slug: Slug(slug.into()),
        display_name: RestaurantDisplayName("Chez Marco".into()),
        description: None,
        tags: None,
        margin_rate: None,
        cuisine_category: None,
        uber_prices_opt_in: None,
        website: None,
        rating: None,
        reviews_count: None,
        gbp_order_url: None,
        gbp_link_status: None,
        address: serde_json::json!({ "line1": "1 Rue Nationale", "postalCode": "37000", "city": "Tours", "country": "FR" }),
        location: None,
        opening_hours: serde_json::json!([]),
        status: RestaurantStatus::DRAFT,
        order_acceptance: OrderAcceptanceMode::NORMAL,
        default_currency: CurrencyCode("EUR".into()),
        timezone: None,
        preparation_time_minutes: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn rid() -> RestaurantId {
    RestaurantId(uuid::Uuid::new_v4())
}

// ------------------------------------------------------------------------------------------------
// Registration (rules.yaml#/LocationRegistrationUnderAccountUniqueSlug, #/ListingSeededFromOpenData)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRestaurantLocationRegistered — rules.yaml#/LocationRegistrationUnderAccountUniqueSlug
#[tokio::test]
async fn registers_a_location_under_an_existing_account() {
    let store = MemStore::default();
    let id = rid();
    let account = RestaurantAccountId(uuid::Uuid::new_v4());

    register_restaurant(&store, &FakeRestaurants::default(), register_cmd(id, Some(account), "chez-marco"), &actor())
        .await
        .expect("register");

    let events = store.stream(&stream(id));
    assert_eq!(events.len(), 1);
    let DomainEvent::RestaurantRegistered(e) = &events[0] else {
        panic!("expected RestaurantRegistered, got {:?}", events[0]);
    };
    assert_eq!(e.restaurant_id, id);
    assert_eq!(e.account_id, Some(account));
    assert_eq!(e.slug.0, "chez-marco");
    // listingStatus omitted on the command → the spec default.
    assert_eq!(e.listing_status, RestaurantListingStatus::NON_PARTNER);
}

/// tests.yaml#/cases/TestRestaurantRegisterIsRejected (SlugAlreadyTaken arm) —
/// rules.yaml#/LocationRegistrationUnderAccountUniqueSlug. The RestaurantAccountNotFound and
/// RefAlreadyUsed arms are TODO(invariant) until their read ports exist.
#[tokio::test]
async fn rejects_registering_when_the_slug_is_taken_by_another_restaurant() {
    let store = MemStore::default();
    let taken_by = rid();
    let repo = FakeRestaurants { row: Some(projected_row(taken_by, "chez-marco")) };

    let err = register_restaurant(&store, &repo, register_cmd(rid(), None, "chez-marco"), &actor())
        .await
        .expect_err("slug is taken");
    assert_eq!(rejection_code(&err), Some("SlugAlreadyTaken"));
    assert!(store.stream(&stream(taken_by)).is_empty(), "no event on rejection");
}

/// Idempotent replay: the slug row belongs to the SAME restaurant id → not a conflict, and the
/// version-0 clash on the stream is absorbed as success (client-generated ids, ADR-0034).
#[tokio::test]
async fn replaying_the_same_registration_is_a_no_op() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::NON_PARTNER, "chez-marco")]);
    let repo = FakeRestaurants { row: Some(projected_row(id, "chez-marco")) };

    register_restaurant(&store, &repo, register_cmd(id, None, "chez-marco"), &actor())
        .await
        .expect("replay absorbed");
    assert_eq!(store.stream(&stream(id)).len(), 1, "no duplicate fact");
}

/// tests.yaml#/cases/TestRestaurantSeededFromSync — rules.yaml#/ListingSeededFromOpenData
#[tokio::test]
async fn sync_acl_seeds_a_public_non_partner_listing_without_account() {
    let store = MemStore::default();
    let id = rid();
    let mut cmd = register_cmd(id, None, "le-saint-honore");
    cmd.listing_status = Some(RestaurantListingStatus::NON_PARTNER);
    cmd.external_identifiers = vec![ExternalIdentifier {
        key: ExternalIdentifierKey("siret".into()),
        value: "12345678900012".into(),
    }];

    register_restaurant(&store, &FakeRestaurants::default(), cmd, &actor()).await.expect("seed");

    let events = store.stream(&stream(id));
    let DomainEvent::RestaurantRegistered(e) = &events[0] else { panic!("expected RestaurantRegistered") };
    assert_eq!(e.account_id, None);
    assert_eq!(e.listing_status, RestaurantListingStatus::NON_PARTNER);
    assert_eq!(e.external_identifiers.len(), 1);
}

// ------------------------------------------------------------------------------------------------
// Activation lifecycle (rules.yaml#/RestaurantActivationVisibility, #/RestaurantActivationIdempotent,
// #/RestaurantDeactivation)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRestaurantActivated — rules.yaml#/RestaurantActivationVisibility
#[tokio::test]
async fn activates_a_registered_restaurant() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    activate_restaurant(&store, ActivateRestaurant { restaurant_id: id, reason: None }, &actor())
        .await
        .expect("activate");

    let events = store.stream(&stream(id));
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[1], DomainEvent::RestaurantActivated(e) if e.restaurant_id == id));
}

/// tests.yaml#/cases/TestRestaurantActivateAgainIsNoOp — rules.yaml#/RestaurantActivationIdempotent
#[tokio::test]
async fn re_activating_an_active_restaurant_is_a_no_op() {
    let store = MemStore::default();
    let id = rid();
    store.seed(
        &stream(id),
        vec![
            registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco"),
            activated_event(id),
        ],
    );

    activate_restaurant(&store, ActivateRestaurant { restaurant_id: id, reason: None }, &actor())
        .await
        .expect("idempotent");
    assert_eq!(store.stream(&stream(id)).len(), 2, "no event emitted");
}

/// tests.yaml#/cases/TestRestaurantActivateIsRejected (RestaurantNotFound arm) —
/// rules.yaml#/RestaurantActivationVisibility. The RestaurantNotReadyForActivation arm is
/// TODO(invariant) until a Catalog read port exists.
#[tokio::test]
async fn rejects_activating_a_missing_restaurant() {
    let store = MemStore::default();
    let err = activate_restaurant(&store, ActivateRestaurant { restaurant_id: rid(), reason: None }, &actor())
        .await
        .expect_err("missing restaurant");
    assert_eq!(rejection_code(&err), Some("RestaurantNotFound"));
}

/// tests.yaml#/cases/TestRestaurantDeactivated — rules.yaml#/RestaurantDeactivation
#[tokio::test]
async fn deactivates_an_active_restaurant() {
    let store = MemStore::default();
    let id = rid();
    store.seed(
        &stream(id),
        vec![
            registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco"),
            activated_event(id),
        ],
    );

    deactivate_restaurant(
        &store,
        DeactivateRestaurant { restaurant_id: id, reason: Some("Holidays".into()) },
        &actor(),
    )
    .await
    .expect("deactivate");

    let events = store.stream(&stream(id));
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[2], DomainEvent::RestaurantDeactivated(e) if e.reason.as_deref() == Some("Holidays")));
}

/// actors.yaml Restaurant/DeactivateRestaurant: deactivating an already-INACTIVE restaurant is a
/// no-op (the command ensures the state, it is not a toggle) — rules.yaml#/RestaurantDeactivation.
#[tokio::test]
async fn re_deactivating_an_inactive_restaurant_is_a_no_op() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);
    deactivate_restaurant(&store, DeactivateRestaurant { restaurant_id: id, reason: None }, &actor())
        .await
        .expect("first deactivate (DRAFT → INACTIVE)");
    let after_first = store.stream(&stream(id)).len();

    deactivate_restaurant(&store, DeactivateRestaurant { restaurant_id: id, reason: None }, &actor())
        .await
        .expect("idempotent");
    assert_eq!(store.stream(&stream(id)).len(), after_first, "no event emitted");
}

// ------------------------------------------------------------------------------------------------
// Update (rules.yaml#/RestaurantLocationFieldsUpdate)
// ------------------------------------------------------------------------------------------------

fn empty_update(id: RestaurantId) -> UpdateRestaurant {
    UpdateRestaurant {
        restaurant_id: id,
        display_name: None,
        contact: None,
        website: None,
        tags: vec![],
        margin_rate: None,
        cuisine_category: None,
        uber_prices_opt_in: None,
        address: None,
        location: None,
        timezone: None,
        preparation_time_minutes: None,
        opening_hours: vec![],
    }
}

/// tests.yaml#/cases/TestRestaurantUpdated — rules.yaml#/RestaurantLocationFieldsUpdate
#[tokio::test]
async fn updates_editable_location_fields() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    let mut cmd = empty_update(id);
    cmd.display_name = Some(RestaurantDisplayName("Chez Marco — Centre".into()));
    update_restaurant(&store, cmd, &actor()).await.expect("update");

    let events = store.stream(&stream(id));
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[1],
        DomainEvent::RestaurantUpdated(e) if e.display_name.as_ref().map(|n| n.0.as_str()) == Some("Chez Marco — Centre")
    ));
}

/// tests.yaml#/cases/TestRestaurantUpdateIsRejected (both arms) —
/// rules.yaml#/RestaurantLocationFieldsUpdate
#[tokio::test]
async fn rejects_updating_a_missing_restaurant_or_an_empty_update() {
    let store = MemStore::default();

    // Missing restaurant → RestaurantNotFound.
    let err = update_restaurant(&store, empty_update(rid()), &actor()).await.expect_err("missing");
    assert_eq!(rejection_code(&err), Some("RestaurantNotFound"));

    // Existing restaurant, nothing editable provided → NoEditableFieldProvided.
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);
    let err = update_restaurant(&store, empty_update(id), &actor()).await.expect_err("empty update");
    assert_eq!(rejection_code(&err), Some("NoEditableFieldProvided"));
    assert_eq!(store.stream(&stream(id)).len(), 1, "no event on rejection");
}

// ------------------------------------------------------------------------------------------------
// Acceptance mode (rules.yaml#/OrderAcceptanceModeManagement)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRestaurantAcceptanceModeChanged — rules.yaml#/OrderAcceptanceModeManagement
#[tokio::test]
async fn switches_an_active_restaurant_to_busy() {
    let store = MemStore::default();
    let id = rid();
    store.seed(
        &stream(id),
        vec![
            registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco"),
            activated_event(id),
        ],
    );

    change_order_acceptance_mode(
        &store,
        ChangeOrderAcceptanceMode { restaurant_id: id, mode: OrderAcceptanceMode::BUSY },
        &actor(),
    )
    .await
    .expect("change mode");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[2],
        DomainEvent::RestaurantAcceptanceModeChanged(e) if e.mode == OrderAcceptanceMode::BUSY
    ));
}

/// tests.yaml#/cases/TestRestaurantAcceptanceModeIsRejected (both arms) —
/// rules.yaml#/OrderAcceptanceModeManagement
#[tokio::test]
async fn rejects_changing_mode_when_inactive_or_unchanged() {
    let store = MemStore::default();

    // Registered but never activated (DRAFT) → RestaurantNotActive.
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);
    let err = change_order_acceptance_mode(
        &store,
        ChangeOrderAcceptanceMode { restaurant_id: id, mode: OrderAcceptanceMode::BUSY },
        &actor(),
    )
    .await
    .expect_err("not active");
    assert_eq!(rejection_code(&err), Some("RestaurantNotActive"));

    // Active but already in the requested mode (NORMAL since registration) → AcceptanceModeUnchanged.
    let id = rid();
    store.seed(
        &stream(id),
        vec![
            registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco"),
            activated_event(id),
        ],
    );
    let err = change_order_acceptance_mode(
        &store,
        ChangeOrderAcceptanceMode { restaurant_id: id, mode: OrderAcceptanceMode::NORMAL },
        &actor(),
    )
    .await
    .expect_err("unchanged");
    assert_eq!(rejection_code(&err), Some("AcceptanceModeUnchanged"));
}

// ------------------------------------------------------------------------------------------------
// Removal (rules.yaml#/RestaurantRemoval)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestRestaurantRemoved — rules.yaml#/RestaurantRemoval
#[tokio::test]
async fn removes_a_location_from_its_account() {
    let store = MemStore::default();
    let id = rid();
    let account = RestaurantAccountId(uuid::Uuid::new_v4());
    store.seed(&stream(id), vec![registered_event(id, Some(account), RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    remove_restaurant(
        &store,
        RemoveRestaurant { restaurant_id: id, account_id: account, reason: Some("Delisted".into()) },
        &actor(),
    )
    .await
    .expect("remove");

    let events = store.stream(&stream(id));
    assert!(matches!(&events[1], DomainEvent::RestaurantRemoved(e) if e.account_id == account));

    // And a missing restaurant rejects with RestaurantNotFound (actors.yaml throws).
    let err = remove_restaurant(
        &store,
        RemoveRestaurant { restaurant_id: rid(), account_id: account, reason: None },
        &actor(),
    )
    .await
    .expect_err("missing");
    assert_eq!(rejection_code(&err), Some("RestaurantNotFound"));
}

// ------------------------------------------------------------------------------------------------
// Listing & pre-registration (rules.yaml#/GoogleBusinessProfileEnrichment,
// #/ClosedListingSyncedFromSource, #/ListingClaimRequiresVerifiedOwnership, #/ListingOptOut,
// #/ListingFunnelProgression, #/GbpOrderLinkSetupAndVerification)
// ------------------------------------------------------------------------------------------------

fn seeded(store: &MemStore) -> RestaurantId {
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::NON_PARTNER, "le-saint-honore")]);
    id
}

/// tests.yaml#/cases/TestRestaurantGoogleBusinessProfileUpdated —
/// rules.yaml#/GoogleBusinessProfileEnrichment
#[tokio::test]
async fn records_google_business_profile_enrichment() {
    let store = MemStore::default();
    let id = seeded(&store);

    update_restaurant_google_business_profile(
        &store,
        UpdateRestaurantGoogleBusinessProfile {
            restaurant_id: id,
            google_place_id: Some(GooglePlaceId("ChIJxyz".into())),
            rating: Some(GoogleRating(4.4)),
            reviews_count: Some(18),
        },
        &actor(),
    )
    .await
    .expect("gbp update");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[1],
        DomainEvent::RestaurantGoogleBusinessProfileUpdated(e)
            if e.google_place_id.as_ref().map(|g| g.0.as_str()) == Some("ChIJxyz") && e.reviews_count == Some(18)
    ));
}

/// tests.yaml#/cases/TestRestaurantMarkedClosed — rules.yaml#/ClosedListingSyncedFromSource
#[tokio::test]
async fn marks_a_listing_closed() {
    let store = MemStore::default();
    let id = seeded(&store);

    mark_restaurant_closed(
        &store,
        MarkRestaurantClosed { restaurant_id: id, reason: Some("Sirene: closed".into()) },
        &actor(),
    )
    .await
    .expect("mark closed");

    let events = store.stream(&stream(id));
    assert!(matches!(&events[1], DomainEvent::RestaurantMarkedClosed(_)));
}

/// tests.yaml#/cases/TestRestaurantListingClaimed — rules.yaml#/ListingClaimRequiresVerifiedOwnership
#[tokio::test]
async fn owner_claims_a_listing_with_a_valid_proof() {
    let store = MemStore::default();
    let id = seeded(&store);
    let account = RestaurantAccountId(uuid::Uuid::new_v4());

    claim_restaurant_listing(
        &store,
        &FakeOwnership,
        ClaimRestaurantListing {
            restaurant_id: id,
            account_id: Some(account),
            google_ownership_proof: "valid-gbp-token".into(),
        },
        &actor(),
    )
    .await
    .expect("claim");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[1],
        DomainEvent::RestaurantListingClaimed(e) if e.account_id == Some(account) && e.proof.is_some()
    ));
}

/// tests.yaml#/cases/TestRestaurantListingClaimUnverifiedIsRejected —
/// rules.yaml#/ListingClaimRequiresVerifiedOwnership
#[tokio::test]
async fn rejects_a_claim_whose_proof_fails() {
    let store = MemStore::default();
    let id = seeded(&store);

    let err = claim_restaurant_listing(
        &store,
        &FakeOwnership,
        ClaimRestaurantListing {
            restaurant_id: id,
            account_id: None,
            google_ownership_proof: "bad-token".into(),
        },
        &actor(),
    )
    .await
    .expect_err("bad proof");
    assert_eq!(rejection_code(&err), Some("ListingOwnershipNotVerified"));
    assert_eq!(store.stream(&stream(id)).len(), 1, "no event on rejection");
}

/// tests.yaml#/cases/TestRestaurantListingClaimAgainIsRejected —
/// rules.yaml#/ListingClaimRequiresVerifiedOwnership
#[tokio::test]
async fn rejects_claiming_an_already_claimed_listing() {
    let store = MemStore::default();
    let id = seeded(&store);
    let mut given = store.stream(&stream(id));
    given.push(DomainEvent::RestaurantListingClaimed(RestaurantListingClaimed {
        restaurant_id: id,
        account_id: Some(RestaurantAccountId(uuid::Uuid::new_v4())),
        proof: Some("gbp-proof-ref-1".into()),
    }));
    store.seed(&stream(id), given);

    let err = claim_restaurant_listing(
        &store,
        &FakeOwnership,
        ClaimRestaurantListing {
            restaurant_id: id,
            account_id: None,
            google_ownership_proof: "valid-gbp-token".into(),
        },
        &actor(),
    )
    .await
    .expect_err("already claimed");
    assert_eq!(rejection_code(&err), Some("ListingAlreadyClaimed"));
}

/// tests.yaml#/cases/TestRestaurantListingOptedOut — rules.yaml#/ListingOptOut
#[tokio::test]
async fn owner_opts_a_listing_out_with_a_valid_proof() {
    let store = MemStore::default();
    let id = seeded(&store);

    opt_out_restaurant_listing(
        &store,
        &FakeOwnership,
        OptOutRestaurantListing {
            restaurant_id: id,
            google_ownership_proof: "valid-gbp-token".into(),
            reason: Some("Please remove my listing".into()),
        },
        &actor(),
    )
    .await
    .expect("opt out");

    let events = store.stream(&stream(id));
    assert!(matches!(&events[1], DomainEvent::RestaurantListingOptedOut(_)));

    // And an unverified proof rejects (actors.yaml throws ListingOwnershipNotVerified).
    let err = opt_out_restaurant_listing(
        &store,
        &FakeOwnership,
        OptOutRestaurantListing {
            restaurant_id: id,
            google_ownership_proof: "bad-token".into(),
            reason: None,
        },
        &actor(),
    )
    .await
    .expect_err("bad proof");
    assert_eq!(rejection_code(&err), Some("ListingOwnershipNotVerified"));
}

/// tests.yaml#/cases/TestRestaurantListingStatusChanged — rules.yaml#/ListingFunnelProgression
#[tokio::test]
async fn admin_moves_a_listing_along_the_partnership_funnel() {
    let store = MemStore::default();
    let id = seeded(&store);

    change_restaurant_listing_status(
        &store,
        ChangeRestaurantListingStatus {
            restaurant_id: id,
            listing_status: RestaurantListingStatus::PASSIVE_PARTNER,
            reason: Some("HubRise menu connected".into()),
        },
        &actor(),
    )
    .await
    .expect("change listing status");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[1],
        DomainEvent::RestaurantListingStatusChanged(e)
            if e.listing_status == RestaurantListingStatus::PASSIVE_PARTNER
    ));
}

/// tests.yaml#/cases/TestRestaurantGbpOrderLinkConfigured —
/// rules.yaml#/GbpOrderLinkSetupAndVerification
#[tokio::test]
async fn configures_the_gbp_order_link() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    configure_gbp_order_link(
        &store,
        ConfigureGoogleBusinessProfileOrderLink {
            restaurant_id: id,
            gbp_order_url: WebUrl("https://chez-marco.captain.food".into()),
        },
        &actor(),
    )
    .await
    .expect("configure");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[1],
        DomainEvent::RestaurantGoogleBusinessProfileOrderLinkConfigured(e)
            if e.gbp_order_url.0 == "https://chez-marco.captain.food"
    ));
}

/// tests.yaml#/cases/TestRestaurantGbpOrderLinkVerified —
/// rules.yaml#/GbpOrderLinkSetupAndVerification
#[tokio::test]
async fn verifies_the_configured_gbp_order_link() {
    let store = MemStore::default();
    let id = rid();
    store.seed(
        &stream(id),
        vec![
            registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco"),
            DomainEvent::RestaurantGoogleBusinessProfileOrderLinkConfigured(
                RestaurantGoogleBusinessProfileOrderLinkConfigured {
                    restaurant_id: id,
                    gbp_order_url: WebUrl("https://chez-marco.captain.food".into()),
                },
            ),
        ],
    );

    verify_gbp_order_link(
        &store,
        &FakeProbe,
        VerifyGoogleBusinessProfileOrderLink { restaurant_id: id },
        &actor(),
    )
    .await
    .expect("verify");

    let events = store.stream(&stream(id));
    assert!(matches!(
        &events[2],
        DomainEvent::RestaurantGoogleBusinessProfileOrderLinkVerified(e)
            if e.status == GbpLinkStatus::VERIFIED
    ));
}

/// tests.yaml#/cases/TestRestaurantGbpOrderLinkVerifyIsRejected —
/// rules.yaml#/GbpOrderLinkSetupAndVerification
#[tokio::test]
async fn rejects_verifying_when_no_order_link_is_configured() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    let err = verify_gbp_order_link(
        &store,
        &FakeProbe,
        VerifyGoogleBusinessProfileOrderLink { restaurant_id: id },
        &actor(),
    )
    .await
    .expect_err("no link configured");
    assert_eq!(rejection_code(&err), Some("GbpOrderLinkNotConfigured"));
}

// ------------------------------------------------------------------------------------------------
// Concurrency: a state-changing append at a stale version conflicts instead of double-applying.
// ------------------------------------------------------------------------------------------------

/// Write-side optimistic concurrency (ADR-0035): the handler appends at the version it loaded, so a
/// write that raced past it surfaces the canonical version conflict (not a silent double-apply).
#[tokio::test]
async fn a_concurrent_write_conflicts_at_the_loaded_version() {
    let store = MemStore::default();
    let id = rid();
    store.seed(&stream(id), vec![registered_event(id, None, RestaurantListingStatus::ACTIVE_PARTNER, "chez-marco")]);

    // Simulate the race: another writer advances the stream between load (inside the handler) and
    // append by appending directly at the version the handler will also target.
    let (_, version) = store.load(&stream(id)).await.expect("load");
    store
        .append(&stream(id), version, &[activated_event(id)], &actor())
        .await
        .expect("winner writes first");
    let stale = store.append(&stream(id), version, &[activated_event(id)], &actor()).await;
    assert!(matches!(&stale, Err(e) if application::ports::is_version_conflict(e)));
}
