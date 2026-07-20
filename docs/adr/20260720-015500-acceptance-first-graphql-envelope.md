# ADR-20260720-015500 — Acceptance-first GraphQL writes: uniform `MutationAcceptance`, the technical envelope, and the operation/payment status reads

## Status

Accepted — the API-surface half of ADR-20260720-015300 (command journal). Amends **ADR-0046**
(mutations no longer complete synchronously), **ADR-20260719-120000** (the rejection contract
splits: sync validation keeps GraphQL `extensions.code`; post-acceptance rejections surface as
`Operation.errorCode`), and **ADR-20260719-193500** (PM-table privacy: a saga row may back an
initiator-scoped transient read the saga explicitly declares).

## Context

GraphQL transport success never guaranteed business completion (P-10), yet every mutation pretended
it did: validate → handle → append → answer in one round trip, each with its own tiny
`<Name>Payload { correlationId }` (+ two outliers carrying business data: `verifyPhone`
customerId/created, `placeOrder` paymentIntentId/clientSecret). With the command journal
(ADR-20260720-015300) the honest contract is **acceptance now, outcome later** — and the spec had
already declared the outcome surface (the `Operation` type, an `operation` poll query, the
`operationStatusChanged` subscription) without a real backing.

## Decision

1. **All mutations are asynchronous, acceptance-first.** Every mutation journals the command and
   returns the ONE shared payload **`MutationAcceptance`**: the **effective technical envelope**
   echoed back — `messageId`, `correlationId`, `causeId?`, `sessionId?`, `traceId?` — plus
   `operationStatus` (`PENDING` on first acceptance; the original's current status on a duplicate)
   and `duplicate: boolean`. No per-mutation payload fields exist anymore (`payload:` is removed
   from api.yaml; the generator forbids it). Synchronous-exception mutations may be introduced
   later if a flow proves too slow — none for now.
2. **The envelope is input too — `metadata: MetadataInput`** (optional on every mutation):
   client-suppliable `messageId`, `correlationId`, `causeId`. The server computes anything missing
   (UUIDv7), validates what is provided (`ValidationError` on shape, `Conflict` on
   messageId-reuse-with-different-payload — both synchronous, before/at journal insert), and echoes
   the effective values. **`sessionId` travels as the `X-SESSION-ID` header** (client-generated
   UUID, cookie/app-cache-persisted — identifies anonymous users; the JWT stays in
   `Authorization`); **`traceId`** is derived from the inbound W3C `traceparent` (or
   server-started), response-only. Envelope ≠ payload, per ADR-0041.
3. **Outcome reads**: query **`operationStatus(messageId)`** (renamed from `operation`; the pull
   counterpart) and subscription **`operationStatusChanged(messageId)`** (declared name kept; arg
   was correlationId). Both are **PUBLIC** and **ownership-scoped in the resolver**: ADMIN, or
   JWT-subject match, or `X-SESSION-ID` match — anyone can ask, only the requester (or an admin)
   sees the row; a non-owned/unknown messageId returns null (no existence oracle). The `Operation`
   type gains `messageId` and **`errorCode`** (the stable errors.yaml code) — the async home of the
   ADR-20260719-120000 rejection contract, since a post-acceptance rejection can no longer ride the
   GraphQL error channel. Anonymous WebSocket subscribers convey `X-SESSION-ID` via the
   `connection_init` payload (browsers cannot set WS headers).
4. **Business responses move to dedicated reads.** The checkout case: `placeOrder` no longer
   returns the Stripe handle; a new query **`paymentStatus(orderId)`** + subscription
   **`paymentStatusChanged(orderId)`** (CUSTOMER) serve `paymentIntentId`, `clientSecret` (while
   the run is in flight) and the folded `PaymentStatus` — the repurposed `PaymentIntent` type.
   `verifyPhone` consumers use `me`. If checkout proves too slow this is the first candidate for
   the sync-exception escape hatch.
5. **`clientSecret` persistence** (nothing persisted it before): on the **`payment_process_manager`
   run row** (+ `customer_id`/`session_id` ownership columns), populated by `place_order`, **nulled
   when the run resolves**. A Stripe client secret is a revocable technical credential, not a
   business fact — it must never enter the append-only event log. This narrowly amends the
   ADR-20260719-193500 doctrine ("PM tables are private"): *a saga state row MAY back an
   initiator-scoped, transient read that the same saga explicitly declares* — `paymentStatus` over
   `payment_process_manager` is the first (and only) such read.

## Alternatives considered

- **Keep synchronous completion, journal as audit only** — rejected by the product owner: the
  two-step model is the honest contract, and sync-exceptions can be added back narrowly.
- **Per-command typed result types (union on Operation)** — rejected: ~70 generated result types
  for two real payloads; dedicated reads are cheaper and match CQRS (results ARE reads).
- **clientSecret in the `PaymentIntentCreated` event payload** — rejected: a credential in an
  append-only log, and payloads are business-only (ADR-0041/dsl rules).
- **sessionId inside MetadataInput** — rejected: it is transport-level identity (like the JWT),
  constant per session, needed on reads too — a header, not a per-call input.

## Consequences

### Positive
- Honest, uniform write contract; clients get one acceptance shape + one status surface for all
  ~70 mutations; retries are explicit (`duplicate: true`).
- Anonymous users are first-class: `X-SESSION-ID` scopes their operations end-to-end.
- The long-declared `operationStatusChanged` subscription finally has a real lifecycle behind it
  (journal + status bus) instead of a liveness tick.

### Negative
- **Breaking API change** (api.yaml MAJOR): every client selection set changes; checkout must
  chain acceptance → `paymentStatus` poll/subscribe before mounting the Stripe element (~one poll
  interval added latency).
- Clients must handle pending states; product/frontend design for the two-step model.
- Business rejections no longer appear as GraphQL errors — dashboards/clients must read
  `Operation.errorCode`.

### Follow-up actions
- `orderStatusChanged` still takes a correlationId arg — align it with the messageId/ownership
  convention in a follow-up pass.
- A generic per-mutation observability contract does not fit the §8 workflow schema (it binds one
  command/saga/aggregate); extend the schema with a `surface: graphql` binding kind later.
- Revisit checkout latency; if a poll interval hurts conversion, introduce the sync-exception flag.
