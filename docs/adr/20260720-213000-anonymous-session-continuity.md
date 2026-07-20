# ADR-20260720-213000 — Anonymous checkout continuity: persistent session id, stamped through the write path

## Status

Accepted (issue #12 contract; client rules are the recorded product decision)

## Context

Acceptance-first (ADR-20260720-015500) lets a visitor pay and follow an order with no identity
beyond the client-generated `X-SESSION-ID`. Until now that was a trap: `place_order` never stamped
the session onto the `payment_process_manager` row (`session_id: None`, deferred in PR #10), so a
guest who closed the app mid-checkout — or whose client re-minted a session id on reload — lost
access to their own paid order (`paymentStatus` is ownership-scoped; #13/PR #30 opened the PUBLIC
path but the session scope had nothing to match).

## Decision

### 1. Client persistence rules (product decision, binding for #17/#21)

- **Web**: the session id lives in a **cookie** (long-lived, `SameSite=Lax`), generated on first
  load and reused on every subsequent load — never regenerated while it exists.
- **Apps (iOS/Android/desktop)**: the session id lives in the **app-level cache/keychain** and is
  reused across launches.
- The SAME session id is kept **until a `customerId` exists** (phone verification binds guest
  carts/orders via CartBindingProcess); only then may the client rotate or drop it.

### 2. Server: the envelope session is stamped onto the PM run row

`place_order` takes the dispatch-layer session as an explicit envelope parameter (NOT command
payload — ADR-0041) and stamps it onto the `payment_process_manager` row. The generated placeOrder
dispatch passes `env.session_id` through. A guest thereby resumes, after a restart, with only the
persisted session id:

- `operationStatus(messageId)` — journal row `session_id` (already stamped since ADR-20260720-015300);
- `paymentStatus(orderId)` / `paymentStatusChanged` — PM run row `session_id` (this change);
- cart — already keyed by `sessionId`.

### 3. Guest order-by-id reads: DEFERRED to phone verification

The `order` query gains **no session scope for now**: a guest tracks the checkout through
`paymentStatus`/`paymentStatusChanged` (payment terminal state + clientSecret); full order tracking
by id requires verifying a phone (CartBindingProcess then binds the guest's orders to the new
`customerId`). Rationale: the OrderTracking read model carries no `session_id` column — adding one
is a projection/spec change out of proportion to V0's confirmation screen, which the payment stream
already serves. Revisit when #14 (subscription ownership convention) or the SDUI tracking screen
demands it.

## Alternatives considered

- **Session column on OrderTracking** (guest `order` reads) — deferred, see §3.
- **Session inside the command payload** — violates the envelope/payload split (ADR-0041); the
  session is transport knowledge, rejected.
- **Extending `Actor` with the session** — would touch every handler for a field only checkout
  uses; a dedicated parameter on the one handler is smaller.

## Consequences

### Positive
- The #12 acceptance holds: build cart → placeOrder → force-close → reopen → the persisted session
  id re-owns `operationStatus` + `paymentStatus` and the checkout completes.
- The prod smoke drops its Stripe-metadata stand-in (same change): it sends `X-SESSION-ID` on
  `placeOrder` and reads the intent through the guest `paymentStatus` on `/public/graphql` — the
  daily smoke now exercises the real anonymous-checkout read path end-to-end in production.

### Negative
- Guests cannot read `order(id)` until they verify a phone (§3) — accepted for V0.
- A cleared cookie/keychain still orphans an in-flight guest checkout — inherent to anonymous
  identity; the client rules minimize the window.

### Follow-up actions
- #17/#21 implement the client rules verbatim (cookie/keychain, no rotation before `customerId`).
- Watch the next scheduled prod smoke: the guest `paymentStatus` read is its new critical step.
- Revisit §3 alongside #14.
