# ADR-0038 — Test mode: non-production entities coexisting in production

## Status

Accepted (CTPO, 2026-07-03). Domain concept; realized in the specs (entities/events/commands/rules) in a
later phase. Complements ADR-0015 (auth), ADR-0017 (payouts), ADR-0031 (delivery).

## Context

We need to exercise the **real production system** end-to-end — place an order, confirm a restaurant's
device receives it, run a rider through accept→pickup→deliver — **without polluting live data, payouts,
analytics, or sending real notifications**. This is a solved, standard problem:

- **Stripe** (canonical): every object carries `livemode: true|false`; test and live data are fully
  isolated yet coexist behind test vs live keys.
- **Uber Eats / Uber Direct**: sandbox + **test orders** / test stores.
- **HubRise**: sandbox + **test locations/accounts** — test orders flow through the integration without
  hitting the real POS.
- **Deliveroo**: partner **staging** environment with test restaurants/orders.

Distinctive here: rather than a separate sandbox *environment*, we want **flagged test entities living in
production**, so the exact prod system is validated safely.

## Decision

Introduce a domain flag **`mode: LIVE | TEST`** on the tenant-facing aggregates: **Restaurant, Customer,
Order, DeliveryJob** (a rider's test status follows the job). Rules:

- **Isolation**: a TEST customer sees and orders only from TEST restaurants. TEST orders, payments and
  delivery jobs are **excluded from payouts, analytics/BAM, and real notifications** (test notifications are
  clearly marked TEST or routed to test channels).
- **Payments**: TEST checkout uses Stripe **test** payment methods — **no live charge** (or an immediate
  auto-void); TEST orders never create a real payout/transfer.
- **Live-restaurant receipt exception** (the reason this isn't just a sandbox): a **TEST order may be placed
  against a LIVE restaurant**. It appears on the restaurant's device/POS clearly marked **TEST**, takes no
  payment and creates no payout, and is excluded from analytics — validating the real receiving path
  (printer/POS/notification) without a real transaction.
- **Test rider**: an internal RIDER may run the real accept→pickup→deliver workflow on TEST (or the flagged
  test-order) jobs in production — the real process is exercised with zero impact on live payout/analytics.

The flag is a **business fact** (it changes money/analytics semantics), so it lives on the aggregate and is
carried on the relevant events; it is set at creation and immutable thereafter.

## Alternatives considered
- **Separate sandbox environment** (distinct DB/deploy): rejected as the primary mechanism — it validates a
  *copy*, not the real prod system, and doubles infra; a coexisting flag matches Stripe/Uber/HubRise and
  meets the "test on a live restaurant" need. (A sandbox may still exist for integration dev, orthogonally.)
- **Test data only in a non-prod env**: rejected — cannot verify a live restaurant actually receives orders.

## Consequences
### Positive
- Safe end-to-end validation on the real system; onboarding demos; monitored synthetic orders.
- Matches partner conventions (Stripe/Uber/HubRise/Deliveroo), easing integration mapping.
### Negative / risks
- `mode` must be threaded through isolation checks, payout/analytics filters, and notification routing — a
  missed filter leaks test data into live metrics or triggers a real charge/payout. Enforce via `rules.yaml`
  invariants + behaviour tests (a TEST order must never produce a payout; a TEST customer must not order a
  LIVE restaurant except the explicit receipt-test path).
### Follow-up actions (spec realization)
- ✅ `Mode` scalar (LIVE/TEST) + `mode` threaded onto Restaurant/Order (entities) and the creation
  events (RestaurantRegistered/CustomerRegistered/OrderPlaced/DeliveryRequested) + commands
  (RegisterRestaurant/PlaceOrder). Optional, absent = LIVE, set at creation. Generates the `Mode` enum,
  a `ref_mode` lookup table, and `mode: Option<Mode>` on the entity structs.
- ✅ Core isolation invariant: `OrderTestModeIsolation` rule + `CannotOrderTestRestaurant` error (thrown
  by `PlaceOrderProcess`) + behaviour test — a LIVE order against a TEST restaurant is rejected; a TEST
  order may target a LIVE restaurant (receipt validation).
- ⬜ **Payout / analytics / notification exclusion** for TEST data — a **projection/read-model + BAM**
  concern (the rule states it; enforced when the `View_*`/payout/notification layers are built, not at the
  command boundary). `Customer`/`DeliveryJob` have no entity yet (event/view-modelled); `mode` on their
  creation events carries the flag.
- ⬜ TTL auto-cleanup of test data (open — a `domain_stream` retention policy on the relevant streams).

## References
Stripe `livemode`/test mode, HubRise sandbox/test locations, Uber Eats/Direct test orders, Deliveroo staging.
