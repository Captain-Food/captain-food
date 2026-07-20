# 📋 Captain.Food — Prioritised Backlog

> Hand-maintained (NOT generated, outside `specs/` so it never affects the DSL). This file is the
> **repository copy of the backlog order** — the same ranking stamped in each GitHub issue header.
> Ordering rule: **value-first** ([ADR-20260720-213024](adr/20260720-213024-value-first-issue-prioritisation.md)) —
> tier 1 = foundations & cross-functional/non-functional, tier 2 = features in value-stream order
> (customer ordering → restaurant onboarding → delivery).
>
> **How to use it (all sessions, human or agent):**
>
> - **Pick work from the top** of the open queue. Skipping an item requires a stated reason
>   (blocked, plan-mode approval pending, product-owner directive) — not preference.
> - **Re-ordering is a product-owner decision**, recorded as an ADR (amend/supersede
>   ADR-20260720-213024) and applied in the same change to BOTH this file and the issue header
>   stamps. Never let the two drift.
> - When an issue **closes**, move its row to the Done section (keep its rank number); do not
>   renumber the survivors — the order, not the denominator, is the contract
>   (ADR-20260720-213024 §3).
> - New issues get a value rank at triage (two-tier test above) and a row here.
> - **GitHub Project sync** ("Prioritized backlog" board): the org **Priority** field mirrors the
>   queue position — ranks 1–6 → `Urgent`, 7–11 (codegen wave) → `High`, 12–14 → `Medium`,
>   15/post-V0 → `Low` — and the **Effort** field mirrors the size label (XXS–S → `Low`,
>   M → `Medium`, L and up → `High`). Value drives Priority; effort is displayed but never drives
>   order. Update both fields whenever a rank or size changes; exact order within a Priority
>   bucket is this file + the issue header stamps.
>
> Last updated: 2026-07-20 · Sizes/estimates: ADR-20260720-143000 (unchanged by the re-ordering).

## Open queue (work top-down)

### Tier 1 — foundations & cross-functional / non-functional

| Rank | Issue | Size | One-liner |
|---|---|---|---|
| 1 | [#14](https://github.com/Captain-Food/captain-food/issues/14) orderStatusChanged convention | S | Last subscription on the old `correlationId` convention — align (orderId + ownership scope) before any client consumes it. |
| 2 | [#22](https://github.com/Captain-Food/captain-food/issues/22) per-field ACL on nav edges | M | FK-derived navigation edges are effectively public; add `roles:` on nav fields before the schema/clients grow. |
| 3 | [#15](https://github.com/Captain-Food/captain-food/issues/15) journal WORKER command sends | S | HubRise enricher + SIRENE worker bypass `command_journal`; restore the all-channels invariant. |
| 4 | [#16](https://github.com/Captain-Food/captain-food/issues/16) command-acceptance observability | M | `surface: graphql` binding kind + the generic acceptance contract for the most critical workflow. |
| 5 | [#19](https://github.com/Captain-Food/captain-food/issues/19) checkout latency watch | XXS | Standing watch on acceptance→clientSecret latency; pre-made `sync: true` escape hatch if it degrades. |
| 6 | [#18](https://github.com/Captain-Food/captain-food/issues/18) journal/mirror retention | S | Retention windows + sweep for `command_journal`/`inbound_events`/`external_*` (disk + GDPR); never `domain_events`. |
| 7 | [#27](https://github.com/Captain-Food/captain-food/issues/27) PM state-store emitters | M | Generate PM row structs, `by_*` lookups, upserts, in-mem doubles from the table DSL. |
| 8 | [#26](https://github.com/Captain-Food/captain-food/issues/26) service-catalog emitters | L | Make `services.yaml` executable: generated port traits, http clients, local bindings. |
| 9 | [#24](https://github.com/Captain-Food/captain-food/issues/24) generated behaviour-test harness | L | `tests.yaml` becomes the test suite; parity oracle for #25/#23. |
| 10 | [#25](https://github.com/Captain-Food/captain-food/issues/25) generated PM orchestrators | L | Spec-only sagas from the typed-step DSL; replace the four hand orchestrators at parity. |
| 11 | [#23](https://github.com/Captain-Food/captain-food/issues/23) lifecycle completion | L | Dynamic targets, Restaurant adoption, fold rewiring, generated handlers (step 4 after #24). |

### Tier 2 — features, in value-stream order

| Rank | Issue | Size | One-liner |
|---|---|---|---|
| 12 | [#17](https://github.com/Captain-Food/captain-food/issues/17) two-step client write model | L | Acceptance-first as the renderer's day-one mutation convention (really #21's mutation layer). |
| 13 | [#21](https://github.com/Captain-Food/captain-food/issues/21) Leptos/WASM SDUI renderer | XXL | The customer app — the PMF unlock. Split into its 4 sub-issues before starting. |
| 14 | [#20](https://github.com/Captain-Food/captain-food/issues/20) HubRise connect flow | L | Self-serve restaurant onboarding: provision on connect + account-scoped token table. |
| 15 | [#28](https://github.com/Captain-Food/captain-food/issues/28) Avelo37 delivery adapter | XL | **post-V0** — automated delivery dispatch; do not start before the codegen wave (#25/#26/#27). |

## Done

_(moved here as issues close; rank numbers are kept, not recycled)_

| Rank | Issue | Closed |
|---|---|---|
