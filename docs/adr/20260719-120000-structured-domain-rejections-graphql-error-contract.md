# ADR-20260719-120000 — Structured domain rejections mapped to the GraphQL error contract (P-10)

## Status
Proposed — realizes the "structured typed error" follow-up flagged in ADR-0046 (Consequences →
"Error typing is interim").

**Amended by ADR-20260720-015500 (2026-07-20):** with acceptance-first mutations the contract
splits — synchronous validation failures (input shape, metadata, duplicate-payload Conflict) keep
the GraphQL `extensions.code` mapping described here; post-acceptance business rejections surface
asynchronously as `Operation.errorCode` + the interpolated message (same errors.yaml catalog).

## Context
Command handlers rejected requests with `DomainError::Invariant(String)` carrying the errors.yaml code
as an interim `"<Code>: <detail>"` string (ADR-0046). That shape is unparseable-by-contract on the wire
(clients had to substring-match GraphQL error messages), loses the error's typed context, and ignores
the localized `messages.en`/`fr` templates that `specs/errors.yaml` already declares per error. The
platform-principle **P-10** (GraphQL error contract) requires machine-readable errors: a stable code
under `extensions`, a human message, 200-with-errors semantics.

`specs/errors.yaml` is already the complete catalog: PascalCase key (= wire `code`), typed `context`
fields (some `$ref` to scalars), and `{placeholder}` message templates referencing context fields.
Nothing new needed in the DSL — only a spec-derived realization.

## Decision
1. **Structured rejection in the domain umbrella type** (`crates/domain/src/shared.rs`):
   `DomainError::Rejected { code: String, context: serde_json::Value }` — the stable errors.yaml code
   plus the error's typed context as a JSON object whose keys are the errors.yaml `context` field
   names (camelCase). Constructor `DomainError::rejected(code, context)` (debug-asserts the code is
   catalogued) and reader `DomainError::code()`. `Repository` is unchanged; `Invariant(String)` stays
   only for NON-catalogued failures (the event store's optimistic-concurrency version conflict, interim
   adapters like the fail-closed payment stand-in). Carrying `serde_json::Value` is data, not
   serialization *logic*, so the ADR-0035 domain-purity rule holds; the pragmatic JSON context (instead
   of one generated struct per error) keeps the catalog light while staying spec-derived.
2. **Generated error catalog** (`crates/domain/src/generated/errors.rs`, emitted by
   `tools/codegen-rs` from errors.yaml — the single source for wire code + localized message): per
   error one `ErrorDef` const (`code`, `message_en`, `message_fr`), the `ERRORS` table, `find(code)`,
   and `interpolate(template, context)` resolving `{placeholder}` tokens from the context object
   (unknown tokens stay visible rather than panicking — a context gap must be seen, not hidden).
3. **Handlers reject structurally**: every `application::commands` handler builds
   `reject(code, json!({ …errors.yaml context fields… }))`. `rejection_code()` reads the structured
   code first-class (and still parses the legacy string shape for interim adapters), so the behaviour
   tests and the HubRise enrich flow keep asserting through the same reader.
4. **GraphQL mapping (generated MutationRoot)**: `DomainError::Rejected` →
   `async_graphql::Error` with **`extensions.code` = the errors.yaml code**, the **interpolated `en`
   message** as the error message, and the context fields under the extensions (P-10). A legacy
   `Invariant` whose prefix is a catalogued code surfaces that code; anything else — and every
   `Repository` failure — maps to the catalogued generic `Internal` (never leaking adapter detail).

## Alternatives considered
- **One generated typed context struct per error** — maximal type safety, but heavy (a parallel struct
  family + conversions for ~50 errors) for a context that is only ever serialized onto the wire and
  into the message templates. The JSON-object context with a debug-asserted catalogued code keeps the
  spec linkage at a fraction of the surface. Can be revisited if handlers start *reading* contexts.
- **Localize at the edge from `translations.yaml`** — errors.yaml already owns per-error `en`/`fr`
  messages (they are the DSL contract); duplicating them in the UI catalog would create drift. The
  server interpolates `en` for the wire message; `fr`/locale negotiation is a later, additive step
  (the code + context on the wire already let clients localize themselves).
- **GraphQL union/result types per mutation** (errors as data) — a larger api.yaml redesign; P-10
  explicitly standardizes on `errors[]` + extensions for V0.

## Consequences
### Positive
- Clients switch on `extensions.code` (stable, spec-derived) instead of substring-matching messages;
  the typed context (ids, names, statuses) rides along for display/telemetry — the
  `business_rejected` status rules in `specs/observability.yaml` get a machine-readable error type.
- errors.yaml is now enforced end-to-end: unknown codes fail debug builds; message templates and codes
  can never drift from the spec (generated catalog).
### Negative / caveats
- `DomainError` loses `Eq` (JSON contexts are `PartialEq` only) — no current code relied on it.
- Two known context gaps where the handler cannot fill a spec'd field yet:
  `QuantityExceedsLimit.productName` (cap checked before the catalog lookup) and
  `CartRestaurantMismatch.restaurantName` in the Cart handlers (no Restaurant lookup there — the
  placeOrder path fills it). The uninterpolated `{token}` stays visible by design.
- A few rejections carry an extra non-spec `detail` diagnostic (option-selection specifics, rider
  mismatch) — additive extension fields, not part of the contract.
- The interim string-invariant adapters (payment stand-in, sirene ACL) still emit the legacy shape;
  the mapping layer covers them until those adapters are migrated.

## References
Realizes the follow-up of **ADR-0046** (write-side command handlers) against the **P-10** platform
principle (docs/adr/README.md); spec sources `specs/errors.yaml` (catalog), `specs/actors.yaml`
(`throws`); generation rule per **ADR-0034** (single Rust codegen gate); domain purity per
**ADR-0035**.
