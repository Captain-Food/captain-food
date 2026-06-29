# ADR-0009 — Observability contracts embedded in the DSL

## Status
Accepted

## Context
"Add tracing later" produces systems that are implemented but not diagnosable. Critical workflows need a
declared, reviewable, testable observability contract — and it should be bound to the domain so it can't
drift.

## Decision
`specs/observability.yaml` declares an observability contract per critical workflow (initially
`place-order` and `refund`): `workflow` (`$ref` bindings to saga/command/events), `run_identity`
(mandatory `correlation_id` + `trace_id`), `spans` (each with an OTel `kind` and required attributes),
`metrics` vs `business_metrics`, `status_rules` (success | technical_error | business_rejected, with
`success.required_spans ⊆` declared spans), and `latency_budget`/`error_budget`. The codegen validates
this shape and the bindings. Runtime emission and contract tests are deferred (P-01…P-03, P-08) until
app code exists.

## Alternatives considered
- Observability documented in prose / dashboards only — not bound to the model, not checkable.
- No contract — "implemented but undiagnosable" outcome.

## Consequences
### Positive
- A workflow can be asserted diagnosable; the contract is reviewed alongside the domain change.
### Negative
- Contracts must be written and kept in step with the workflows.
### Follow-up actions
- Implement framework instrumentation + in-memory-exporter contract tests when `apps/` exists.
