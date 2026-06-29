# ADR-0012 — Domain / infrastructure / observability separation (no OTel in aggregates)

## Status
Accepted

## Context
OpenTelemetry calls scattered through aggregates couple the business model to a telemetry SDK, make
domain unit tests require a telemetry stack, and bury business logic in instrumentation noise.

## Decision
Keep three layers separate: **domain** (aggregates, pure command handlers) carries no telemetry SDK
calls; **infrastructure/framework** (command bus, event-store adapter, publisher, consumers, projection
updaters, GraphQL gateway, BAM projector, ACL, payment adapter) carries instrumentation; **observability
middleware/decorators** attach the `business.*` attributes and identifier propagation. This boundary is
encoded in `specs/architecture/c4-l3.yaml` via `instrumented: true|false` per component. Business unit
tests must pass with no telemetry stack enabled.

## Alternatives considered
- Instrument aggregates directly — couples domain to OTel, breaks isolated testing.
- No layering — instrumentation rots into business code over time.

## Consequences
### Positive
- Domain stays clean and unit-testable; instrumentation is centralized and consistent.
### Negative
- Requires decorators/middleware plumbing at the boundaries (runtime, deferred).
### Follow-up actions
- Implement the framework-boundary instrumentation (P-03) and identifier propagation (P-02) when
  `apps/` exists.
