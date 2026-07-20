# 🚦 Captain.Food — Development & Deployment Status

> Hand-maintained snapshot (NOT generated, outside `specs/` so it never affects the DSL).
> Last updated: 2026-07-20 (early). Legend: ✅ done & verified · 🚧 in progress · ⏳ blocked/waiting · 📋 planned.

> ✅ **2026-07-20 (early) — post-merge wave, all landed directly on `main` (user-directed), each
> workstream gated in an isolated worktree then re-gated integrated (final: 29x tests green,
> validate 0 errors, drift clean):** ① **Production JWT bug fixed** — `jsonwebtoken` v10 had no
> crypto backend selected → every authenticated GraphQL request panicked (502) in prod; fixed with
> the `rust_crypto` feature. ② **Automated prod E2E smoke test (Stripe TEST mode)** —
> `tools/smoke/prod-smoke.sh` (`make smoke-prod`, `.github/workflows/prod-smoke.yml`
> workflow_dispatch + daily cron; needs repo secrets `STRIPE_SECRET_KEY`/`RENDER_API_KEY`, not yet
> configured): layered ping/health → public GraphQL → idempotent `smoke-test` tenant fixture →
> full checkout with `pm_card_visa` confirmed server-side → poll until captured. Stripe test
> webhook endpoint created → `https://api.captain.food/adapters/stripe/webhooks`
> (`payment_intent.succeeded`/`payment_intent.payment_failed`/`charge.refunded`), signature
> verified live; `STRIPE_WEBHOOK_SECRET` set in Render. ③ **Server-side pricing, fail-closed**
> (ADR-20260720-002217): `place_order` reprices every folded cart line from the live catalog
> (`application::pricing::price_cart`) → PaymentIntent amount + frozen snapshot; optional
> `PlaceOrder.expectedTotal` equality check; `PriceMismatch`/`PriceUnresolvable`; rule
> `ServerPriceAuthority`. ④ **`pendingRefunds` read model** (ADR-20260720-003142): new
> `RefundOpened` event on the Payment stream, `View_PendingRefunds` fold view + migration,
> `pendingRefunds` query (RESTAURANT+ADMIN) + story steps, rule `PendingRefundVisibleUntilDecided`.
> ⑤ **Bounded partner re-offer policy** (ADR-20260720-004556): decline → re-offer, cap 3
> (`offer_attempts` in the run row), exhaustion → `DeliveryDispatchFailed` + run FAILED (status
> `FAILED` replaces `REOFFER_REQUIRED`); offer timeouts deferred (no time-based sweep host yet).
> ⑥ **Codegen roadmap item 1, first slice** (ADR-20260720-004419): `lifecycle:` DSL in actors.yaml
> (event-keyed), 8 `lc-*` validator rules + coverage warning, generated
> `domain/src/generated/lifecycles.rs` transition tables + mermaid state diagrams in the docs;
> Order wired end-to-end. Remaining open: fee/split breakdown (ADR-0016/0017), offer timeouts,
> Rider/DeliveryJob/Restaurant lifecycle adoption, worker `DeliveryJob-%` drain, roadmap items 2–7,
> GitHub repo secrets for the smoke workflow.

> 🔀 **Parallel session engaged: command sourcing + inbound-event sourcing** (infrastructure
> journals, branched from main after this branch merges). Two constraints agreed here carry over:
> journals NEVER write `domain_events` (aggregates own the log — ADR-20260719-193500; hook points =
> the ACL ingest seams + the server's mutation-dispatch boundary), and the event log stays the single
> source of truth (a command journal records requests incl. rejections, never replays as state).
> Journal TABLES still get declared in `specs/database/tables/*.yaml` so their DDL generates.
>
> 🧭 **Agreed direction (2026-07-19, late):** generalize the spec→codegen approach — ①
> **service catalog with configurable binding** (ADR-20260719-214500, Proposed): `specs/services.yaml`
> declares the abstract APIs, own spec apart from api.yaml (`/services/payment` `request`/`refund` → Stripe adapter, delivery,
> identity, catalog_sync, …); binding + exposure DECIDED IN THE SPEC (local for all of V0; config carries only addresses); PM
> `ports` will `$ref` the catalog. ② **Codegen roadmap** ([docs/codegen-roadmap.md](codegen-roadmap.md)),
> ranked: aggregate lifecycle state machines → generated behaviour-test harness from tests.yaml →
> PM orchestrator scaffolding → the service catalog → PM state-store generation.
> ① LANDED (2026-07-19): `specs/services.yaml` + validator §2d (`svc-*` rules) are in, PM `ports` now `$ref` the catalog (ADR Accepted); trait/client/route emitters still to come.
>
> ✅ **RUNTIME REIMPLEMENTED (2026-07-19 night) — the state-table PM runtime is live on this branch
> (ADR-20260719-193500), 266 workspace tests green, `make validate` 0 errors, no drift.** Landed:
> the `Payment` (stream `Payment-{intentId}`) + `Rider` aggregates and DeliveryJob partner/issue
> folds; the 4 PM state tables (migration + `pm_state` ports + Pg stores); the full missing command
> surface (Rider ×3, DeliveryJob ops ×7, `bindCartToCustomer`); `placeOrder` delivers
> `PaymentIntentCreated` to the Payment stream and opens the run row (concurrent checkout →
> Conflict); all four orchestrators execute their DSL legs (guards throw typed errors —
> `PaymentEventOrphaned`, `DeliveryJobNotFound`; refund decisions by RESTAURANT/ADMIN via
> `approve_refund`/`deny_refund` + fail-closed `request_refund`; cart binding really binds; close
> order via `send MarkOrderDelivered`); the runner surfaces thrown guards on `/saga`; the Stripe ACL
> is a stateless translator (no more `StripeEvent-%` streams, `CheckoutSnapshotSource` seam
> retired). Since then, ALL THREE remaining runtime gaps closed tonight: ① the **refund decision
> API surface** — `approveRefund`/`denyRefund` mutations (api.yaml, roles RESTAURANT+ADMIN, V0;
> story steps in ManageOrders + admin ArbitrateRefunds), emitted resolvers calling the RefundProcess
> orchestrator legs over the new `WriteDeps.refund_state` (`PgRefundProcessState`) + the
> PaymentGateway. ② The **real outbound Stripe adapter** (`stripe::outbound::StripePaymentGateway`):
> form-encoded create-intent (+ `metadata[orderId]`/`[restaurantId]`/`[cartId]`, which the webhook
> ACL requires) and refunds; the port grew a typed `PaymentIntentRequest`; constructed when
> `STRIPE_SECRET_KEY` is set, else the fail-closed stand-in (logged at startup). ③ The
> **`OrderTracking.payment_status` cross-stream feed**: the projection worker's Order group slices
> BOTH `Order-%` and `Payment-%` under its single 'Order' checkpoint (`stream_name LIKE ANY`), and
> Payment-stream facts key the Order row from the payload's `orderId` (a capture without one is
> log-skipped). Still open (see docs/sagas.md): partner re-offer policy, server-side pricing,
> `pendingRefunds` read model/query.
>
> 📣 **Earlier on this branch (2026-07-19 evening):** ① Guard semantics hardened — **in case of error a
> guard always `throws` a typed exception, on EVENT legs too** (run aborts + error surfaced — e.g.
> `PaymentEventOrphaned` for an orphan Stripe capture/failure, `DeliveryJobNotFound` for partner
> reports on an unknown dispatch run); `skip` is strictly for benign alternatives, and the validator
> enforces exactly-one-outcome per guard. ② The **CI gate (workflow `ci`, ex `codegen-consistency`) now runs on every
> branch push** (was main-only), so no branch escapes validate + test + drift. ③ The **per-PM
> sequence diagrams are now embedded in the product documentation** — `documentation.generated.md`
> (mermaid fences, renders on GitHub) **and** `documentation.generated.html` (in-page mermaid
> renderer, offline-degrades to readable source) — generated from the typed steps, zero drift.
>
> 🚧 **Feature branch — Process-manager re-architecture: DSL layer DONE, runtime pending.** Process
> managers are now **state-table orchestrators specified by a TYPED step DSL** (ADR-20260719-172821):
> `specs/processmanager.yaml` legs are ordered `read`/`guard`/`call`/`deliver`/`send`/`state` steps —
> every field a `$ref` or enum const, state in declared tables (`process_managers.yaml`), command-leg
> guards `throws` / event legs `skip`, emits **derived** from steps, sequence diagrams **generated**
> from steps (`c4.generated.md`). Validator §2b proves the wiring; the ADR-0032 gate applies to PMs
> unexempted. `make validate` **58 → 0 errors** (behaviour tests added for Rider, DeliveryJob ops,
> Payment records, admin-approved RefundProcess incl. `RefundNotPending`). `cargo test --workspace`
> green. The PM **runtime is NOT reimplemented yet** (still the event-sourced runner): see
> **[docs/process-manager-rearchitecture.md](process-manager-rearchitecture.md)** for the phase plan.
> Also on the branch (green): the write-side **`Repository`** refactor (ADR-20260719-031136) + the
> **checkout snapshot** (ADR-20260719-014434) — the runtime rework will rebuild the saga side of these.

## 🌐 Deployment

| Piece | Status | Notes |
|---|---|---|
| Render web service (Docker, Frankfurt) | ✅ | Blueprint IaC (`render.yaml`), cargo-chef cached build, verified live |
| Supabase Postgres (Frankfurt, eu-central-1) | ✅ | Session pooler; Data API off (intentional) |
| CI workflow `ci` (build+test+validate+drift; ex `codegen-consistency`) | ✅ | Gates deploys (`autoDeployTrigger: checksPass`) |
| CI `db-migrate` (sqlx-cli, gated on green build) | ✅ | Applies `migrations/*.sql` out-of-band (ADR-0043) |
| `/health` (schema-version readiness), `/ping`, `/projector` | ✅ | `>=` version gate; in-process projector |
| GraphQL `/{role}/graphql` + `/{role}/voyager` | ✅ | Role-as-path; per-role filtered schema |
| Custom domains `*.captain.food` (Dynadot wildcard → Render) + Host router | ✅ | Wildcard TLS issued; apex+`www` 301→`join` (GitHub Pages); `hosts.rs` dispatches audiences (`live`/`restos`/`riders`/`system`) + `{slug}` tenants; onrender URL disabled. Recorded in **ADR-0036 amendment (2026-07-18) + ADR-0042** |

## 📖 Read side (queries)

| Query | Status | Notes |
|---|---|---|
| `restaurants` / `restaurant` | ✅ | Real data once SIRENE runs |
| `prospectionPipeline` | ✅ | Admin; fed by SIRENE registrations |
| `pricingPolicy` / `uberEstimationPolicy` / `uberSplitPolicy` | ✅ | **Real seeded data** |
| `catalog` / `categories` | ✅ | **Real nested data** — catalog `tree` projector (categories→products→offers/option-lists + derived `stockStatus`) |
| `carts` / `cart` / `orders` / `order` | ✅ wired | Populated as carts/orders are placed |
| `me` / `favoriteRestaurants` | ✅ | `me` resolves the verified ADR-0047 `Principal` → Customer read model; `favoriteRestaurants` joins the customer's favourites |
| Projection worker → registry (per-aggregate checkpoints) | ✅ | In-process; **no batch cap** (drains all pending per tick, loops 1.5s); hardened to **log-skip a poison event** so one bad record can't wedge projection. ⚠️ Free-tier **spin-down** pauses it when the app is idle >15 min → kept warm via **uptimerobot `/ping` every 5 min** |

## ✍️ Write side (mutations)

| Piece | Status | Notes |
|---|---|---|
| `MutationRoot` (all api.yaml mutations generated) | ✅ | |
| Restaurant aggregate (13 commands) | ✅ | Spec invariants (event-stream rehydration) + 25 behaviour tests |
| Cart (3) · Order (11) · DeliveryJob (4) | ✅ | Round 2a — real invariants + tests; **Cart line-checks now enforced** (OfferUnavailable/InsufficientStock/InvalidOptionSelection) via the catalog offer read port |
| Catalog (12) · Prospect (3) · RestaurantAccount (3) | ✅ | Round 2b — real invariants + behaviour tests |
| Customer (14) | ✅ | Wired end-to-end: `customer` read model + Pg repo, fail-closed `AuthProviderGateway` stand-in (real Supabase ACL deferred), injected at the composition root |
| `placeOrder` + process managers (4 sagas) | ✅ wired | `placeOrder` live (fail-closed `PaymentGateway` stand-in); in-process PM runtime (`/saga`) — PlaceOrder/Refund/CartBinding/DeliveryDispatch react to payment/delivery facts → `OrderPlaced`/`OrderDelivered`/… **Real Stripe create-intent = 🅑**; ✅ **checkout-snapshot DSL closed** (ADR-20260719-014434): `PaymentIntentCreated` now carries `checkout` (`CheckoutSnapshot`), frozen by `place_order`, so `OrderPlaced` rebuilds from the log — priced `items`/`breakdown` + retiring the fail-closed `CheckoutSnapshotSource` ride on server-side pricing |
| Structured typed errors | ✅ | `DomainError::Rejected{code,context}` → GraphQL `extensions.code` + interpolated en/fr message (ADR-20260719-120000) |
| GraphQL **subscriptions** | ✅ | `SubscriptionRoot` + in-process event bus + WS transport + per-role ACL (`orderStatusChanged`/`operationStatusChanged`); works while the app is warm |

## 🔐 Authorization

| Piece | Status | Notes |
|---|---|---|
| Per-role ACL — execution guard + per-role introspection/Voyager | ✅ | Spec-derived from api.yaml `roles` (ADR-0006); role now **verified** by JWT (ADR-0047), so Voyager filtering is trustworthy |
| Per-field ACL on FK-derived nav edges | 📋 | api.yaml has **op-level** `roles` only; needs a DSL extension → **plan mode** |
| EXTERNAL machine callers | ✅ | Pre-shared `X-External-Api-Key` (`EXTERNAL_API_TOKENS`, constant-time) or Supabase JWT w/ captain_role EXTERNAL (ADR-0047) |
| Authentication / identity (Supabase JWT) | ✅ | **First cut shipped (ADR-0047)**: verify Supabase JWT via JWKS at `/{role}/graphql` (public keys, no shared secret; ~1h cache, serve-stale-on-refresh-failure — no per-request Supabase call); `app_metadata.captain_role` gates the path (`/public` open, else 401/403), fail-closed on cold cache, asymmetric-only. Verified role + `Principal` injected. **EXTERNAL service tokens** via `X-External-Api-Key` (constant-time, `EXTERNAL_API_TOKENS`) shipped. Per-field `@auth` on FK-nav edges = DSL/plan-mode follow-up |

## 🔎 SIRENE prospection (ADR-0019/0020/0027/0045)

| Piece | Status | Notes |
|---|---|---|
| SIRENE ACL (INSEE → RegisterRestaurant mapping) | ✅ | Unit + DB verified |
| Interim direct-write `sirene_sync` binary | ✅ | **Retired** (ADR-0045) — replaced by the split below |
| `external_sirene_restaurants` staging table | ✅ | Migration applied by CI |
| Thin CI ingestion crate `sirene_ingest` (fetch → UPSERT raw rows, France-wide by department, active-only) | ✅ | No domain deps; scheduled workflow builds only this crate |
| On-app `sync_sirene_worker` (ACL on deployed version) + deletion reconciliation | ✅ | Per-row checkpoint; detect-by-absence (21d debounce) + explicit `F`/`C`; NON_PARTNER auto-close, partners flagged; `POST /internal/sirene/drain` (token-gated, fail-closed) |
| `INSEE_API_TOKEN` repo secret | ✅ | Added; SIRENE runs live on deploy (scheduled ingestion → staging → worker) |
| `INTERNAL_TRIGGER_TOKEN` (Render env + repo secret) to enable the CI→worker ping | ⏳ | Optional; without it CI ingests and the worker drains on its own poll loop (`RUN_SIRENE_WORKER`, default on) |

## 🔌 External integrations — partner adapters & M2M (ADR-20260718-145856 / -213352)

**Partner webhook adapters are self-contained crates** under `crates/adapters/*` — each an ACL +
axum shell + standalone binary, mountable into the monolith **or** deployable as its own web service.
Two directions: partner-**push** webhooks (below) vs external-**drive** `/external/graphql` (M2M).

| Piece | Status | Notes |
|---|---|---|
| **Stripe** — `crates/adapters/stripe` (`POST /adapters/stripe/webhooks`, `stripe-webhook` bin) | ✅ | `Stripe-Signature` HMAC over raw body (constant-time, 300s replay, fail-closed); ACL → `PaymentCaptured`/`PaymentFailed`/`PaymentRefunded`; idempotent by Stripe event id. 12 tests |
| Checkout must set `metadata.restaurantId` (+`orderId`) on the PaymentIntent/charge | 📋 | Else `charge.refunded` is unmappable (logged + 200-ACKed). Lands with `placeOrder` |
| **HubRise** — `crates/adapters/hubrise` (`POST /adapters/hubrise/webhooks`, `hubrise-webhook` bin) | ✅ | **Ingress** ✅ (HMAC-SHA256 hex, fail-closed, envelope parse). **Outbound OAuth2 client** ✅ (`api.rs`: `X-Access-Token`, non-expiring token from `HUBRISE_ACCESS_TOKEN`, `exchange_code` connect helper, catalog/inventory pull). **Domain wiring** ✅ (`enrich.rs`): verified catalog/inventory callback → API pull → enrichment ACL → `ImportCatalog` / per-SKU `update_offer_stock` handlers. **Deterministic UUIDv5-of-HubRise-id** ids reconciled with the **Catalog aggregate** (offer seeded from the SKU `ref` = inventory's `sku_ref`, so a stock update hits the imported `OfferId`); `"9.80 EUR"`→`Money`, tax-rate strings→`TaxRate`, `data` envelope translated at the boundary; catalog = rejectable command (`CatalogNotFound`→skip), inventory = reported fact (`OfferNotFound`→skip, never rejected). 14 tests. Enricher wired at the server composition root + the standalone bin (both gated on `HUBRISE_ACCESS_TOKEN`). **Open**: the connect flow must create the `Catalog`/`Restaurant` with these derived ids + a token table (→ plan mode) |
| **`/external/graphql`** — M2M standard | ✅ | External entities query/mutate via the `EXTERNAL` role path; API-key auth (`X-External-Api-Key`, ADR-0047); allowlist is per-op `roles: [EXTERNAL]`. **Subscribe** = future (needs `SubscriptionRoot` + WS + `api.yaml`); per-partner keys = future |

## 👤 Ops / user actions

- ✅ Keep the web service **warm via uptimerobot `/ping` every 5 min** (prevents free-tier spin-down so the in-process projector + SIRENE worker keep running).
- 🗑️ `INTERNAL_TRIGGER_TOKEN` / `POST /internal/sirene/drain` — agreed to **remove** (superseded by the `/ping` warmth approach); code removal deferred to avoid colliding with concurrent `routes.rs` edits — harmless meanwhile (fail-closed 503 when the secret is unset).

## 📋 Remaining work — todo & session split

Two sessions run in parallel — 🅐 = this (desktop) session, 🅑 = the iPhone/other session. Pull-rebase before every push.

| # | Item | Owner | Status |
|---|---|---|---|
| 1 | **Checkout saga** — `placeOrder` + `PlaceOrderProcess` + PM runtime | 🅐 | ✅ wired (fail-closed gateway) |
| 1a | **Checkout snapshot** on `PaymentIntentCreated` (ADR-20260719-014434) — DSL + `place_order` freeze + tests done | 🅐 | ✅ DSL · runtime population + port retirement ride pricing |
| 1b | Stripe **outbound** `PaymentGateway` (create PaymentIntent) in the Stripe adapter crate | 🅑 (owns Stripe) | 📋 |
| 2 | **HubRise** domain ACL — webhook → `ImportCatalog`/`OfferStockUpdated` (OAuth2 pull + deterministic ref-mapping) | 🅐 | ✅ landed (`enrich.rs`, 14 tests) |
| 2a | ⚠️ **Connect flow** — provision `RegisterRestaurantAccount` + `Restaurant`(s) + `CreateCatalog` with the enricher's derived UUIDv5 ids, and persist the HubRise **account-scoped** token in a connection/token table keyed by `RestaurantAccount` (HubRise Account⇔RestaurantAccount, Location⇔Restaurant; `HUBRISE_ACCESS_TOKEN` today = one account). See `docs/integrations/hubrise-process.md` §0 | plan mode | 📋 |
| 3 | **Process managers** — Refund/CartBinding/DeliveryDispatch + PM runtime (event-driven, `/saga`) | 🅐 | ✅ (Refund/CartBinding emit [] per spec; partner re-offer + outbound refund = TODO(saga)) |
| 4 | **Cart line invariants** + catalog `tree` projector + offer read port | 🅐 | ✅ |
| 5 | **Frontend** — Leptos/WASM SDUI renderer (customer/restaurant/rider apps) | unassigned | 📋 |
| 6 | GraphQL **subscriptions** (`SubscriptionRoot` + bus + WS + ACL) | 🅐 | ✅ |
| 7 | **Structured typed errors** (ADR-20260719-120000) | 🅐 | ✅ |
| 8 | **Per-field nav-edge ACL** — optional `roles:` on nav fields (default public), same guard/visible as ops; design agreed | 🅐 | 📋 plan mode (after ACL emitter free) |
| 8b | Delivery/account read queries + catalog `tree` + `me`/favorites | 🅐 | ✅ (read surface complete except `phoneCountries`=client-const, `operation`) |
| 9 | Remove `INTERNAL_TRIGGER_TOKEN`/drain endpoint (use `/ping` warmth) | 🅐 | 🗑️ deferred |
| 10 | Projection worker robustness (poison-skip) + spin-down mitigation (uptimerobot `/ping`) | 🅐 | ✅ |

## 🧭 Architecture decisions
See [`docs/adr/`](adr/) — latest: 0047 (API auth — Supabase JWT/JWKS), 20260719-120000 (structured domain rejections), **20260719-014434 (checkout snapshot on `PaymentIntentCreated`)**, **20260719-031136 (write-side `Repository` / event-sourced actors — handlers + saga runner route through it, never the raw `EventStore`)**, 20260718-145856 amendment (adapter webhook routes → `/adapters/{partner}/webhooks`). **ADR ids are now date-time** to avoid concurrent-session collisions (ADR-20260718-135417).

> Convention: keep this file current with every substantive change, and record cross-cutting decisions as an ADR in the same change.
