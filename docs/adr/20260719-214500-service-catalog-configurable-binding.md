# ADR-20260719-214500 — Service catalog with configurable binding (local | http)

## Status

Proposed — direction agreed 2026-07-19; specification and implementation to follow.

## Context

The process managers and command handlers call outbound capabilities through application-layer port
traits (`PaymentGateway`, `DeliveryPartner`, `AuthProviderGateway`, …) implemented by adapter crates
(`crates/adapters/stripe`, `hubrise`) that today are hard-wired in-process into the Axum server. The
ports exist, but (a) they are declared ad hoc (inline `ports:` per process manager, hand-written
traits), and (b) the local-vs-remote deployment decision is code, not configuration. Calling our own
adapter over HTTP inside one deployable would be a useless network hop; hard-wiring it in-process
forever blocks splitting the deployable later.

## Decision

Introduce **services** — a spec-level catalog of the abstract APIs the domain calls — with the
implementation **binding chosen by server configuration**:

1. **`specs/services.yaml`** (new DSL source): each service declares typed operations in the house
   style — `payment: { operations: { request: {input/output/errors as $refs}, refund: {…} } }`,
   `delivery: { offer_job, cancel_job }`, `identity: { verify_phone_otp, verify_email_token }`,
   `catalog_sync: { import_catalog, sync_inventory }`, `listing_enrichment: { … }`. The abstract
   service API surface is `/external/<service>/<operation>` (e.g. `/external/payment/request`,
   `/external/payment/refund`); provider adapters keep their own surface
   (`/adapters/stripe/intentPayment`, `/adapters/stripe/refund`, `/adapters/avelo37/…`).
2. **processmanager.yaml `ports:` become `$ref`s into services.yaml** — the validator proves every
   `call` step against the catalog (operation exists, error set declared), same as every other ref.
3. **Codegen emits per service**: the Rust trait (application), the HTTP client implementation
   (infrastructure, targeting `/external/<service>/<op>` or the adapter route), the adapter-side
   Axum routes, and the composition-root **binding switch** read from configuration
   (`SERVICE_PAYMENT=local` → call the adapter crate in-process; `SERVICE_PAYMENT=http:<base-url>`
   → the generated client). Splitting the monolith becomes deployment configuration, not a rewrite.
4. **The workers follow the same model**: the projector and the saga runner already toggle
   in-process vs (future) dedicated deployable via `RUN_PROJECTOR`/`RUN_PROCESS_MANAGERS` — they are
   services under this decision, not exceptions.
5. **C4-L3 and observability bind to the catalog**: service components/relations and the span
   contracts around service calls derive from services.yaml instead of being maintained by hand.

## Alternatives considered

- **Keep ad-hoc port traits** — works (it is today's state) but leaves the catalog implicit, the
  deployment topology hard-coded, and C4/observability hand-maintained.
- **Always-HTTP internal APIs (microservices first)** — pays the network/ops tax on day one for a
  V0 that fits one deployable; the whole point is to defer that choice to configuration.

## Consequences

### Positive
- Process managers/handlers stay ignorant of transport and provider; local mode has zero HTTP
  overhead inside one app; remote mode is a config flip per service.
- One more hand-maintained surface (trait + client + routes + wiring) becomes generated from spec.

### Negative
- One more DSL source file and emitter to maintain; config matrix needs CI coverage for both
  bindings of at least one service.

## Sequencing

See docs/codegen-roadmap.md — the service catalog is item 4; the aggregate-lifecycle DSL and the
generated behaviour-test harness land first because they shrink the hand-written (misinterpretable)
surface the most.
