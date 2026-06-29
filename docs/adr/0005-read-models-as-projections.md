# ADR-0005 — Read side served by `View_*` projections, never the event log

## Status
Accepted

## Context
CQRS-light + event log: the write side appends business events to `domain_events`. Queries must be fast
and shaped for the UI without scanning or joining the raw event log.

## Decision
Queries read exclusively from denormalized `View_*` read tables (`specs/views.yaml`), fed by projections
from the event log — **never** directly from `domain_events`. Each view declares its source aggregate,
the events it is `fedBy`, and per-column lineage (`from` → the exact event property), which the codegen
validates (and surfaces in the docs). GraphQL output types bind to views via `reads`. No full event
sourcing (no snapshots/replay) in V0.

## Alternatives considered
- Query the event log directly — slow, couples read shapes to write history.
- ORM over normalized tables — reintroduces the write/read coupling CQRS removes.

## Consequences
### Positive
- Read shapes are explicit, fast, and traceable to their source events (hole detection).
### Negative
- Projection code must be written and kept consistent (a runtime concern, deferred).
### Follow-up actions
- Implement projection updaters per view (see `specs/architecture/c4-l3.yaml`).
