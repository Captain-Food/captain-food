# ADR-20260721-031734 — PM state-table rows and Postgres stores are generated from the table DSL

## Status

Accepted

## Context

The four process-manager state tables (`specs/database/tables/process_managers.yaml`,
ADR-20260719-172821) were backed by two hand-written modules written to strict, repeated
conventions: `crates/application/src/pm_state.rs` (row structs, `…StateStore` ports with `by_*`
lookups, in-memory doubles) and `crates/infrastructure/src/persistence/pm_state.rs` (Postgres
adapters: enum ordinals, `.0` newtype binds, `INSERT … ON CONFLICT (pk) DO UPDATE` upserts that
stamp `last_update_utc = now()` server-side). Hand-maintaining N copies of a mechanical pattern is
exactly the drift the codegen operating model exists to eliminate (docs/codegen-roadmap.md item 5,
issue #27), and every new PM (#20, #28) multiplies the copies: a lookup could silently diverge from
the table YAML with no gate catching it.

## Decision

Extend the `rows.rs`/`projectors.rs` emitter family in `tools/codegen-rs` with two emitters over
`database/tables/process_managers.yaml`:

- `crates/application/src/generated/pm_state.rs` — one `<Base>Row` struct and `<Base>StateStore`
  trait per table, plus the `mem::Mem<Base>State` doubles the orchestrator tests run against.
  Re-exported from the application crate root (`pub use generated::pm_state`), so the stable
  `application::pm_state` path is unchanged for every caller.
- `crates/infrastructure/src/generated/pm_state.rs` — one `Pg<Base>State` adapter per port,
  re-exported through `infrastructure::persistence` so wiring paths are unchanged.

Conventions the emitters encode (all derived from the table DSL):

1. **Base name** = the table name minus the `_process_manager` suffix, CamelCased; a single-word
   stem keeps the `Process` word for readability (`payment` → `PaymentProcess`, `cart_binding` →
   `CartBinding`).
2. **Lookups** = the pk column + every `unique: true` correlation column. Method name is the
   mechanical `by_<column minus trailing _id>` — so the `state.by` keys of processmanager.yaml map
   1:1 onto store methods when orchestrator generation lands (roadmap item 3). This renamed the
   hand-written `by_job` to `by_delivery_job`.
3. **Extra lookups** — the narrowly-scoped initiator reads a saga explicitly declares (the
   `paymentStatus(orderId)` read over `payment_process_manager.order_id`, ADR-20260720-015500) —
   are registered in a small emitter-side table (`EXTRA_LOOKUPS`), next to the hardcoded
   `paymentStatus`/`paymentStatusChanged` resolver bodies that need them. The table DSL stays
   untouched.
4. **Envelope stamp**: every table must carry a non-nullable `last_update_utc` timestamptz column
   (asserted at generation time); upserts write `now()` server-side and IGNORE the row's carried
   value — in Postgres and in the mem doubles alike.
5. **Type mapping**: scalars.yaml enums ↔ INTEGER declaration-order ordinals via
   `persistence::enum_sql::EnumOrd`; uuid/i64-backed newtypes pass by value, String-backed by
   reference; SQL `integer` is `i32` (matching the migrations' INTEGER, unlike projection rows'
   `i64`).

The hand-written modules are deleted; their mem-double tests moved to
`crates/application/tests/pm_state_mem.rs`. The journal stores (`command_journal.rs` /
`inbound_events.rs`) remain hand-written — same conventions, candidate for a follow-up slice of the
same emitter family.

## Alternatives considered

- **A `lookup: true` marker in the table DSL** for the `paymentStatus` read — cleaner derivation,
  but a spec change is out of scope for an execution loop (specs/** frozen, and issue #27 states
  "specs/** untouched"); can be proposed later from plan mode and would replace `EXTRA_LOOKUPS`.
- **Keeping the hand-written `by_job` name** via an emitter-side naming map — rejected: an
  irregular name per column defeats the mechanical `state.by` → method mapping that orchestrator
  generation (roadmap item 3) will rely on.
- **Byte-identical regeneration of the hand-written files first, swap second** — the emitters
  reproduce the API surface and behaviour exactly (parity gate = the behaviour tests), but doc
  comments come verbatim from the YAML notes rather than the hand-polished originals; chasing
  byte-parity on prose had no consumer.

## Consequences

### Positive
- New PM state tables (#20 delivery-partner, #28 connect) become spec-only: add the YAML table,
  regenerate, wire the orchestrator.
- A lookup can no longer silently diverge from the table YAML — the CI drift gate
  (`make check-drift`) now covers the PM state layer.
- Prerequisite mindset for #25 (orchestrator generation), which builds on generated state access.

### Negative
- The `paymentStatus` extra lookup lives in the emitter, not the spec — acceptable while the
  resolver bodies it serves are also emitter-hardcoded, but it should migrate to the DSL together
  with them.
- Generated doc comments are terser than the hand-written originals (YAML notes verbatim).

### Follow-up actions
- Slice 2 (same emitter family): the journal stores `command_journal.rs` / `inbound_events.rs`.
- When orchestrator generation (roadmap item 3) lands, derive the lookup set from
  processmanager.yaml `state.by` too and validate both directions.
