# ADR-0024 — Standardized Uber Eats price estimation (when real data is unavailable)

## Status
Proposed

## Context
Most French restaurants are **not** on HubRise (they run Uber/Deliveroo directly on tablets), so there is
no reliable Uber price feed for them. But convergent sources show ~30% delivery commission (6% pickup)
and typical menu mark-ups of 30–40%. (Source: ADR-NEW-009; complements ADR-0022.)

## Decision
With no real Uber price, apply a **default mark-up coefficient per cuisine category** to each line:

| Category | Coefficient |
|---|---|
| Fast-food / burgers | 1.30 |
| Pizza / casual | 1.35 |
| Traditional | 1.40 |
| Bistronomic / premium | 1.45 |
| Food truck | 1.35 |

Compute an *"estimated Uber Eats total"* = coefficient × Captain food prices + an average delivery fee
(e.g. €3.99 urban) + typical platform fees (e.g. ~10%, configurable). Always show the disclaimer:
*"Estimate based on average commissions and mark-ups by delivery platforms in France; exact prices vary."*
Coefficients are configuration (DB / feature flag), updatable without code changes.

## Alternatives considered
- Show comparison only for HubRise restaurants — leaves most of the catalog uncovered.
- Aggressive/precise estimates — misleading-advertising risk; stay conservative + labelled.

## Consequences
### Positive
- Comparison works on 100% of the catalog, even "low-tech" restaurants; reinforces transparency with no
  third-party feed.
### Negative
- It's an estimate (clear disclaimers required); Uber could contest exaggerated gaps (stay conservative);
  coefficients need periodic review.
### Follow-up actions
- DSL: a `cuisineCategory` on the restaurant/catalog + an estimation-coefficients reference table
  (like `View_PhoneCountry`); feature-flagged rollout on pilot restaurants.
