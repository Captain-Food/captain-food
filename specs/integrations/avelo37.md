# Avelo37 (Delivery Partner) Integration

Avelo37 is Captain.Food's V0 delivery partner (HISTORY decision 008). Captain **dispatches** ready
DELIVERY orders to the partner and **receives** courier/status facts back. All translation goes through the
**`avelo37-acl`** Anti-Corruption Layer (c4-l3), which keeps the partner SDK out of the domain — mirroring
the `stripe-adapter`. See [ADR-0031](../../docs/adr/0031-delivery-bounded-context.md).

Delivery is modelled generically (`DeliveryProvider = PARTNER | INDEPENDENT`): the same `DeliveryJob`
lifecycle also serves **independent Captain riders** (RIDER role, via commands). Avelo37 is the PARTNER path.

## 1. Direction of flow

| Direction | Trigger | ACL action |
|---|---|---|
| **Out** (request) | domain event `DeliveryRequested` (emitted by `DeliveryDispatchProcess` when a DELIVERY order is ready) | Call the Avelo37 API to create a delivery job (pickup = restaurant, dropoff = customer). |
| **In** (report) | Avelo37 webhooks | Translate to the INBOUND facts below and record them (no command — nothing to reject). |

## 2. Inbound facts (📥 — reported, not requested)

These are recorded as-is (the request/report split, CLAUDE.md). **Idempotent on `partnerRef`** (the
partner-side delivery id).

- **`DeliveryAcceptedByPartner`** — partner accepted and assigned one of its couriers.
  Carries `partnerRef`, `courier { displayName, phone? }` (no `riderId` — not a Captain rider), and ETAs.
- **`DeliveryRejectedByPartner`** — partner declined; `DeliveryDispatchProcess` re-offers (another partner /
  independent riders) or flags for manual handling. The job stays `PENDING`.
- **`DeliveryStatusUpdated`** — status progression (`PICKED_UP`, `OUT_FOR_DELIVERY`, `DELIVERED`, `FAILED`).
  On `DELIVERED`, `DeliveryDispatchProcess` emits `OrderDelivered` to close the order.

## 3. Mapping → domain

| Avelo37 concept | Captain domain |
|---|---|
| delivery job id | `partnerRef` (scalar `ExternalReference`; idempotent key) |
| courier name / phone | `Courier { displayName, phone }` (no `riderId`) |
| pickup / dropoff | `DeliveryRequested.pickup` / `.dropoff` (`Address`) |
| partner status | `DeliveryStatus` enum (mapped in the ACL) |
| delivery fee (dynamic, partner-decided — decision 003) | `PaymentBreakdown.delivery` → `riderPayout` (ADR-0028) |

## 4. ACL / boundary rules

- The partner's raw status strings and payloads never leak into the domain — the ACL maps them to
  `DeliveryStatus` and the typed inbound events.
- Recording is **idempotent** on `partnerRef` (duplicate webhooks are safe).
- The dynamic delivery cost is settled at checkout into `PaymentBreakdown` (ADR-0028); this integration
  does not re-price.

## 5. Gaps / deferred (runtime)

1. The actual Avelo37 API/webhook contract (dispatch payload, auth, retry) — lands with the runtime.
2. Independent-rider onboarding/auth (RIDER) is a separate concern from this partner ACL.
3. A `deliveryStatusChanged` subscription for live customer tracking (poll `delivery` in the meantime).
4. Fallback routing when the partner rejects (re-offer vs manual) is an ACL/PM policy, not yet specified.
