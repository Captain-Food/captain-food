# ADR-0040 — Materialized read-model tables + projector mechanism

## Status

Accepted (CTPO, 2026-07-06). Extends ADR-0039 (fold generator) and ADR-0005 (read models as projections).
Realized incrementally.

## Context

ADR-0039 split read models into generated **fold views** and **materialized** ones (computed columns, or
folds the generator can't yet express). It left two loose ends: the materialized ones still lived in
`projection_views.yaml` (a file of "views" that emitted `CREATE TABLE`s — misleading), and *how* they get
filled was undefined. The CTPO asked for them to be real tables under `tables/`, filled automatically by a
projector, ideally a SQL trigger on `domain_events` (one per projected table) where possible.

## Decision

### 1. Physical form is decided by file (drop the `strategy` field)
- `specs/database/projection_views.yaml` — read models realized as SQL **VIEWS** (the generated folds).
  Named `View_*`.
- `specs/database/tables/projection_tables.yaml` — read models realized as materialized **TABLES**.
  **No `View_*` prefix** (they are real tables). Same metadata as a view (aggregate/slice/fedBy/rules/
  columns with `from` lineage) so the validator still proves API↔read-model both ways.

Naming convention, now an invariant: **`View_*` = a database VIEW; an unprefixed name = a TABLE.** The
codegen emits fold views into `views.generated.sql` and projection tables into `schema.generated.sql`
(column types resolved from the `from` lineage). The validator enforces it (`view-naming`).

### 2. `projector: app` — a Rust projector, no SQL triggers
Every materialized read-model table is maintained by an **application-layer (Rust) projector** that
subscribes to `domain_events` (in `crates/application`/`infrastructure`), declared as a **deferred runtime
contract** until `crates/` lands. `projector: app` is the only value.

SQL triggers on `domain_events` (one per table) were considered — they would give strong read-your-write
consistency with zero extra infrastructure — but **rejected**:
- Business rules (pricing split with clamping, category-tree assembly, weighted score, Uber/tip
  comparison) would leak into plpgsql, where they are untestable with the behaviour harness and duplicate
  the domain logic (`PlaceOrder` already computes the authoritative breakdown) → drift.
- A synchronous projection error would **abort the event append** — a read-model bug must never block
  recording a fact that already happened.
- Even the *mechanical* folds (`Restaurant`, `Customer`) go through the projector, for **one uniform
  mechanism** rather than a trigger/app split — simpler to reason about and test.

### 3. Guardrails
- All projection logic lives in the tested application layer; none in the database.
- The event-store append is never coupled to a read-model projection.
- The projector owns rebuild/backfill (replay from `position` 0) — no separate SQL path.

## Alternatives considered
- **SQL triggers on `domain_events` (one per table), mechanical folds generated**: rejected (see §2) —
  business logic in the DB, and a projection error aborting the event append. The strong-consistency /
  zero-infra upside didn't outweigh keeping logic testable and the write path uncoupled.
- **plpgsql triggers for ALL read models** (fully DB-resident V0): rejected, same reasons, more so.
- **Keep materialized read models in `projection_views.yaml`**: rejected — a "views" file emitting tables
  is misleading; file = physical form is clearer.

## Consequences
### Positive
- Each generated artifact matches its file; the `View_*`/unprefixed convention is unambiguous.
- One uniform projection mechanism (Rust projector); all logic testable and in the application layer.
- The event-store write path stays uncoupled from read-model maintenance.
### Negative / risks
- Read models are eventually consistent (projector lag) rather than updated in the append transaction —
  acceptable for V0; a hot read model can be revisited later.
- The projector is a deferred contract — the tables are declared but unfilled until `crates/` exists.

## Implementation (generated, incremental)
The projectors are themselves generated from the specs (spec-driven), landing in slices:
- **Slice 1 (done):** a typed `DomainEvent` enum in `crates/domain/src/generated/events.rs` (adjacently
  tagged `{eventType, payload}`) for dispatch, and the `<Table>Row` structs in
  `crates/application/src/generated/rows.rs` (one per projection table; scalars → newtypes, jsonb/entity
  columns → `serde_json::Value`, timestamps → `chrono`).
- **Slice 2 (done — option A):** the projector **wiring** is generated in
  `crates/application/src/generated/projectors.rs` — per table a `<Table>Handlers` trait (one
  `on_<event>` method per `fedBy` event) and a `project_<table>(h, state, &Envelope)` dispatch that routes
  each event to its handler, stays exhaustive/in-sync with `fedBy`, stamps `updated_at` from the event
  time, deletes on a declared `tombstone`, and passes unrelated events through untouched. The fold LOGIC
  is the hand-written `…Handlers` impl (tested app code) — generation owns only the structure, keeping
  projection/business logic out of generated code (consistent with §2). `Envelope` is hand-written glue
  in `crates/application/src/projections.rs`. All 6 tables wired; dispatch usability is unit-tested.
  Considered and rejected: auto-generating the mechanical folds (option B) — it would re-introduce
  projection logic into generated code and needs several new column modes for modest gain.

## References
Extends ADR-0039; refines ADR-0005/0035 #2. Builds on the `tables/` folder from ADR-0037.
