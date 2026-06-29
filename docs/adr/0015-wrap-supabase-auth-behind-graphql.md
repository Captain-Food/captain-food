# ADR-0015 — Wrap Supabase Auth behind our GraphQL (synchronous, effect-only auth commands)

## Status
Accepted

## Context
Customer identity is passwordless: phone SMS OTP, plus an optional email magic link, delegated to
**Supabase Auth** (SMS via Twilio; a mock provider in dev). Previously the web client talked to Supabase
directly and only afterwards called `registerCustomer`/`identifyCustomer`. We want **all auth
orchestration behind our GraphQL** so the client has one contract and the backend stays authoritative —
while keeping the event log clean (auth send/verify must not pollute it) and the domain free of the
vendor SDK. A key open question: do we drive verification **synchronously** (the handler calls Supabase
and records the result) or **asynchronously** (await a Supabase webhook)?

## Decision
GraphQL **wraps** Supabase Auth. The orchestration is modeled as commands on the `Customer` aggregate,
executed **synchronously through the `supabase-acl` adapter**:
- `RequestPhoneVerification` / `RequestEmailVerification` / `RequestPhoneChange` — call Supabase to send
  the SMS/link; **emit no event** (auth send is not a domain fact). The first SMS is localized by an
  optional `locale` defaulted from the dialing code; later authenticated sends use the stored locale.
- `VerifyPhone` — verify the OTP, then **register-or-identify** (the backend decides), emitting
  `CustomerRegistered` (new) or `CustomerIdentified` (returning). Replaces the old client-driven
  `registerCustomer`/`identifyCustomer`.
- `ConfirmEmailVerification` / `ConfirmPhoneChange` — verify the magic-link token / OTP **server-side**,
  then emit `CustomerEmailVerified` / `CustomerPhoneChanged`.

Only the **verified identity facts** are events. Phone is entered as `dialingCode` (`+33`) +
`nationalNumber`; the backend composes the canonical E.164 `PhoneNumber`. Email is verified-only (removed
from `UpdateCustomerInfo`); language has one setter (`ChangeLanguage`).

**Synchronous, not webhook.** These are user-initiated request/response actions — the customer is waiting
— so the handler calls Supabase in-band, gets an immediate yes/no, and **atomically appends the fact or
rejects** (`InvalidVerificationCode`, `InvalidVerificationToken`, …). There is no "lost webhook" risk
because nothing is in flight: if our app is down the request just fails and the user retries. The
crash-after-Supabase-verified case is handled by **idempotency** (keyed by phone/authRef + code/token)
plus the **outbox** (event appended in the same transaction, published reliably) — not by webhooks.

## Alternatives considered
- **Webhook-driven (async)**: isolate the aggregate, let Supabase push the result. Rejected for V0 —
  delivery is best-effort (retry/dedup infra), it can't reject in-band, and the user is left waiting on
  an out-of-band callback. Kept as **future hardening** (e.g. a magic link opened on another device, or
  admin-side Supabase changes), to be modeled then as **inbound integration events** like Stripe's
  `PaymentCaptured`.
- **Trust a client claim of verification**: rejected — email/phone verification must be re-checked
  server-side against Supabase.
- **Keep `registerCustomer`/`identifyCustomer`**: rejected — leaks the new-vs-returning decision to the
  client; the OTP verify is an idempotent upsert the backend owns.

## Consequences
### Positive
- One GraphQL contract; the domain stays free of the Supabase SDK (the `supabase-acl` adapter is the only
  caller — which is also what enables the dev mock). Event log carries only verified facts.
### Negative
- Synchronous calls to Supabase sit on the request path (latency budget in `observability.yaml`
  `customer-identification`). No async resilience for abandoned magic links until the webhook is added.
### Follow-up actions
- Implement the `supabase-acl` adapter + Twilio/mock SMS when `apps/` exists; add the email-verified
  webhook as inbound events if abandonment proves material.
