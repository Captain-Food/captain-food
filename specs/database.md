## 1. Event store table

PostgreSQL table definition (conceptual):

```sql
CREATE TABLE domain_events (
  id              UUID PRIMARY KEY,
  aggregate_type  TEXT NOT NULL,
  aggregate_name  TEXT NOT NULL, // to indicate the business name of the aggregate
  aggregate_id    UUID NOT NULL,
  user_id         UUID NOT NULL,
  user_type       INT NOT NULL,
  correlation_id  UUID NOT NULL,
  cause_id        UUID NULL,
  version         INT  NOT NULL,
  type            TEXT NOT NULL,
  payload         JSONB NOT NULL,
  metadata        JSONB,
  occurred_at     TIMESTAMPTZ NOT NULL, // could be interesting if we use integer
  expired_at      TIMESTAMPTZ NULL
);

CREATE INDEX ON domain_events (aggregate_type, aggregate_id, version);
CREATE INDEX ON domain_events (type);
CREATE INDEX ON domain_events (occurred_at);
```

- `aggregate_type`: `'restaurant' | 'menu' | 'customer' | 'cart' | 'order' | 'delivery_job'`
  (matches the aggregates in [actors.yaml](actors.yaml)).
- `version`: monotonic per `(aggregate_type, aggregate_id)`; we can use it for optimistic concurrency later.
- `metadata`: optional (user id, correlation id, source, etc.).

## 2. Read models — projection views (`View_*`)

Queries **never** read `domain_events`; they read dedicated read tables fed by projections that
consume events. These read tables are **"fake" tables** (denormalized, query-shaped, rebuildable
from the log) — to avoid any confusion with a real/normalized table, every one is prefixed
**`View_`** (`View_{TableName}`).

Each view below is **driven by the UI/query that consumes it** ([PRODUCT_SPEC_WEB_CLIENT.md](PRODUCT_SPEC_WEB_CLIENT.md)
+ [schema.graphql](schema.graphql)), and **fed by the events** of the aggregate(s) it projects
([events.yaml](events.yaml) / [actors.yaml](actors.yaml)). Money is stored as integer minor units
(`*_cents` + `currency`), matching `Money`. `JSONB` is used where the UI fetches a whole sub-tree at once.

### `View_RestaurantsPublic`
- **Serves**: `Query.restaurants` (discover list — `status = ACTIVE` only) and the header of
  `Query.restaurant(slug)`. UI: §3.1 / §3.2.
- **Aggregate**: `Restaurant`. **Fed by**: `RestaurantRegistered`, `RestaurantUpdated`,
  `RestaurantActivated`, `RestaurantDeactivated`, `RestaurantAcceptanceModeChanged`.
- **Columns**: `restaurant_id` (PK), `slug` (unique), `display_name`, `description`, `tags` JSONB,
  `address` JSONB, `opening_hours` JSONB, `status`, `order_acceptance`, `default_currency`,
  `preparation_time_minutes`, `updated_at`.

### `View_RestaurantMenu`
- **Serves**: the `menus` field of `Query.restaurant(slug)`. UI: §3.2 / §3.3 (browse & build cart).
- **Aggregate**: `Menu`. **Fed by**: `MenuCreated`, `Category*`, `Product*`, `OptionList*`,
  `VariantStockUpdated`, `CatalogImported`.
- **Columns**: `menu_id` (PK), `restaurant_id` (index), `slug`, `name`, `catalog` JSONB — the
  assembled tree: categories → products → variants `{ price_cents, currency, availability,
  stock_status }` + option lists. `updated_at`.
- **Note**: `stock_status` is derived (quantity vs `lowStockThreshold`); orderable = `AVAILABLE` **and**
  stock > 0. Could be normalized (one row per variant) if per-item querying is needed later.

### `View_Cart`
- **Serves**: `Query.cart(id)`. UI: §3.3 cart panel (priced).
- **Aggregate**: `Cart` (joined with the catalog for pricing). **Fed by**: `CartStarted`,
  `CartLineAdded`, `CartLineQuantityChanged`, `CartLineRemoved`, `CartCheckedOut`.
- **Columns**: `cart_id` (PK), `restaurant_id`, `customer_id` (NULL while guest), `status`,
  `lines` JSONB `[{ cart_line_id, variant_id, product_id, name, variant_name, quantity,
  unit_price_cents, selected_options, line_total_cents }]`, `total_amount_cents`, `currency`, `updated_at`.
- **Note**: prices are computed by the projection from the current catalog, never trusted from the client.

### `View_OrderTracking`
- **Serves**: `Query.order(id)` (customer tracking + restaurant/admin single-order view). UI: §3.6.
- **Aggregate**: `Order` (+ payment facts). **Fed by**: `OrderPlaced`, `OrderAcceptedByRestaurant`,
  `OrderPreparationStarted`, `OrderMarkedReady`, `OrderDelivered`, `OrderRejectedByRestaurant`,
  `OrderCancelledByCustomer`, `OrderCancelledByRestaurant`, `PaymentCaptured`, `PaymentRefunded`.
- **Columns**: `order_id` (PK), `ref`, `restaurant_id`, `customer_id` (NULL), `status`,
  `service_type`, `items` JSONB, `total_amount_cents`, `currency`, `delivery_address` JSONB,
  `estimated_ready_at`, `placed_at`, `status_changed_at`, `payment_status`.

### `View_OrdersByRestaurant`
- **Serves**: `Query.ordersByRestaurant` (back-office queue). UI: web-admin / future restaurant app.
- **Aggregate**: `Order`. **Fed by**: the same order lifecycle events as `View_OrderTracking`.
- **Columns**: `order_id` (PK), `restaurant_id` (index), `status`, `service_type`,
  `customer_display_name`, `total_amount_cents`, `currency`, `placed_at`, `accepted_at`,
  `estimated_ready_at`. Index `(restaurant_id, status, placed_at)`.

### `View_OrdersByCustomer`  *(V1)*
- **Serves**: a future customer order-history list (post-V0; no query in V0 schema yet).
- **Aggregate**: `Order`. **Fed by**: the order lifecycle events.
- **Columns**: `customer_id` (index), `order_id`, `restaurant_id`, `restaurant_display_name`,
  `status`, `total_amount_cents`, `currency`, `placed_at`.

> A projection may consume events from **more than one aggregate** (e.g. `View_OrderTracking` folds
> `Order` lifecycle + Stripe payment facts; `View_Cart`/`View_RestaurantMenu` price against the
> catalog). The owning aggregate above is the primary one; the others are joins.
