# ADR-20260721-031127 — Observability `surface` binding kind + the generic `command-acceptance` contract

## Status

Accepted — implements the "extend the schema with a `surface: graphql` binding kind" follow-up of
ADR-20260720-015500 (#16). Refines the §8 contract schema (ADR-0009 lineage); no change to any
existing per-workflow contract.

## Context

Acceptance-first (ADR-20260720-015500) changed the shape of EVERY write: mutations journal the
command and answer `MutationAcceptance`; the old "GraphQL error = rejection" signal is gone,
replaced by `command_journal` terminal statuses (ADR-20260720-015300). Nothing measured that
pipeline: #19 (checkout latency) was flying on smoke-test timings, and the playbook rule "every
critical workflow must have an observability contract" was violated by the most critical workflow
of all — the dispatch surface itself.

A contract for it does not fit the §8 schema: `workflow` had to bind ONE `command` and/or
`saga`/`aggregate` by `$ref`, but the acceptance pipeline spans ~70 mutations. One contract per
mutation would be noise; one contract bound to a fake command would be a lie.

## Decision

1. **New binding kind in §8**: a contract's `workflow` may declare `surface: <kind>` INSTEAD of
   the `$ref` bindings. Known surfaces: `graphql` (the BFF dispatch). Validator rules:
   - `obs-no-workflow-binding` now accepts `surface` as a valid binding;
   - `obs-surface-unknown` — the surface must be a known kind (`graphql` for now);
   - `obs-surface-exclusive` — a surface contract must NOT also bind a
     `command`/`saga`/`aggregate`: it deliberately binds the whole pipeline, per-workflow
     contracts remain the deep-dive complement.
   All other §8 rules (mandatory `correlation_id`/`trace_id`, span kinds, `success.required_spans
   ⊆ spans`) apply unchanged. The doc emitters render the surface binding; a surface contract
   files under `cross-cutting` (no owning bounded context).

2. **The `command-acceptance` contract** (`specs/observability.yaml`, `surface: graphql`,
   criticality high):
   - spans `command.receive` (SERVER) / `command.journal` (INTERNAL, `journal_status`
     RECEIVED | duplicate | conflict) / `command.dispatch` (INTERNAL);
   - run identity `message_id` / `correlation_id` / `trace_id` / `command_type` / `channel`
     (scalars `CommandChannel` — the channel split becomes fully meaningful once #15 journals
     WORKER sends);
   - metrics `commands_accepted_total{channel}`, `command_duplicates_total{channel}` (client
     retry correctness, #17), `command_sync_conflicts_total{command_type}` (messageId reuse with
     a different payload — the sync Conflict of ADR-20260720-015500 §2),
     `command_completion_ms{status}` with the REJECTED/FAILED split — #19's decision data;
   - status rules: a duplicate IS a successful acceptance; a business rejection is the journal's
     terminal REJECTED (surfaced as `Operation.errorCode`), never a GraphQL error;
   - latency budget (p95 150 ms / p99 400 ms) binds the SYNCHRONOUS acceptance path only —
     async completion is watched via `command_completion_ms`.

Runtime emission stays contract-only until the OpenTelemetry layer exists (project status);
instrumentation will live in the GraphQL dispatch middleware, never in handlers (ADR-0016).

## Alternatives considered

- **One contract per mutation** — rejected: ~70 near-identical contracts, unmaintainable, and
  the pipeline (journal, duplicates, dispatch) is genuinely one thing.
- **Bind the contract to a representative command** (e.g. `PlaceOrder`) — rejected: lies about
  scope; validator would enforce nothing about the other mutations.
- **A separate top-level `pipelines:` section in observability.yaml** — rejected: the contract
  shape (ids/spans/metrics/status rules/SLOs) is identical; a binding-kind variant keeps one
  schema and one validator path.

## Consequences

### Positive
- The playbook rule holds again: the most critical "workflow" (every write) has a contract.
- #19 gets real decision data (`command_completion_ms`), #17 gets duplicate-rate visibility,
  silent duplicate storms / journal stalls become visible-by-contract.
- The binding kind is extensible (a future `surface: webhook` or `surface: worker-drain` is a
  one-line SURFACE_KINDS addition).

### Negative
- A surface contract's `emits`/`inbound` are empty — the codegen cannot prove event lineage for
  it (that proof stays with the per-workflow contracts).
- `channel` covers WORKER only after #15 lands (accepted: the contract is forward-declared).

### Follow-up actions
- #15: journal the HubRise enricher / SIRENE worker sends so `{channel}` is complete.
- #19: wire the checkout-latency decision to `command_completion_ms` once emission exists.
- When the OpenTelemetry layer lands, instrument the GraphQL dispatch middleware against this
  contract (spans + metrics above), keeping handlers telemetry-free (ADR-0016).
