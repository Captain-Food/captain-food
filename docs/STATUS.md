# 🚦 Captain.Food — Development & Deployment Status

> Hand-maintained snapshot (NOT generated, outside `specs/` so it never affects the DSL).
> Last updated: 2026-07-18. Legend: ✅ done & verified · 🚧 in progress · ⏳ blocked/waiting · 📋 planned.

## 🌐 Deployment

| Piece | Status | Notes |
|---|---|---|
| Render web service (Docker, Frankfurt) | ✅ | Blueprint IaC (`render.yaml`), cargo-chef cached build, verified live |
| Supabase Postgres (Frankfurt, eu-central-1) | ✅ | Session pooler; Data API off (intentional) |
| CI `codegen-consistency` (build+test+validate+drift) | ✅ | Gates deploys (`autoDeployTrigger: checksPass`) |
| CI `db-migrate` (sqlx-cli, gated on green build) | ✅ | Applies `migrations/*.sql` out-of-band (ADR-0043) |
| `/health` (schema-version readiness), `/ping`, `/projector` | ✅ | `>=` version gate; in-process projector |
| GraphQL `/{role}/graphql` + `/{role}/voyager` | ✅ | Role-as-path; per-role filtered schema |

## 📖 Read side (queries)

| Query | Status | Notes |
|---|---|---|
| `restaurants` / `restaurant` | ✅ | Real data once SIRENE runs |
| `prospectionPipeline` | ✅ | Admin; fed by SIRENE registrations |
| `pricingPolicy` / `uberEstimationPolicy` / `uberSplitPolicy` | ✅ | **Real seeded data** |
| `catalog` / `categories` / `carts` / `cart` / `orders` / `order` | ✅ wired | Empty until the write side emits their events |
| `me` / `favoriteRestaurants` | ⏳ | Needs auth/identity (JWT) |
| Projection worker → registry (per-aggregate checkpoints) | ✅ | In-process |

## ✍️ Write side (mutations)

| Piece | Status | Notes |
|---|---|---|
| `MutationRoot` (all api.yaml mutations generated) | ✅ | |
| Restaurant aggregate (13 commands) | ✅ | Spec invariants (event-stream rehydration) + 25 behaviour tests |
| Other aggregates (Prospect, Catalog, Cart, Order, Customer, RestaurantAccount, Delivery) | 🚧 | Round 2 — in progress |
| Structured typed errors (vs interim `"Code: detail"`) | 📋 | ADR-0046 follow-up |

## 🔐 Authorization

| Piece | Status | Notes |
|---|---|---|
| Per-role ACL — execution guard + per-role introspection/Voyager | ✅ | Spec-derived from api.yaml `roles` (ADR-0006) |
| Per-field ACL on FK-derived nav edges | 📋 | Needs a DSL decision (escalate to plan mode) |
| Authentication / identity (Supabase JWT) | ⏳ | Not started |

## 🔎 SIRENE prospection (ADR-0019/0020/0027/0045)

| Piece | Status | Notes |
|---|---|---|
| SIRENE ACL (INSEE → RegisterRestaurant mapping) | ✅ | Unit + DB verified |
| Interim direct-write `sirene_sync` binary | ✅ | Superseded by ADR-0045; to retire |
| `external_sirene_restaurants` staging table | ✅ | Migration applied by CI |
| Thin CI ingestion (fetch → UPSERT raw rows, nationwide) | 🚧 | ADR-0045 — in progress |
| On-app `sync_sirene_worker` (ACL on deployed version) + deletion | 🚧 | ADR-0045 — in progress |
| `INSEE_API_TOKEN` repo secret (to go live) | ⏳ | User action |

## 👤 Pending user actions

- ⏳ Rotate the earlier leaked `SUPABASE_SECRET_KEY` + DB password.
- ⏳ Add `INSEE_API_TOKEN` repo secret to run SIRENE live.

## 🧭 Architecture decisions
See [`docs/adr/`](adr/) — latest: 0042 (hosting), 0043 (migrations), 0044 (license), 0045 (SIRENE redesign), 0046 (write side).
