# ADR-0046 — GraphQL write side: CQRS command handlers with event-stream aggregate rehydration

## Status
Accepted — first realization shipped 2026-07-18 (MutationRoot + the Restaurant aggregate). Remaining
aggregates (Prospect, Catalog, Cart, Order, Customer, RestaurantAccount, Delivery) are a follow-up that
reuses this pattern.

**Amended by ADR-20260720-015300/-015500 (2026-07-20):** dispatch is now journal-then-spawn —
mutations persist a `command_journal` row and return a uniform acceptance; the handler pattern
described here is unchanged but runs asynchronously after acceptance.

## Context
The read side landed first (ADR-0039/0040 projections; the `restaurants`/`prospectionPipeline`/policy
queries). The API was read-only: `EmptyMutation`, and the only write path was the idempotent
`register_restaurant` handler used by the SIRENE sync, which appended its event with **`TODO(invariant)`
markers** because validating most invariants needs the aggregate's current state, which we had no way to
load on the write side.

The specs already define everything a real write side needs: `commands.yaml` (payloads), `events.yaml`,
`actors.yaml` (each aggregate message → the events it `emits`, the errors it `throws`), `errors.yaml`
(typed anticipated errors), `rules.yaml` (business invariants) and `tests.yaml` (Given/When/Then behaviour
cases, ADR-0032). The `<Command>Input` GraphQL input types are already generated. The open question was
**how a command handler validates invariants that depend on current aggregate state**, given V0 has **no
full event sourcing / no snapshots** (ADR-0005) and the read models are **eventually consistent**.

## Decision
One GraphQL mutation = one **thin application-layer command handler** that maps the input onto the
declared domain command, validates the `actors.yaml` `throws`, and appends the `emits` business event(s)
to the aggregate stream through the `EventStore` port.

**Invariant validation strategy (the core decision):**

1. **Intra-aggregate / state invariants → rehydrate from the aggregate's own event stream.** Added
   `EventStore::load(stream) -> (Vec<DomainEvent>, version)`; a **pure domain fold**
   (`domain::restaurant::{RestaurantState, fold}`) reconstructs current state. This is **authoritative and
   race-free** — unlike reading the (lagging) projection. Covers e.g. `RestaurantNotFound`, illegal
   lifecycle transitions, `AcceptanceModeUnchanged`, `ListingAlreadyClaimed`.
2. **Cross-aggregate / global-uniqueness invariants → read-model ports** (e.g. `SlugAlreadyTaken` via
   `RestaurantReadRepository::by_slug`). These are **best-effort / eventually-consistent** and documented as
   such; where no port exists yet the handler keeps an explicit `TODO(invariant)` naming the missing port
   (`RestaurantAccountNotFound`, `RefAlreadyUsed`, `RestaurantNotReadyForActivation`, `InvalidCurrency`).
3. **Intrinsic value invariants → inline** on the command payload.

**Optimistic concurrency:** append at the loaded expected version; **creation** commands use expected
version 0 and are **idempotent-on-conflict** (client-generated ids — a replay of a known id is absorbed as
a no-op, not a duplicate).

**Completeness (ADR-0032):** every implemented invariant maps to an `errors.yaml` error and is exercised by
its `tests.yaml` behaviour case (25 Given/When/Then for the Restaurant aggregate, each linked to its
`rules.yaml` rule).

**External seams are fail-closed:** the listing-verification invariants use new `GoogleOwnershipVerifier`
(ADR-0019) and `GbpOrderLinkProbe` (ADR-0021) ports whose interim adapters **reject/never falsely accept**
— a listing claim can never be silently granted before the real integration lands.

**Codegen:** the emitter generates a `MutationRoot` from `api.yaml` — one `<Name>Payload` (carrying
`correlationId: CorrelationId!`) and one resolver per mutation; a `wired_mutation_body` table wires the
implemented handlers, the rest stub `"not implemented"`. Same generate-from-spec rule as the query side.

## Alternatives considered
- **Validate from the read model** — simplest (repos already exist) but the projection lags the log, so a
  write-side check races (two commands both pass "slug free" before either projects). Rejected for
  intra-aggregate/state invariants; accepted only for cross-aggregate/uniqueness as a documented best-effort.
- **Full event sourcing with snapshots** — the eventual target for hot aggregates, but ADR-0005 defers
  snapshots/replay in V0. Streams are short at Tours scale, so rehydrate-per-command is cheap enough now.
- **DB constraints only / no domain invariants** — rejected: business rules (`rules.yaml`) belong in the
  domain and must produce the typed `errors.yaml` rejections, not raw constraint violations.

## Consequences
### Positive
- Authoritative, race-free intra-aggregate validation; no dependence on projection freshness for the rules
  that matter most.
- Spec-derived and testable end-to-end (behaviour tests + a DB-gated GraphQL→`domain_events` test).
- Clean growth path: other aggregates add a `fold` + handlers + `wired_mutation_body` arms; a hot aggregate
  can later gain a snapshot with no handler-signature change.

### Negative / caveats
- **Rehydration cost per command** (load + fold the stream). Fine at V0 scale; revisit with snapshots if a
  stream grows hot.
- **Error typing is interim:** `DomainError` still only models `Invariant(String)`, so rejections carry the
  `errors.yaml` code as `"<Code>: <detail>"` (a `rejection_code()` reader). A structured typed error (code +
  typed context, mapped to the GraphQL error contract P-10) is a follow-up.
- **Authorization is separate:** mutations currently stamp an anonymous PUBLIC actor; path-role enforcement
  is ADR-0006 (the per-role ACL guard), and user **identity/authn (JWT)** is a later workstream — until the
  ACL lands, the MutationRoot is exposed on every role path.
- Only the Restaurant aggregate is wired; the other aggregates return `"not implemented"` until Round 2.

## References
Realizes the write half of **ADR-0034/0035** (full-stack Rust, Clean Architecture) over the event store of
**ADR-0005/0039/0040**. Commands-from-use-cases per **ADR-0004**; completeness per **ADR-0032**; served
under the role paths of **ADR-0006** (ACL enforcement tracked separately). Fail-closed seams for
**ADR-0019/0021**. The `register_restaurant` idempotency it generalizes is also used by the SIRENE sync
(**ADR-0045**).
