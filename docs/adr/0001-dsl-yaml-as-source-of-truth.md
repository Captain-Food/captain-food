# ADR-0001 — YAML DSL (`specs/**`) as the functional source of truth

## Status
Accepted

## Context
Captain.Food needs the business model and contracts (events, commands, entities, scalars, errors,
actors, API, stories, behaviour tests, observability, architecture) to be unambiguous, machine-checkable,
and stable enough to generate code, docs, SQL, and SDL from — without an LLM in the generation loop.

## Decision
The YAML files under `specs/**` are the single functional source of truth. Everything else
(documentation, GraphQL SDL, view SQL, the `database.md` schema section, and later runtime code) is a
**downstream generated artifact** derived from them, not a peer source. The DSL is read-only for
autonomous/execution loops; changes go through plan mode with explicit approval.

## Alternatives considered
- Code-first (model in TypeScript), docs/specs generated from code — couples the contract to one runtime
  and language; harder to reason about and review as a product.
- Prose specs only — not machine-checkable; drifts immediately.

## Consequences
### Positive
- One reviewable contract; generated artifacts never drift (regenerate to reconcile).
- Tooling can validate referential integrity across the whole model.
### Negative
- Requires discipline: new concepts must be modeled in the DSL and validated, not coded ad hoc.
### Follow-up actions
- Keep `CLAUDE.md` "Specifications" index current; keep generated outputs out of source control (`out/`).
