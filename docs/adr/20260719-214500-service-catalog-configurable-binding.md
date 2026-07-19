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

1. **`specs/services.yaml`** (new DSL source, deliberately SEPARATE from api.yaml): each service
   declares typed operations in the house style — `payment: { operations: { request: {input/output/
   errors as $refs}, refund: {…} } }`, `delivery: { offer_job, cancel_job }`, `identity:
   { verify_phone_otp, verify_email_token }`, `catalog_sync: { import_catalog, sync_inventory }`,
   `listing_enrichment: { … }`. The HTTP binding's surface is **`/services/<service>/<operation>`**
   (e.g. `/services/payment/request`, `/services/payment/refund`); provider adapters keep their own
   surface (`/adapters/stripe/intentPayment`, `/adapters/stripe/refund`, `/adapters/avelo37/…`).
   NOTE the namespace: `/external/*` is NOT used — `EXTERNAL` is an api.yaml ROLE, so
   `/external/graphql` is already the external partners' GraphQL endpoint (role-as-path); the
   service transport must not overload it.
2. **processmanager.yaml `ports:` become `$ref`s into services.yaml** — the validator proves every
   `call` step against the catalog (operation exists, error set declared), same as every other ref.
3. **Codegen emits per service**: the Rust trait (application), the HTTP client implementation
   (infrastructure, targeting `/services/<service>/<op>` or the adapter route), the adapter-side
   Axum routes, and the composition-root **binding switch** read from configuration
   (`SERVICE_PAYMENT=local` → call the adapter crate in-process; `SERVICE_PAYMENT=http:<base-url>`
   → the generated client). Splitting the monolith becomes deployment configuration, not a rewrite.
4. **The workers follow the same model**: the projector and the saga runner already toggle
   in-process vs (future) dedicated deployable via `RUN_PROJECTOR`/`RUN_PROCESS_MANAGERS` — they are
   services under this decision, not exceptions.
5. **C4-L3 and observability bind to the catalog**: service components/relations and the span
   contracts around service calls derive from services.yaml instead of being maintained by hand.
6. **Relation to the GraphQL API (api.yaml)**: no overlap by design. api.yaml is the PRODUCT API —
   GraphQL, role-filtered (`/{role}/graphql`), consumed by UIs and external partners. services.yaml
   is the INTERNAL capability catalog — consumed by the domain through generated traits, with the
   HTTP binding as a deployment option. GraphQL never fronts a service call, and services never
   appear in the GraphQL schema.

## Naming & exposure convention (agreed 2026-07-19)

- **Operations are short domain verbs, snake_case**, grouped under their service — the service
  carries the noun, the operation is the bare intention (`payment.request`, `payment.refund`,
  `delivery.offer_job`, `identity.verify_phone_otp`). An operation name must be unambiguous WITHIN
  its service; observability always emits the qualified `service.operation` form, never the bare op.
- **Provider vocabulary never appears at the service level** (no `payment.create_payment_intent`) —
  the ACL translates names as well as payloads.
- **HTTP binding paths are DERIVED, never hand-picked**: `POST /services/<service>/<op>` with
  snake_case → kebab-case (`payment.request` → `POST /services/payment/request`,
  `delivery.offer_job` → `POST /services/delivery/offer-job`). All service operations are `POST` —
  they are commands with typed bodies; queries stay on GraphQL.
- **Adapter routes speak the provider's vocabulary** (`/adapters/stripe/payment-intents`, like
  `/adapters/stripe/webhooks` already does), and the service-op → adapter-route mapping is DECLARED
  in the spec per implementation — the name-level ACL is spec, not code.
- **Exposure is two-level: the spec bounds, the config chooses.** `bindings:` declares what a
  service MAY do — `[local]` = in-process only, its `/services/*` routes must never exist;
  `[local, http]` = a deployment may consume it remotely and/or expose it. Configuration then
  selects within those bounds per deployment: `SERVICE_<NAME>=local` (default) or
  `http:<base-url>` for how this deployable CONSUMES the service, and
  `EXPOSE_SERVICE_<NAME>=true` for whether it MOUNTS the `/services/<name>/*` routes.
  Configuration exceeding the spec's `bindings` is a startup error.

### Example — the emitter's input contract (`specs/services.yaml`)

Operations are grouped by service, and the mapping onto the provider adapter's own API is part of
the declaration:

```yaml
payment:
  description: "Payments capability the domain calls — provider-agnostic (ACL: Stripe behind it)."
  operations:
    request:
      description: "Create the payment intent for a priced checkout."
      input:
        orderId: { $ref: 'scalars.yaml#/OrderId' }
        cartId:  { $ref: 'scalars.yaml#/CartId' }
        amount:  { $ref: 'entities.yaml#/Money' }
      output:
        paymentIntentId: { $ref: 'scalars.yaml#/PaymentIntentId' }
      errors:
        - { $ref: 'errors.yaml#/PaymentDeclined' }
    refund:
      description: "Request a (possibly partial) refund of a captured intent."
      input:
        paymentIntentId: { $ref: 'scalars.yaml#/PaymentIntentId' }
        amount:          { $ref: 'entities.yaml#/Money' }
      errors: []
  bindings: [local, http]          # what deployments MAY choose; config picks within this set
  implementations:
    stripe:                        # the provider adapter (ACL) — translates names AND payloads
      routes:                      # service op → adapter route, in the PROVIDER's vocabulary
        request: 'POST /adapters/stripe/payment-intents'
        refund:  'POST /adapters/stripe/refunds'
```

From this one block the codegen emits: the `Payment` service trait (`request`/`refund` with the
typed input/output/error signatures), the HTTP client (`POST /services/payment/request`, …), the
`/services/payment/*` server routes (mounted only when exposed by config, allowed only because
`http ∈ bindings`), the Stripe-side route mapping, and the composition-root binding switch — and the
validator proves every processmanager.yaml `call` step against the catalog (`port: payment,
operation: refund` must exist, with its declared errors ⊆ the leg's error surface).

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
