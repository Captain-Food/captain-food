# ADR-0002 — Codegen referential validation as the DSL schema mechanism (instead of JSON Schema)

## Status
Accepted

## Context
The source playbook proposes one JSON Schema per DSL artifact. Captain.Food's DSL is heavily
**cross-referential** (`$ref`s between scalars/entities/events/commands/errors/actors/views/api/tests/
observability/c4) — the interesting failures are *relational* (a dangling `$ref`, an event a handler
does not emit, a view column with no source event, a command no actor handles), which per-file JSON
Schema cannot express.

## Decision
Use the codegen's own referential + semantic validation (`tools/codegen/src/validate.ts`) as the schema
mechanism. `npm run validate` is the gate; the generator refuses to emit on a broken model. Each new
spec concept ships with its validation rule in the same change, and new source files are added to
`SOURCE_FILES` so their `$ref`s are checked.

## Alternatives considered
- JSON Schema per file — good for shape, blind to cross-file/relational integrity; two sources to
  maintain.
- No validation — guarantees drift.

## Consequences
### Positive
- One executable gate covers shape *and* relationships (1500+ `$ref`s, actor wiring, coverage).
- 0 errors required; the only accepted warnings are known view design-holes.
### Negative
- Validation logic is bespoke TypeScript rather than a standard schema language.
### Follow-up actions
- If an external consumer ever needs JSON Schema, generate it from the model (downstream artifact).
