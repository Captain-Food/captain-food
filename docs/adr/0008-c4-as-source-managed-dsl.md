# ADR-0008 — C4 L2/L3 as source-managed, validated DSL

## Status
Accepted

## Context
Architecture views drift when drawn by hand in a separate tool. We want C4 to be reviewable,
machine-checkable, and tied to the domain model.

## Decision
C4 Level 2 (bounded contexts + containers + external systems + relationships) and Level 3 (components of
the `api` container) live as YAML under `specs/architecture/c4-l2.yaml` and `c4-l3.yaml` — part of the
validated DSL. Bounded contexts bind aggregates by `$ref` into `actors.yaml`; components bind to
aggregates (`handles`) and read models (`updates` → `views.yaml`). The codegen checks that every
aggregate is mapped to a bounded context (`c4-actor-unmapped`) and that all C4 `$ref`s resolve (no
phantom). **Structurizr DSL** is the intended generated target (P-11). This adapts the playbook's
`/architecture/` location to `specs/architecture/` so C4 is validated with the rest of the model.

## Alternatives considered
- Diagrams in a drawing tool — drift, not checkable.
- C4 under repo-root `/architecture/` — would sit outside the validated `specs/` tree.

## Consequences
### Positive
- Architecture cannot silently diverge from the actor/view model.
### Negative
- C4 YAML must be updated when aggregates/views are added (validator flags drift).
### Follow-up actions
- Emit Structurizr DSL from the C4 YAML (P-11).
