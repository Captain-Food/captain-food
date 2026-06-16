# Captain.Food — Traceability matrix

End-to-end wiring, so nothing is missed: who triggers what, which handler runs, and what the reads
are backed by. This is a **derived cross-reference** over [commands.yaml](commands.yaml) (`actor`),
[schema.graphql](schema.graphql) (operations + `@auth`), [actors.yaml](actors.yaml) (handlers),
[events.yaml](events.yaml) and [database.md](database.md) (views) — it defines nothing new and can be
regenerated/validated from those files.

Personas map 1:1 to GraphQL paths/roles: `PUBLIC` · `CUSTOMER` · `RESTAURANT` · `RIDER` · `ADMIN` ·
`EXTERNAL` (see [ARCHITECTURE_OVERVIEW.md](ARCHITECTURE_OVERVIEW.md) §8).

---

## 1. Writes — persona → mutation → actor / process-manager

| Persona (path) | GraphQL mutation | Command | Handler (actor) | Emits → |
|---|---|---|---|---|
| ADMIN | `registerRestaurant` | RegisterRestaurant | `Restaurant` (aggregate) | RestaurantRegistered |
| ADMIN | `activateRestaurant` | ActivateRestaurant | `Restaurant` | RestaurantActivated |
| ADMIN | `updateRestaurant` | UpdateRestaurant | `Restaurant` | RestaurantUpdated |
| ADMIN | `deactivateRestaurant` | DeactivateRestaurant | `Restaurant` | RestaurantDeactivated |
| RESTAURANT | `changeAcceptanceMode` | ChangeAcceptanceMode | `Restaurant` | RestaurantAcceptanceModeChanged |
| ADMIN | `createMenu` | CreateMenu | `Menu` (aggregate) | MenuCreated |
| ADMIN | `addProduct` | AddProduct | `Menu` | ProductAdded |
| ADMIN | `updateProduct` / `removeProduct` | UpdateProduct / RemoveProduct | `Menu` | ProductUpdated / ProductRemoved |
| ADMIN | `addCategory` / `updateCategory` / `removeCategory` | Add/Update/RemoveCategory | `Menu` | Category{Added,Updated,Removed} |
| ADMIN | `addOptionList` / `updateOptionList` / `removeOptionList` | Add/Update/RemoveOptionList | `Menu` | OptionList{Added,Updated,Removed} |
| ADMIN | `updateVariantStock` | UpdateVariantStock | `Menu` | VariantStockUpdated |
| ADMIN · EXTERNAL | `importCatalog` | ImportCatalog | `Menu` | CatalogImported |
| PUBLIC (guest) | `addCartLine` | AddCartLine | `Cart` (aggregate) | CartStarted, CartLineAdded |
| PUBLIC (guest) | `removeCartLine` | RemoveCartLine | `Cart` | CartLineRemoved |
| PUBLIC (guest) | `changeCartLineQuantity` | ChangeCartLineQuantity | `Cart` | CartLineQuantityChanged |
| CUSTOMER | `registerCustomer` | RegisterCustomer | `Customer` (aggregate) | CustomerRegistered |
| CUSTOMER | `placeOrder` | PlaceOrder | `PlaceOrderProcess` (saga) → `Order`, `Cart` | PaymentIntentCreated → OrderPlaced + CartCheckedOut |
| RESTAURANT | `acceptOrder` | AcceptOrder | `Order` (aggregate) | OrderAcceptedByRestaurant |
| RESTAURANT | `rejectOrder` | RejectOrder | `Order` → `RefundProcess` | OrderRejectedByRestaurant (+ refund) |
| RESTAURANT | `startPreparation` | StartPreparation | `Order` | OrderPreparationStarted |
| RESTAURANT | `markOrderReady` | MarkOrderReady | `Order` | OrderMarkedReady |
| RESTAURANT · RIDER | `markOrderDelivered` | MarkOrderDelivered | `Order` | OrderDelivered |
| CUSTOMER | `cancelOrderByCustomer` | CancelOrderByCustomer | `Order` → `RefundProcess` | OrderCancelledByCustomer (+ refund) |
| RESTAURANT | `cancelOrderByRestaurant` | CancelOrderByRestaurant | `Order` → `RefundProcess` | OrderCancelledByRestaurant (+ refund) |

`@public` mutations (the cart) appear in the PUBLIC schema and therefore also in every authenticated
path-schema — a logged-in CUSTOMER edits the cart through the same operations.

---

## 2. Reads — persona → query → resolver → view (driven by UI)

Resolvers read **only** `View_*` projection tables (never `domain_events`). See [database.md](database.md).

| Persona (path) | GraphQL query | Read view(s) | UI / expectation |
|---|---|---|---|
| PUBLIC (+ all) | `restaurants` | `View_RestaurantsPublic` (status = ACTIVE) | Discover list — §3.1 |
| PUBLIC (+ all) | `restaurant(slug)` | `View_RestaurantsPublic` (header) + `View_RestaurantMenu` | Restaurant page + menu — §3.2 / §3.3 |
| PUBLIC (+ all) | `cart(id)` | `View_Cart` (priced) | Cart panel — §3.3 |
| CUSTOMER · RESTAURANT · ADMIN | `order(id)` | `View_OrderTracking` | Order tracking / single-order view — §3.6 |
| RESTAURANT · ADMIN | `ordersByRestaurant` | `View_OrdersByRestaurant` | Back-office order queue |
| CUSTOMER *(V1)* | *(future)* `ordersByCustomer` | `View_OrdersByCustomer` | Order history — post-V0 |

Ownership ("this is *my* order") is enforced in the resolver/command layer, not by the path alone.

---

## 3. External systems → process-manager → actors

External systems never call aggregates directly; they hit the ACL (the `/external/graphql` path or a
REST webhook), which feeds a process-manager that drives the aggregates.

| External system | Channel | Inbound event(s) / op | Process-manager | Resulting actor → event |
|---|---|---|---|---|
| Stripe | REST webhook (Stripe-controlled payload) | 📥 `PaymentCaptured` / `PaymentFailed` | `PlaceOrderProcess` | on capture: `Order` ← OrderPlaced, `Cart` ← CartCheckedOut; on failure: none (cart stays OPEN) |
| Stripe | REST webhook | 📥 `PaymentRefunded` | `RefundProcess` | records the settled refund (requested by Reject/Cancel commands) |
| HubRise | `/external/graphql` (ACL) | `importCatalog` command | — (validated command) | `Menu` ← CatalogImported |
| HubRise | `/external/graphql` (ACL) | 📥 `VariantStockUpdated` (inventory sync) | — (recorded via ACL) | `Menu` ← VariantStockUpdated |
| Avelo37 (delivery) *(post-V0)* | `/external/graphql` or webhook | 📥 `DeliveryStatusUpdated` / `DeliveryAcceptedByPartner` | (delivery process — TBD) | `Order` / `DeliveryJob` (payloads not yet in events.yaml) |

Note the request/report split: a refund is **requested** by a command (`RejectOrder` / `CancelOrder*`)
but the `PaymentRefunded` **fact** is **reported** by Stripe (inbound). `ImportCatalog` stays a
**command** (the ACL can reject it), whereas the HubRise inventory sync is a bare inbound event.

---

## 4. Coverage checklist

- **Every command** in [commands.yaml](commands.yaml) has (a) a GraphQL mutation in
  [schema.graphql](schema.graphql) and (b) a handler in [actors.yaml](actors.yaml) — §1 above.
- **Every query** in [schema.graphql](schema.graphql) has a backing `View_*` in
  [database.md](database.md) and a named UI expectation — §2 above.
- **Every inbound (📥) event** has a consuming process-manager — §3 above.
- **Every event** emitted by an actor is consumed by at least one `View_*` projection
  (see the per-view "Fed by" lists in [database.md](database.md)).

Gaps deliberately open (see [story-map.md](story-map.md) "Gaps to resolve"): delivery-partner event
payloads (post-V0), `View_OrdersByCustomer` query (V1), and the `/external` ingestion ops beyond
`importCatalog` (HubRise inventory / Avelo delivery as first-class external mutations).
