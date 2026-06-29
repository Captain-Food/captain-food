# ADR-0025 — Pedagogical "who gets what" amount-split display (Captain vs Uber Eats)

## Status
Proposed

## Context
Customers rarely understand who earns what on an order. Public data: Uber restaurant commission 25–30%
(+ fixed fees); rider ≈ €2.85/trip + €0.76–0.81/km (min ~€3). Captain.Food takes 0% on food, earns via
the service fee, and keeps delivery cost transparent (Avelo37). (Source: ADR-NEW-010; complements the
Stripe Connect / Avelo37 / Open Collective ADRs.)

## Decision
On the confirmation/receipt, show a comparison block:
```
On your €XX.XX order:           On Uber Eats (~€YY.YY):
🍽 Restaurant   AA.AA €          🍽 Restaurant  ~RR.RR €
🚲 Rider        BB.BB €          🚲 Rider       ~LL.LL €
⚓ Captain.Food CC.CC €          🏢 Uber Eats   ~PP.PP €
```
Captain figures are exact (from the Stripe Connect split, ADR-0017). Uber figures are real if available
(HubRise, ADR-0023) else estimated — restaurant = Uber food × (1−0.30); rider = €2.85 + €0.80/km
(configurable); platform = remainder — labelled *"estimated figures"*. Tone stays pedagogical, not
aggressive.

## Alternatives considered
- Show only Captain's split — misses the contrast that makes the point.

## Consequences
### Positive
- "Receipt activism": the receipt shows the impact of the customer's choice; ties to Open Collective
  monthly totals; shareable marketing content.
### Negative
- Calculation complexity (esp. the Uber side) → needs good test coverage; must stay pedagogical to avoid
  a frontal conflict with Uber.
### Follow-up actions
- DSL: expose the Captain split (restaurant/rider/Captain) on the order read model; reuse ADR-0023/0024
  for the Uber side.
