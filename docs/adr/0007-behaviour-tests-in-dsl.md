# ADR-0007 — Behaviour tests embedded in the DSL with a full-coverage gate

## Status
Accepted

## Context
The actor model (commands → events, with throwable errors) must be exercised and kept honest, without an
app to run yet, and without duplicating sample data across cases.

## Decision
`specs/tests.yaml` holds Given/When/Then behaviour tests over the actor model, with **centralized event
fixtures** reused across `given`/`then`. A test names the actor, a `when` (command, or event for
process-manager reactions), and asserts `then` (emitted events; `[]` = idempotent no-op) and/or `thrown`
(errors). The codegen validates: data shapes (recursive required + no unknown fields), the actor handles
the `when`, `then ⊆ emits`, `thrown ⊆` the handler's declared `throws`, and **coverage** — every inbox
message, emitted event, and throwable error must be exercised (`test-uncovered-*`).

## Alternatives considered
- Tests only in app code — impossible pre-app; no model-level coverage guarantee.
- Inline data per test — duplication and drift.

## Consequences
### Positive
- The model is provably covered (86 tests, 48 fixtures, 0 uncovered) and stays aligned with `actors.yaml`.
### Negative
- Fixtures must be maintained as the model evolves (validation forces this).
### Follow-up actions
- App-level runners later execute these cases against real handlers.
