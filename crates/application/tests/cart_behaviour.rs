//! BEHAVIOUR tests for the Cart aggregate — the executable form of the `specs/tests.yaml`
//! Given/When/Then cases whose `when` is a Cart-aggregate command (ADR-0032: each test cites the
//! `specs/rules.yaml` rule it asserts). Given = pre-seeded stream events (in-memory event store),
//! When = the command handler, Then = the emitted event(s) / the errors.yaml rejection code.
//!
//! Pure and offline: an in-memory [`EventStore`] plus a fake `CatalogReadRepository` whose row is
//! built by folding catalog events through the REAL `CatalogProjector` tree fold — so the live-catalog
//! line invariants (OfferNotFound / OfferUnavailable / InsufficientStock / InvalidOptionSelection) are
//! asserted against exactly what the projection worker would materialize. `CartNotFound` is
//! unreachable for AddCartLine by construction (create-on-first-add) and is asserted on remove/change
//! instead.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use application::commands::{
    add_cart_line, bind_cart_to_customer, change_cart_line_quantity, rejection_code,
    remove_cart_line,
};
use application::ports::{version_conflict, Actor, EventStore};
use application::projections::{project_catalog, Envelope};
use application::projectors::catalog::CatalogProjector;
use application::queries::{CatalogReadRepository, CatalogRow};
use domain::cart::MAX_LINE_QUANTITY;
use domain::generated::commands::{
    AddCartLine, BindCartToCustomer, CartLine, ChangeCartLineQuantity, RemoveCartLine,
};
use domain::generated::entities::{
    CartLineItem, Money, Offer, OptionList, Product, ProductItemOption, Stock, TaxRate,
};
use domain::generated::events::{
    CartBoundToCustomer, CartCheckedOut, CartLineAdded, CartStarted, CatalogCreated, DomainEvent,
    OptionListAdded, ProductAdded,
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

/// Fake Catalog read model: at most one projected row (built through the REAL `CatalogProjector`
/// fold — see [`projected_catalog`]); `offer_by_id` runs the trait's provided tree walk, exactly the
/// code path `PgCatalogRepository` uses.
#[derive(Default)]
struct FakeCatalogs {
    row: Option<CatalogRow>,
}

#[async_trait]
impl CatalogReadRepository for FakeCatalogs {
    async fn by_restaurant(
        &self,
        restaurant_id: RestaurantId,
    ) -> Result<Option<CatalogRow>, DomainError> {
        Ok(self.row.clone().filter(|r| r.restaurant_id == restaurant_id))
    }
}

// ------------------------------------------------------------------------------------------------
// Fixtures (tests.yaml `fixtures`, with UUIDs instead of the sample string ids)
// ------------------------------------------------------------------------------------------------

fn actor() -> Actor {
    Actor {
        user_id: uuid::Uuid::new_v4(),
        user_type: 0, // UserType::PUBLIC ordinal — carts are built by (possibly guest) visitors
        correlation_id: uuid::Uuid::new_v4(),
        cause_id: None,
    }
}

fn stream(id: CartId) -> String {
    format!("Cart-{}", id.0)
}

/// Fixture `cartStarted`.
fn sid() -> SessionId {
    SessionId(uuid::Uuid::new_v4())
}

fn cart_started(cart_id: CartId, restaurant_id: RestaurantId) -> DomainEvent {
    DomainEvent::CartStarted(CartStarted { cart_id, restaurant_id, session_id: sid(), customer_id: None })
}

/// Fixture `cartLineAdded` (quantity 2, no options).
fn cart_line_added(cart_id: CartId, cart_line_id: CartLineId, offer_id: OfferId) -> DomainEvent {
    DomainEvent::CartLineAdded(CartLineAdded {
        cart_id,
        line: CartLineItem { cart_line_id, offer_id, quantity: 2, selected_option_ids: vec![] },
    })
}

/// Fixture `cartCheckedOut` — closes the cart (status CHECKED_OUT).
fn cart_checked_out(cart_id: CartId) -> DomainEvent {
    DomainEvent::CartCheckedOut(CartCheckedOut { cart_id, order_id: OrderId(uuid::Uuid::new_v4()) })
}

fn add_cmd(
    cart_id: CartId,
    restaurant_id: RestaurantId,
    line_id: CartLineId,
    offer_id: OfferId,
    quantity: i64,
) -> AddCartLine {
    AddCartLine {
        cart_id,
        restaurant_id,
        session_id: sid(),
        line: CartLine {
            cart_line_id: line_id,
            offer_id,
            quantity,
            selected_option_ids: vec![],
        },
    }
}

fn cid() -> CartId {
    CartId(uuid::Uuid::new_v4())
}
fn rid() -> RestaurantId {
    RestaurantId(uuid::Uuid::new_v4())
}
fn lid() -> CartLineId {
    CartLineId(uuid::Uuid::new_v4())
}
fn oid() -> OfferId {
    OfferId(uuid::Uuid::new_v4())
}

// ------------------------------------------------------------------------------------------------
// Catalog fixtures — the read model the line checks run against, materialized by the REAL
// `CatalogProjector` tree fold (so these tests also exercise the projection, ADR-0040).
// ------------------------------------------------------------------------------------------------

/// Fold catalog events through the generated dispatch + the real Compute impl into a `CatalogRow`,
/// exactly like the projection worker does.
fn projected_catalog(events: Vec<DomainEvent>) -> CatalogRow {
    let mut row = None;
    for (i, event) in events.into_iter().enumerate() {
        let env = Envelope {
            stream_name: "Catalog-test".into(),
            position: i as i64 + 1,
            occurred_at: chrono::Utc::now(),
            event,
        };
        row = project_catalog(&CatalogProjector, row, &env);
    }
    row.expect("catalog projected")
}

fn tracked_stock(quantity: f64) -> Stock {
    Stock {
        quantity: Quantity(quantity),
        low_stock_threshold: None,
        status: StockStatus::IN_STOCK, // carried value is ignored — the projector re-derives it
        expires_at: None,
    }
}

fn offer_fixture(
    offer_id: OfferId,
    availability: CatalogItemAvailability,
    stock: Option<Stock>,
    option_list_ids: Vec<OptionListId>,
) -> Offer {
    Offer {
        id: offer_id,
        r#ref: None,
        product_id: ProductId(uuid::Uuid::new_v4()),
        name: OfferName("Regular".into()),
        price: Money { amount_cents: MoneyCents(980), currency: CurrencyCode("EUR".into()) },
        availability,
        stock,
        option_list_ids,
    }
}

fn option_fixture(option_id: OptionId, option_list_id: OptionListId) -> ProductItemOption {
    ProductItemOption {
        id: option_id,
        r#ref: None,
        option_list_id,
        name: OptionName("Extra".into()),
        price: Money { amount_cents: MoneyCents(100), currency: CurrencyCode("EUR".into()) },
        r#default: false,
        availability: CatalogItemAvailability::AVAILABLE,
        stock: None,
    }
}

/// A catalog for `restaurant_id` holding one product with `offer` (+ optional option lists),
/// projected through the real tree fold.
fn catalog_with(
    restaurant_id: RestaurantId,
    offer: Offer,
    option_lists: Vec<OptionList>,
) -> FakeCatalogs {
    let catalog_id = CatalogId(uuid::Uuid::new_v4());
    let product = Product {
        id: offer.product_id,
        r#ref: None,
        catalog_id,
        restaurant_id,
        category_ref: None,
        name: ProductName("Margherita".into()),
        description: None,
        tags: vec![],
        image_ids: vec![],
        tax_rate: TaxRate { delivery: TaxRatePercent(10.0), collection: None, eat_in: None },
        offers: vec![offer],
    };
    let mut events = vec![
        DomainEvent::CatalogCreated(CatalogCreated {
            catalog_id,
            r#ref: None,
            restaurant_id,
            name: CatalogName("Main menu".into()),
        }),
        DomainEvent::ProductAdded(ProductAdded { catalog_id, restaurant_id, product }),
    ];
    for option_list in option_lists {
        events.push(DomainEvent::OptionListAdded(OptionListAdded {
            catalog_id,
            restaurant_id,
            option_list,
        }));
    }
    FakeCatalogs { row: Some(projected_catalog(events)) }
}

/// The default orderable catalog: `offer_id` AVAILABLE and not stock-tracked (never blocks).
fn orderable_catalog(restaurant_id: RestaurantId, offer_id: OfferId) -> FakeCatalogs {
    catalog_with(
        restaurant_id,
        offer_fixture(offer_id, CatalogItemAvailability::AVAILABLE, None, vec![]),
        vec![],
    )
}

// ------------------------------------------------------------------------------------------------
// Adding lines (rules.yaml#/CartPricedFromLiveCatalog, #/CartRejectsUnorderableOrInvalidLine)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestCartFirstLineAdded — rules.yaml#/CartPricedFromLiveCatalog
#[tokio::test]
async fn adds_the_first_line_creating_the_cart() {
    let store = MemStore::default();
    let (cart, resto, line, offer) = (cid(), rid(), lid(), oid());
    let catalogs = orderable_catalog(resto, offer);

    add_cart_line(&store, &catalogs, add_cmd(cart, resto, line, offer, 2), &actor())
        .await
        .expect("first add");

    let events = store.stream(&stream(cart));
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        DomainEvent::CartStarted(e) if e.restaurant_id == resto && e.customer_id.is_none()
    ));
    assert!(matches!(
        &events[1],
        DomainEvent::CartLineAdded(e) if e.line.cart_line_id == line && e.line.quantity == 2
    ));
}

/// Client-generated line ids: re-sending a line the cart already holds is an idempotent replay
/// (no duplicate fact) — rules.yaml#/CartPricedFromLiveCatalog.
#[tokio::test]
async fn re_adding_the_same_line_is_a_no_op() {
    let store = MemStore::default();
    let (cart, resto, line, offer) = (cid(), rid(), lid(), oid());
    let catalogs = orderable_catalog(resto, offer);
    store.seed(&stream(cart), vec![cart_started(cart, resto), cart_line_added(cart, line, offer)]);

    add_cart_line(&store, &catalogs, add_cmd(cart, resto, line, offer, 2), &actor())
        .await
        .expect("replay absorbed");
    assert_eq!(store.stream(&stream(cart)).len(), 2, "no duplicate fact");
}

/// tests.yaml#/cases/TestCartAddLineIsRejectedWhenCartInvalid (CartNotOpen / CartRestaurantMismatch /
/// QuantityExceedsLimit arms) — rules.yaml#/CartRejectsUnorderableOrInvalidLine. The CartNotFound arm
/// is unreachable for AddCartLine (create-on-first-add); the InvalidOptionSelection arm is asserted in
/// `rejects_an_invalid_option_selection`. Cart-state invariants fire BEFORE the catalog lookups, so an
/// empty catalog read model never masks them.
#[tokio::test]
async fn rejects_adding_on_a_closed_or_mismatched_cart_or_over_the_limit() {
    let store = MemStore::default();
    let catalogs = FakeCatalogs::default();

    // Checked-out cart → CartNotOpen.
    let (cart, resto) = (cid(), rid());
    store.seed(&stream(cart), vec![cart_started(cart, resto), cart_checked_out(cart)]);
    let err = add_cart_line(&store, &catalogs, add_cmd(cart, resto, lid(), oid(), 1), &actor())
        .await
        .expect_err("closed");
    assert_eq!(rejection_code(&err), Some("CartNotOpen"));

    // Another restaurant's line on an open cart → CartRestaurantMismatch (no mixing).
    let (cart, resto) = (cid(), rid());
    store.seed(&stream(cart), vec![cart_started(cart, resto)]);
    let err = add_cart_line(&store, &catalogs, add_cmd(cart, rid(), lid(), oid(), 1), &actor())
        .await
        .expect_err("mismatch");
    assert_eq!(rejection_code(&err), Some("CartRestaurantMismatch"));
    assert_eq!(store.stream(&stream(cart)).len(), 1, "no event on rejection");

    // Over the per-line cap → QuantityExceedsLimit.
    let err = add_cart_line(
        &store,
        &catalogs,
        add_cmd(cid(), rid(), lid(), oid(), MAX_LINE_QUANTITY + 1),
        &actor(),
    )
    .await
    .expect_err("over limit");
    assert_eq!(rejection_code(&err), Some("QuantityExceedsLimit"));
}

/// tests.yaml#/cases/TestCartAddLineIsRejectedWhenOfferNotOrderable (OfferNotFound / OfferUnavailable /
/// InsufficientStock arms, via the offer-level Catalog read port over the projected tree) —
/// rules.yaml#/CartRejectsUnorderableOrInvalidLine
#[tokio::test]
async fn rejects_adding_an_unknown_unavailable_or_out_of_stock_offer() {
    let store = MemStore::default();
    let (resto, offer) = (rid(), oid());

    // Unknown offer (not in the restaurant's catalog — here: no catalog at all) → OfferNotFound.
    let err = add_cart_line(
        &store,
        &FakeCatalogs::default(),
        add_cmd(cid(), resto, lid(), offer, 1),
        &actor(),
    )
    .await
    .expect_err("unknown offer");
    assert_eq!(rejection_code(&err), Some("OfferNotFound"));

    // A catalog exists but holds a DIFFERENT offer → still OfferNotFound.
    let err = add_cart_line(
        &store,
        &orderable_catalog(resto, oid()),
        add_cmd(cid(), resto, lid(), offer, 1),
        &actor(),
    )
    .await
    .expect_err("other offer");
    assert_eq!(rejection_code(&err), Some("OfferNotFound"));

    // Manual availability flag UNAVAILABLE → OfferUnavailable (availability ≠ stock).
    let catalogs = catalog_with(
        resto,
        offer_fixture(offer, CatalogItemAvailability::UNAVAILABLE, Some(tracked_stock(10.0)), vec![]),
        vec![],
    );
    let err = add_cart_line(&store, &catalogs, add_cmd(cid(), resto, lid(), offer, 1), &actor())
        .await
        .expect_err("unavailable");
    assert_eq!(rejection_code(&err), Some("OfferUnavailable"));

    // Stock-tracked at 0 → InsufficientStock.
    let catalogs = catalog_with(
        resto,
        offer_fixture(offer, CatalogItemAvailability::AVAILABLE, Some(tracked_stock(0.0)), vec![]),
        vec![],
    );
    let err = add_cart_line(&store, &catalogs, add_cmd(cid(), resto, lid(), offer, 1), &actor())
        .await
        .expect_err("out of stock");
    assert_eq!(rejection_code(&err), Some("InsufficientStock"));

    // Requesting more than the tracked stock covers → InsufficientStock; within it → accepted.
    let catalogs = catalog_with(
        resto,
        offer_fixture(offer, CatalogItemAvailability::AVAILABLE, Some(tracked_stock(2.0)), vec![]),
        vec![],
    );
    let err = add_cart_line(&store, &catalogs, add_cmd(cid(), resto, lid(), offer, 3), &actor())
        .await
        .expect_err("beyond stock");
    assert_eq!(rejection_code(&err), Some("InsufficientStock"));
    add_cart_line(&store, &catalogs, add_cmd(cid(), resto, lid(), offer, 2), &actor())
        .await
        .expect("within stock");
}

/// tests.yaml#/cases/TestCartAddLineIsRejectedWhenCartInvalid (InvalidOptionSelection arm: selected
/// options ∈ the offer's option lists, within minSelections/maxSelections, duplicates only when
/// multipleSelection) — rules.yaml#/CartRejectsUnorderableOrInvalidLine
#[tokio::test]
async fn rejects_an_invalid_option_selection() {
    let store = MemStore::default();
    let (resto, offer) = (rid(), oid());
    let list_id = OptionListId(uuid::Uuid::new_v4());
    let (opt_a, opt_b) = (OptionId(uuid::Uuid::new_v4()), OptionId(uuid::Uuid::new_v4()));
    // "Sauces": pick EXACTLY one (min 1, max 1), no duplicate picks.
    let sauces = OptionList {
        id: list_id,
        r#ref: None,
        name: OptionListName("Sauces".into()),
        min_selections: 1,
        max_selections: Some(1),
        multiple_selection: false,
        options: vec![option_fixture(opt_a, list_id), option_fixture(opt_b, list_id)],
    };
    let catalogs = catalog_with(
        resto,
        offer_fixture(offer, CatalogItemAvailability::AVAILABLE, None, vec![list_id]),
        vec![sauces],
    );
    let cmd_with = |selected: Vec<OptionId>| {
        let mut cmd = add_cmd(cid(), resto, lid(), offer, 1);
        cmd.line.selected_option_ids = selected;
        cmd
    };

    // An option from NO list of this offer → InvalidOptionSelection.
    let foreign = OptionId(uuid::Uuid::new_v4());
    let err = add_cart_line(&store, &catalogs, cmd_with(vec![foreign]), &actor())
        .await
        .expect_err("foreign option");
    assert_eq!(rejection_code(&err), Some("InvalidOptionSelection"));

    // Under minSelections (none picked) → InvalidOptionSelection.
    let err = add_cart_line(&store, &catalogs, cmd_with(vec![]), &actor())
        .await
        .expect_err("under min");
    assert_eq!(rejection_code(&err), Some("InvalidOptionSelection"));

    // Over maxSelections (two picked) → InvalidOptionSelection.
    let err = add_cart_line(&store, &catalogs, cmd_with(vec![opt_a, opt_b]), &actor())
        .await
        .expect_err("over max");
    assert_eq!(rejection_code(&err), Some("InvalidOptionSelection"));

    // A duplicate pick without multipleSelection → InvalidOptionSelection (also over max here).
    let err = add_cart_line(&store, &catalogs, cmd_with(vec![opt_a, opt_a]), &actor())
        .await
        .expect_err("duplicate");
    assert_eq!(rejection_code(&err), Some("InvalidOptionSelection"));

    // A valid selection is accepted.
    add_cart_line(&store, &catalogs, cmd_with(vec![opt_b]), &actor()).await.expect("valid selection");
}

// ------------------------------------------------------------------------------------------------
// Changing / removing lines (rules.yaml#/CartPricedFromLiveCatalog,
// #/CartRejectsUnorderableOrInvalidLine)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestCartLineQuantityChanged — rules.yaml#/CartPricedFromLiveCatalog
#[tokio::test]
async fn changes_the_quantity_of_an_existing_line() {
    let store = MemStore::default();
    let (cart, resto, line, offer) = (cid(), rid(), lid(), oid());
    let catalogs = orderable_catalog(resto, offer);
    store.seed(&stream(cart), vec![cart_started(cart, resto), cart_line_added(cart, line, offer)]);

    change_cart_line_quantity(
        &store,
        &catalogs,
        ChangeCartLineQuantity { cart_id: cart, cart_line_id: line, quantity: 3, session_id: sid() },
        &actor(),
    )
    .await
    .expect("change quantity");

    let events = store.stream(&stream(cart));
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[2],
        DomainEvent::CartLineQuantityChanged(e) if e.cart_line_id == line && e.quantity == 3
    ));
}

/// tests.yaml#/cases/TestCartLineRemoved — rules.yaml#/CartPricedFromLiveCatalog
#[tokio::test]
async fn removes_a_line_from_the_cart() {
    let store = MemStore::default();
    let (cart, resto, line) = (cid(), rid(), lid());
    store.seed(&stream(cart), vec![cart_started(cart, resto), cart_line_added(cart, line, oid())]);

    remove_cart_line(&store, RemoveCartLine { cart_id: cart, cart_line_id: line, session_id: sid() }, &actor())
        .await
        .expect("remove line");

    let events = store.stream(&stream(cart));
    assert!(matches!(&events[2], DomainEvent::CartLineRemoved(e) if e.cart_line_id == line));
}

/// tests.yaml#/cases/TestCartRemoveLineIsRejected (all three arms) —
/// rules.yaml#/CartRejectsUnorderableOrInvalidLine
#[tokio::test]
async fn rejects_removing_from_a_missing_or_closed_cart_or_a_missing_line() {
    let store = MemStore::default();

    // Missing cart → CartNotFound.
    let err = remove_cart_line(&store, RemoveCartLine { cart_id: cid(), cart_line_id: lid(), session_id: sid() }, &actor())
        .await
        .expect_err("missing cart");
    assert_eq!(rejection_code(&err), Some("CartNotFound"));

    // Checked-out cart → CartNotOpen.
    let (cart, resto, line) = (cid(), rid(), lid());
    store.seed(
        &stream(cart),
        vec![cart_started(cart, resto), cart_line_added(cart, line, oid()), cart_checked_out(cart)],
    );
    let err = remove_cart_line(&store, RemoveCartLine { cart_id: cart, cart_line_id: line, session_id: sid() }, &actor())
        .await
        .expect_err("closed cart");
    assert_eq!(rejection_code(&err), Some("CartNotOpen"));

    // Open cart, unknown line → CartLineNotFound.
    let (cart, resto) = (cid(), rid());
    store.seed(&stream(cart), vec![cart_started(cart, resto)]);
    let err = remove_cart_line(&store, RemoveCartLine { cart_id: cart, cart_line_id: lid(), session_id: sid() }, &actor())
        .await
        .expect_err("missing line");
    assert_eq!(rejection_code(&err), Some("CartLineNotFound"));
    assert_eq!(store.stream(&stream(cart)).len(), 1, "no event on rejection");
}

/// ChangeCartLineQuantity rejection arms (actors.yaml throws: CartNotFound / CartLineNotFound /
/// QuantityExceedsLimit / InsufficientStock — the stock re-check runs against the line's offer in the
/// LIVE catalog; an offer that has since left the catalog does not block, checkout re-validates) —
/// rules.yaml#/CartRejectsUnorderableOrInvalidLine
#[tokio::test]
async fn rejects_changing_quantity_on_invalid_cart_line_over_the_limit_or_beyond_stock() {
    let store = MemStore::default();
    let catalogs = FakeCatalogs::default();

    let err = change_cart_line_quantity(
        &store,
        &catalogs,
        ChangeCartLineQuantity { cart_id: cid(), cart_line_id: lid(), quantity: 1, session_id: sid() },
        &actor(),
    )
    .await
    .expect_err("missing cart");
    assert_eq!(rejection_code(&err), Some("CartNotFound"));

    let (cart, resto, line, offer) = (cid(), rid(), lid(), oid());
    store.seed(&stream(cart), vec![cart_started(cart, resto), cart_line_added(cart, line, offer)]);

    let err = change_cart_line_quantity(
        &store,
        &catalogs,
        ChangeCartLineQuantity { cart_id: cart, cart_line_id: lid(), quantity: 1, session_id: sid() },
        &actor(),
    )
    .await
    .expect_err("missing line");
    assert_eq!(rejection_code(&err), Some("CartLineNotFound"));

    let err = change_cart_line_quantity(
        &store,
        &catalogs,
        ChangeCartLineQuantity { cart_id: cart, cart_line_id: line, quantity: MAX_LINE_QUANTITY + 1, session_id: sid() },
        &actor(),
    )
    .await
    .expect_err("over limit");
    assert_eq!(rejection_code(&err), Some("QuantityExceedsLimit"));

    // The new quantity exceeds the offer's live tracked stock → InsufficientStock.
    let low_stock = catalog_with(
        resto,
        offer_fixture(offer, CatalogItemAvailability::AVAILABLE, Some(tracked_stock(2.0)), vec![]),
        vec![],
    );
    let err = change_cart_line_quantity(
        &store,
        &low_stock,
        ChangeCartLineQuantity { cart_id: cart, cart_line_id: line, quantity: 3, session_id: sid() },
        &actor(),
    )
    .await
    .expect_err("beyond stock");
    assert_eq!(rejection_code(&err), Some("InsufficientStock"));
    assert_eq!(store.stream(&stream(cart)).len(), 2, "no event on rejection");

    // An offer that has since LEFT the catalog does not block the change (no OfferNotFound declared).
    change_cart_line_quantity(
        &store,
        &catalogs,
        ChangeCartLineQuantity { cart_id: cart, cart_line_id: line, quantity: 3, session_id: sid() },
        &actor(),
    )
    .await
    .expect("offer gone from catalog — change still recorded");
}

// ------------------------------------------------------------------------------------------------
// Customer binding (rules.yaml#/GuestCartsBoundOnIdentification)
// ------------------------------------------------------------------------------------------------

/// tests.yaml#/cases/TestCartBoundToCustomer — rules.yaml#/GuestCartsBoundOnIdentification
#[tokio::test]
async fn a_guest_cart_is_bound_to_the_identified_customer_one_time() {
    let store = MemStore::default();
    let (cart, resto) = (CartId(uuid::Uuid::new_v4()), RestaurantId(uuid::Uuid::new_v4()));
    let customer = CustomerId(uuid::Uuid::new_v4());
    store.seed(&stream(cart), vec![cart_started(cart, resto)]);

    bind_cart_to_customer(
        &store,
        BindCartToCustomer { cart_id: cart, customer_id: customer },
        &actor(),
    )
    .await
    .expect("bind");

    let events = store.stream(&stream(cart));
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[1],
        DomainEvent::CartBoundToCustomer(e) if e.customer_id == customer
    ));

    // Re-delivering the SAME bind is an idempotent no-op (no duplicate fact).
    bind_cart_to_customer(
        &store,
        BindCartToCustomer { cart_id: cart, customer_id: customer },
        &actor(),
    )
    .await
    .expect("idempotent replay");
    assert_eq!(store.stream(&stream(cart)).len(), 2, "no duplicate fact");

    // The bind is ONE-TIME, first wins: a DIFFERENT customer is also a no-op — the earlier bind
    // stands and is never overwritten (nothing to reject; no error declared for it).
    let other = CustomerId(uuid::Uuid::new_v4());
    bind_cart_to_customer(&store, BindCartToCustomer { cart_id: cart, customer_id: other }, &actor())
        .await
        .expect("first wins — silent no-op");
    let events = store.stream(&stream(cart));
    assert_eq!(events.len(), 2, "no re-bind fact");
    assert!(matches!(
        &events[1],
        DomainEvent::CartBoundToCustomer(e) if e.customer_id == customer
    ));

    // A missing cart rejects with CartNotFound (the only declared throw).
    let err = bind_cart_to_customer(
        &store,
        BindCartToCustomer { cart_id: CartId(uuid::Uuid::new_v4()), customer_id: customer },
        &actor(),
    )
    .await
    .expect_err("missing cart");
    assert_eq!(rejection_code(&err), Some("CartNotFound"));
}
