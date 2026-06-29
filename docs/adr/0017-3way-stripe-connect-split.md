# ADR-0017 — 3-way Stripe Connect split with proportional service fee

## Status
Proposed

## Context
Payments use **Stripe Connect "Separate Charges & Transfers"** (`transfer_group`). One PaymentIntent on
the Captain platform account (merchant of record), then transfers to the restaurant and the rider after
`payment_intent.succeeded`; Captain's net fee stays on the platform account. Transfers must be created
backend-side after capture — it cannot be one API call. (Source: ADR-NEW-002; modifies the Nov-2025
3-way-split ADR — mechanics kept, amounts recomputed for ADR-0016.)

## Decision
Compute the split **server-side before creating the PaymentIntent**:
```
PaymentIntent.amount = food + delivery + buyer_service_fee     (transfer_group = ORDER_{id})
on payment_intent.succeeded:
  Transfer → rider account      = delivery_total
  Transfer → restaurant account = food - restaurant_service_contribution
  remainder on Captain account  = buyer_service_fee + restaurant_service_contribution (net of Stripe)
```
Fee split (indicative, to calibrate): `F = 5%` of food TTC; `buyer_part = 60%`;
`score_margin = clamp((margin - 55%)/(70%-55%), 0, 1)`; `F_buyer = 60%·F`,
`F_restaurant = 40%·score_margin·F`. Captain is merchant of record → no extra PSP/EMI license (PSD2-ok).

## Alternatives considered
- Destination charges / single-call split — can't fan out to multiple connected accounts from one buyer
  charge. Rejected.
- Monthly manual settlement — no real-time payout, reconciliation burden. Rejected.

## Consequences
### Positive
- Real-time per-order settlement (~30 min), no month-end invoicing, full Stripe traceability
  (Open-Collective-compatible), PSD2-compliant with no extra licensing.
### Negative
- Restaurant transfer amount is dynamic (margin-dependent) → must be computed before each PaymentIntent;
  refunds require a documented transfer-reversal procedure.
### Follow-up actions
- Model `MoneyCents`-based service-fee + split fields on the checkout/order events when the pricing phase
  lands; document the refund reversal flow.
