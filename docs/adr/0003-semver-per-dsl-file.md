# ADR-0003 — SemVer versioning per DSL file

## Status
Accepted

## Context
DSL files evolve; downstream generators and (later) deployed services need to reason about whether a
change is safe.

## Decision
Each DSL file carries a `version:` and follows SemVer: **MAJOR** = breaking change to structure or
semantics; **MINOR** = backward-compatible addition; **PATCH** = validation tightening or documentation
correction that does not break valid payloads. Every plan classifies its DSL changes as `breaking`,
`backward-compatible`, `generator-only`, `documentation-only`, or `observability-only`.

## Alternatives considered
- A single repo-wide version — too coarse; hides which contract changed.
- Git-only history — not a declared compatibility signal for consumers.

## Consequences
### Positive
- Change intent is explicit and reviewable; consumers can gate on MAJOR.
### Negative
- Authors must bump versions deliberately.
### Follow-up actions
- Consider a validator check that a structural change is accompanied by a version bump.
