## 1. Event store table

PostgreSQL table definition (conceptual):

```sql
CREATE TABLE domain_events (
  position        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY, // $all order: total order across every stream (projection checkpoint)
  id              UUID NOT NULL UNIQUE,        // event id — idempotency key, deduped on append
  stream_name     TEXT NOT NULL,               // '<Category>-<id>', e.g. 'Menu-12345'; category = prefix before the first '-'
  version         INT  NOT NULL,               // 0-based event number within the stream (expected-version concurrency)
  user_id         UUID NOT NULL,
  user_type       INT NOT NULL,
  correlation_id  UUID NOT NULL,
  cause_id        UUID NULL,
  event_type      TEXT NOT NULL,               // event type ($et-<type> projection)
  payload         JSONB NOT NULL,              // event data
  metadata        JSONB,
  occurred_at     TIMESTAMPTZ NOT NULL,        // could be interesting if we use integer
  expired_at      TIMESTAMPTZ NULL,            // per-event TTL (cf. per-stream $maxAge / $maxCount metadata)
  UNIQUE (stream_name, version)                // optimistic concurrency: one event per (stream, expected version)
);

CREATE INDEX ON domain_events (stream_name, version);  // read one stream in order; prefix-scannable per category
CREATE INDEX ON domain_events (event_type);            // $et-<type>
CREATE INDEX ON domain_events (occurred_at);
```

This mirrors **EventStoreDB / SqlStreamStore** in plain SQL. A **stream** is the ordered event
sequence of one aggregate instance; it maps 1:1 to a domain aggregate (`actors.yaml`). The mapping:

| EventStore concept | Column / mechanism here |
|---|---|
| Stream name (`<Category>-<id>`, e.g. `Menu-12345`) | `stream_name` — category = prefix, so **no `stream_type` column** |
| Event number / stream revision (0-based) | `version` — `UNIQUE (stream_name, version)` gives expected-version concurrency |
| `$all` global position | `position` (identity) — total order; projections track a checkpoint on it |
| Event id (idempotent append) | `id` — `UNIQUE` |
| Event type | `event_type` |
| `$ce-<category>` projection | `ce_events(category)` (below) |
| `$et-<type>` projection | `et_events(event_type)` (below) |
| Stream `$maxAge` / `$maxCount` | `expired_at` (simplified to a per-event TTL) |

- The category prefix is one of `Restaurant | Menu | Customer | Cart | Order | DeliveryJob`
  (matches the aggregates in [actors.yaml](actors.yaml)); the `<id>` suffix is the instance id.
- `metadata`: optional. To stay faithful to EventStore, `correlation_id` / `cause_id` / user could be
  folded in here (as `$correlationId` / `$causationId`) rather than kept as columns — left as columns
  for now for query convenience.

### Helper — events for a category (`$ce-<category>`)

`ce_events(category)` returns every event whose stream belongs to one **category**, in chronological
order — the SQL equivalent of EventStoreDB's `$ce-<category>` projection. This is for
inspection/replay over the log only — read paths still go through the `View_*` projections, never
`domain_events` directly.

```sql
-- ce_events('Menu')  ==  SELECT * FROM domain_events WHERE stream_name LIKE 'Menu-%'
CREATE FUNCTION ce_events(category TEXT)
RETURNS SETOF domain_events
LANGUAGE sql STABLE AS $$
  SELECT *
  FROM domain_events
  WHERE split_part(stream_name, '-', 1) = category
  ORDER BY stream_name, version;
$$;
```

- `category` is a stream-name prefix: `Restaurant | Menu | Customer | Cart | Order | DeliveryJob`.
- The category is derived from `stream_name` (prefix before the first `-`), so no `stream_type`
  column is stored.
- Ordered by `(stream_name, version)` so each stream stays contiguous and replay-ordered.

### Helper — events for an event type (`$et-<type>`)

`et_events(event_type)` returns every event of one **event type** across all streams, in global
order — the SQL equivalent of EventStoreDB's `$et-<type>` projection. Same caveat: inspection/replay
only, never a read path.

```sql
-- et_events('RestaurantRegistered')  ==  SELECT * FROM domain_events WHERE event_type = 'RestaurantRegistered'
CREATE FUNCTION et_events(event_type TEXT)
RETURNS SETOF domain_events
LANGUAGE sql STABLE AS $$
  SELECT *
  FROM domain_events
  WHERE domain_events.event_type = et_events.event_type
  ORDER BY position;
$$;
```

- `event_type` is an event name from [events.yaml](events.yaml), e.g. `'RestaurantRegistered'`.
- Backed by the `(event_type)` index.
- Ordered by `position` (the `$all` global order), since the result spans many streams.

## 2. Read models — projection views (`View_*`)

Queries **never** read `domain_events`; they read dedicated read tables fed by projections that
consume events. These read tables are **"fake" tables** (denormalized, query-shaped, rebuildable
from the log) — to avoid any confusion with a real/normalized table, every one is prefixed
**`View_`** (`View_{TableName}`).

Each view below declares only what is intrinsic to the read model: its **source aggregate(s) +
events** ([events.yaml](events.yaml) / [actors.yaml](actors.yaml)), its **business filters/rules**,
and its **columns**. The consumer mapping — which GraphQL query reads it and which UI screen it
backs — is **not** repeated here; it lives in [traceability.md](traceability.md) §2 (persona → query
→ view → UI). Money is stored as integer minor units (`*_cents` + `currency`), matching `Money`.
`JSONB` is used where a whole sub-tree is fetched at once.

### `View_RestaurantsPublic`
- **Aggregate**: `Restaurant`. **Fed by**: `RestaurantRegistered`, `RestaurantUpdated`,
  `RestaurantActivated`, `RestaurantDeactivated`, `RestaurantAcceptanceModeChanged`.
- **Filters**: the public discover list exposes `status = ACTIVE` only; other statuses are kept in
  the view for the single-restaurant header.
- **Columns**: `restaurant_id` (PK), `slug` (unique), `display_name`, `description`, `tags` JSONB,
  `address` JSONB, `opening_hours` JSONB, `status`, `order_acceptance`, `default_currency`,
  `preparation_time_minutes`, `updated_at`.

### `View_RestaurantMenu`
- **Aggregate**: `Menu`. **Fed by**: `MenuCreated`, `Category*`, `Product*`, `OptionList*`,
  `VariantStockUpdated`, `CatalogImported`.
- **Rules**: `stock_status` is derived (quantity vs `lowStockThreshold`); orderable = `AVAILABLE`
  **and** stock > 0. Could be normalized (one row per variant) if per-item querying is needed later.
- **Columns**: `menu_id` (PK), `restaurant_id` (index), `slug`, `name`, `catalog` JSONB — the
  assembled tree: categories → products → variants `{ price_cents, currency, availability,
  stock_status }` + option lists. `updated_at`.

### `View_Cart`
- **Aggregate**: `Cart` (joined with the catalog for pricing). **Fed by**: `CartStarted`,
  `CartLineAdded`, `CartLineQuantityChanged`, `CartLineRemoved`, `CartCheckedOut`.
- **Rules**: prices are computed by the projection from the current catalog, never trusted from the
  client. `customer_id` is NULL while the cart is owned by a guest.
- **Columns**: `cart_id` (PK), `restaurant_id`, `customer_id` (NULL while guest), `status`,
  `lines` JSONB `[{ cart_line_id, variant_id, product_id, name, variant_name, quantity,
  unit_price_cents, selected_options, line_total_cents }]`, `total_amount_cents`, `currency`, `updated_at`.

### `View_OrderTracking`
- **Aggregate**: `Order` (+ payment facts). **Fed by**: `OrderPlaced`, `OrderAcceptedByRestaurant`,
  `OrderPreparationStarted`, `OrderMarkedReady`, `OrderDelivered`, `OrderRejectedByRestaurant`,
  `OrderCancelledByCustomer`, `OrderCancelledByRestaurant`, `PaymentCaptured`, `PaymentRefunded`.
- **Rules**: `payment_status` is folded from the Stripe payment facts.
- **Columns**: `order_id` (PK), `ref`, `restaurant_id`, `customer_id` (NULL), `status`,
  `service_type`, `items` JSONB, `total_amount_cents`, `currency`, `delivery_address` JSONB,
  `estimated_ready_at`, `placed_at`, `status_changed_at`, `payment_status`.

### `View_OrdersByRestaurant`
- **Aggregate**: `Order`. **Fed by**: the same order lifecycle events as `View_OrderTracking`.
- **Filters**: queried/indexed by `(restaurant_id, status, placed_at)`.
- **Columns**: `order_id` (PK), `restaurant_id` (index), `status`, `service_type`,
  `customer_display_name`, `total_amount_cents`, `currency`, `placed_at`, `accepted_at`,
  `estimated_ready_at`. Index `(restaurant_id, status, placed_at)`.

### `View_OrdersByCustomer`  *(V1)*
- **Aggregate**: `Order`. **Fed by**: the order lifecycle events.
- **Filters**: queried/indexed by `customer_id`.
- **Columns**: `customer_id` (index), `order_id`, `restaurant_id`, `restaurant_display_name`,
  `status`, `total_amount_cents`, `currency`, `placed_at`.

> A projection may consume events from **more than one aggregate** (e.g. `View_OrderTracking` folds
> `Order` lifecycle + Stripe payment facts; `View_Cart`/`View_RestaurantMenu` price against the
> catalog). The owning aggregate above is the primary one; the others are joins.
