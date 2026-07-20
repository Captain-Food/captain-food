# ADR-20260719-193500 — State-table process-manager runtime (executing the typed step DSL)

## Status

Accepted — implementation in progress on the `claude/process-manager-sagas-dsl-ki2tlj` branch.

**Amended by ADR-20260720-015500 (2026-07-20):** the "PM state tables are private" doctrine gains
one narrow exception — a saga row MAY back an initiator-scoped transient read the saga explicitly
declares (`paymentStatus` over `payment_process_manager`, which also gains
`customer_id`/`session_id`/`client_secret` columns).

## Context

ADR-20260719-172821 made `specs/processmanager.yaml` a typed, code-generation-grade step DSL, but the
running code was still the previous model: stateless pure deciders `(trigger, pre-loaded streams) →
Decision` executed by a fold-based runner, a Stripe adapter appending `StripeEvent-{id}` envelope
streams straight into `domain_events`, a fail-closed `CheckoutSnapshotSource` seam, and no `Payment`
or `Rider` aggregate. Several DSL legs (`RefundProcess` decisions, `CartBindingProcess` bind,
partner/rider command surface) were inert `Skip`s or missing entirely.

## Decision

Reimplement the process managers as **state-table orchestrators that execute their DSL legs**, and
make the aggregates own every fact:

1. **State rows.** Each PM run is one row in its declared table (`payment_process_manager`,
   `refund_process_manager`, `cart_binding_process_manager`, `delivery_dispatch_process_manager` —
   migrated as specced). Application-layer port traits (`pm_state`) front them; sqlx stores
   implement them. `last_update_utc` is stamped by the store (runtime envelope). Idempotency moves
   from "re-fold the target stream" to the row's `by`/`expect` checks (plus the aggregates' own
   record-idempotency as a second line).
2. **Payment aggregate.** New `domain::payment` keyed by the Stripe `PaymentIntentId` (a String
   identity, so it exposes `stream(&PaymentIntentId)`/`fold` directly instead of `impl_aggregate!`).
   `PlaceOrderProcess` delivers `PaymentIntentCreated` (with the frozen checkout) to
   `Payment-{intentId}`; the capture leg reads the snapshot back **from that stream**, retiring the
   fail-closed `CheckoutSnapshotSource` seam.
3. **Stateless Stripe ACL.** The webhook ingestor stops appending `StripeEvent-{id}` streams and
   instead delivers `PaymentCaptured`/`PaymentFailed`/`PaymentRefunded` to the Payment aggregate
   through an application use case. Dedup is the actor's business decision (fold says "already
   recorded") — no adapter idempotency table, nothing synthetic in the log. Existing
   `StripeEvent-%` history stays in the append-only log; new facts land on `Payment-%`.
4. **Send vs deliver at runtime.** `deliver` = append the fact to the target aggregate's stream
   under the saga actor identity (correlation propagated, cause = trigger id). `send` = invoke the
   target's command handler; on an event leg a rejection is logged and skipped (the DSL note), so
   the close-order leg now sends `MarkOrderDelivered` and the Order's own invariants prevent
   resurrecting terminal orders.
5. **Guard outcomes at runtime.** A thrown guard on a command leg rejects the command. On an event
   leg the runner records the typed error on the group status (`/saga` `last_error`, logged) and
   advances the checkpoint — surfaced, never wedging the group, never a silent skip.
6. **Cart binding.** `CartBindingProcess` reads the session's OPEN carts and sends
   `BindCartToCustomer` per cart; `Cart.customer_id` folds from the same-stream
   `CartBoundToCustomer`, deleting the impossible cross-stream `CustomerIdentified` projector
   routing.
7. **New command surface.** Handlers for `ApproveRefund`/`DenyRefund` (RefundProcess command legs —
   decided by the RESTAURANT for its own orders or an ADMIN), `BindCartToCustomer`, the Rider
   lifecycle, and the DeliveryJob partner/ops commands. `PaymentGateway` grows `request_refund`;
   the fail-closed stand-in declines it until the real Stripe adapter lands (nothing silently
   pretends to refund).

## Alternatives considered

- **Keep the fold-based stateless runner** — proven idempotent, but it cannot express the DSL's
  state expectations (`PENDING_APPROVAL`, dedup keys), forces every leg to re-derive context from
  streams, and leaves the adapter writing the log.
- **Generate the orchestrators from the steps now** — the DSL is codegen-grade, but hand-writing
  this first implementation keeps the generator honest (scaffolding can follow once the shape is
  proven).

## Consequences

### Positive
- Runtime finally matches the validated spec; every DSL leg is executable and testable against the
  state rows; the `/saga` status surfaces typed errors.
- Adapters are pure translators; the log contains only aggregate-owned facts.
- The checkout snapshot round-trips through the log alone — no out-of-log store, no fail-closed seam.

### Negative
- Two idempotency mechanisms coexist during the transition (row expectations + fold checks).
- `OrderTracking.payment_status` still has no feed (payment facts live on `Payment-%`, the worker
  slices `Order-%`) — pre-existing gap, now explicit; needs a cross-stream projector route keyed by
  the payload's orderId (follow-up).
- Old `StripeEvent-%` streams remain as historical envelope records.
