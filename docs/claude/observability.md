# Claude rules — observability

Observability is a **contract**, declared in `specs/observability.yaml` and validated by the codegen.
Runtime emission is **deferred until app code exists** (`apps/`/`packages/`), but the contract and rules
are authoritative now.

## Where instrumentation lives (and does NOT)

- **Yes**: GraphQL gateway, command bus, event-store adapter, event publisher, message consumers,
  projection updaters, BAM projector, HubRise ACL, Stripe adapter, middleware (see
  `specs/architecture/c4-l3.yaml` — components with `instrumented: true`).
- **No**: aggregates / pure command handlers (`instrumented: false`). Business unit tests must pass with
  no telemetry stack enabled (ADR-0016).

## Three instrumentation layers

1. Auto-instrumentation: inbound/outbound HTTP, DB, messaging, framework.
2. Framework instrumentation: command bus, event store, publisher, consumer, projection updater,
   GraphQL gateway, BAM projector — where business context is attached to technical spans.
3. Targeted business enrichment (set ONLY in middleware/decorators/adapters): `business.correlation_id`,
   `business.command_type`, `business.actor`, `business.aggregate_id`, `business.result`,
   `business.event_type`, `business.projection_name`.

## Required identifiers (ADR-0018)

`message_id`, `correlation_id`, `cause_id`, `trace_id`, `span_id`, `aggregate_id`.
- `correlation_id` — business-facing, survives the whole causality chain.
- `trace_id` — technical, may rotate across long async boundaries.
- `cause_id` — links a message to its parent; `message_id` uniquely identifies each emitted message.

## Contract shape (`specs/observability.yaml`)

Each critical workflow declares: `workflow` ($ref bindings to saga/command/events — OR a dispatch
`surface: graphql` for a PIPELINE contract binding a whole dispatch surface instead of one
command/saga/aggregate, mutually exclusive with the $ref bindings; ADR-20260721-031127),
`run_identity` (must include `correlation_id` + `trace_id`), `spans` (each with an OTel `kind` in
SERVER|CLIENT|INTERNAL|PRODUCER|CONSUMER and required attributes), `metrics` vs `business_metrics`,
`status_rules` (success | technical_error | business_rejected; `success.required_spans ⊆` declared
spans), and `latency_budget` / `error_budget`. The codegen enforces all of the above.

The `command-acceptance` contract is the surface-bound instance: it instruments the acceptance-first
write pipeline (ADR-20260720-015500) — spans `command.receive`/`command.journal`/`command.dispatch`,
ids `message_id`/`correlation_id`/`trace_id`, metrics `commands_accepted_total{channel}`,
`command_duplicates_total{channel}`, `command_sync_conflicts_total{command_type}`,
`command_completion_ms{status}` (REJECTED/FAILED split). Its latency budget binds the synchronous
acceptance path only; async completion is watched via `command_completion_ms`.

## BAM and GraphQL (runtime, deferred)

- BAM = projections over the same event stream; keep business vs technical observability separate;
  dashboards join to traces via `correlation_id` + workflow/actor/aggregate keys.
- GraphQL: HTTP 200 can still carry `errors[]`. Per operation collect `operationName`, `operationType`,
  `httpStatus`, `hasData`, `errorCount`, `graphql_error_codes`, `duration_ms`, `trace_id`,
  `correlation_id`, `actor`, `tenant_id`. Monitor gateway and application layers separately.

## Rule

If an observability contract test fails, fix instrumentation/middleware — **not the domain model**.
