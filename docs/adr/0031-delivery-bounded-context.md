# ADR-0031 — Delivery bounded context (DeliveryJob + dispatch)

## Status

Accepted

## Context

Delivery was scaffolded but unbuilt: `DeliveryJobId`/`RiderId` scalars, a placeholder `Rider` type, the
`RIDER` role + `rider` persona, the `delivery-partner` (Avelo37) external system, and `DeliveryJob` as a
reserved event-stream prefix — but there was **no delivery aggregate, dispatch, or read model**. `Order`
already owned `MarkOrderReady`/`MarkOrderDelivered`; errors `DeliveryAddressRequired`/`OutsideDeliveryArea`
existed (validated at `PlaceOrder`). HISTORY decision 008 makes **Avelo37 the MVP delivery partner** with a
**dynamic, partner-decided delivery cost** (decision 003) that already flows through
`PaymentBreakdown.delivery` → `riderPayout`. This records the concrete delivery domain.

The product must **not** commit to a single fleet model: Captain will likely have **both** delivery
**partners (with their own couriers)** and **independent riders**.

## Decision

1. **New `delivery` bounded context** = the **`DeliveryJob`** aggregate + the **`DeliveryDispatchProcess`**
   process manager. The `RIDER` role is wired to this context (its read/discovery home; previously neutral).
2. **One lifecycle, two fulfilment paths converge** on `DeliveryJob` (`DeliveryStatus`:
   PENDING→ASSIGNED→PICKED_UP→OUT_FOR_DELIVERY→DELIVERED/FAILED/CANCELLED); `DeliveryProvider = PARTNER | INDEPENDENT`:
   - **Partner (Avelo37):** the `avelo37-acl` dispatches on `DeliveryRequested` and translates partner
     webhooks into **INBOUND** facts `DeliveryAcceptedByPartner` / `DeliveryRejectedByPartner` /
     `DeliveryStatusUpdated` (no command — nothing to reject; idempotent on `partnerRef`). Mirrors
     `stripe-adapter` and the "report, not request" rule (CLAUDE.md).
   - **Independent rider (`RIDER`):** commands `AcceptDelivery`→`DeliveryAcceptedByRider`,
     `ConfirmPickup`→`DeliveryPickedUp`, `CompleteDelivery`→`DeliveryCompleted`.
3. **`DeliveryRequested` is PM-emitted, not a command** — `DeliveryDispatchProcess` reacts to
   `OrderMarkedReady` (DELIVERY orders only) and emits it, exactly as `PlaceOrderProcess` emits `OrderPlaced`.
   No `RequestDelivery` command (nothing to validate at ready-time; the address was validated at checkout).
4. **The order is auto-closed on delivery completion:** `DeliveryDispatchProcess` emits `OrderDelivered`
   on `DeliveryCompleted` (independent) or `DeliveryStatusUpdated`=DELIVERED (partner). `OrderDelivered`
   therefore has **two emitters** — the delivery PM (delivery orders) and the `Order` aggregate's manual
   `MarkOrderDelivered` (COLLECTION hand-over at pickup). This is intentional and the validator accepts it.
5. **Reads:** `View_DeliveryJob` is the operational board (rider job list, restaurant delivery board, admin);
   `View_OrderTracking` also mirrors `delivery_status`/`courier`/`estimated_dropoff_at` so the customer's
   existing order query shows live progress. API: `DeliveryJob` type (replaces the placeholder `Rider`),
   `delivery`/`myDeliveries`/`restaurantDeliveries` queries, `accept/confirm/complete/cancelDelivery`
   mutations, `Courier` VO `{ displayName, phone?, riderId? }` (riderId set only for an independent rider).

## Alternatives considered
- **Partner-only (Avelo37)** — contradicts the "partners AND independent riders" intent. Rejected.
- **Fold delivery into the `Order` aggregate** — delivery is independently long-lived, has its own status
  machine + external partner + rider actor, and is a distinct context. Rejected (separate aggregate).
- **`RequestDelivery` command / ACL-issued `MarkOrderDelivered`** — the codebase's PMs emit events directly
  (`PlaceOrderProcess`); a command would add surface with nothing to validate. Rejected.
- **Single emitter for `OrderDelivered`** — would force either a delivery-only or collection-only close.
  Dual emission models both hand-over paths honestly. **Fallback** if a future emitter forbids multi-emitter:
  keep delivery terminal as `DeliveryCompleted`/`DeliveryStatusUpdated`=DELIVERED and close the order via a
  manual `MarkOrderDelivered`.

## Consequences
### Positive
- Delivery is a first-class context supporting partners and independent riders on one lifecycle; the RIDER
  role/persona is realized; customers get live tracking; the partner integration follows the proven Stripe
  inbound-fact pattern.
### Negative
- `OrderDelivered` has two emitters (documented). `View_OrderTracking` cross-references the DeliveryJob
  (correlated by `order_id`). New surface: 1 aggregate, 1 PM, 8 events, 4 commands, 3 errors, 1 view.
### Follow-up actions (runtime, deferred)
- The `avelo37-acl` (dispatch out, webhooks in, idempotent `partnerRef`), independent-rider onboarding/auth,
  the delivery-fee quote feeding `PaymentBreakdown.delivery`, and a `deliveryStatusChanged` subscription for
  live tracking (polling via `delivery` in the meantime).

## References
HISTORY decisions 003/008; `specs/integrations/avelo37.md`; complements ADR-0028 (delivery fee → riderPayout).
`specs/scalars.yaml` (`DeliveryStatus`, `DeliveryProvider`), `specs/entities.yaml#/Courier`,
`specs/{events,commands,errors}.yaml` (Delivery*), `specs/actors.yaml` (`DeliveryJob`,
`DeliveryDispatchProcess`), `specs/views.yaml#/View_DeliveryJob`, `specs/api.yaml` (`DeliveryJob` type +
delivery queries/mutations), `specs/architecture/c4-l2.yaml` (`delivery` context) + `c4-l3.yaml`
(`avelo37-acl`).
