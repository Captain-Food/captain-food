# ADR-0030 — Uber Eats price-comparison domain model

## Status

Accepted

## Context

ADR-0022 (price comparison in the client, product + cart), ADR-0023 (real Uber prices via HubRise opt-in),
ADR-0024 (standardized estimation when no real data), and ADR-0025 (pedagogical "who gets what" split
display) were `Proposed`. Now that the exact Captain split exists (`PaymentBreakdown`, ADR-0028), the Uber
side can be modelled and contrasted against it. This records the concrete DSL model realizing all four.

The comparison is fundamentally a **read/display concern**: for a Captain price (an offer, a cart total,
an order), show what the same purchase would cost — and how it would be split — on Uber Eats.

## Decision

1. **Estimate is the default and covers 100% of the catalog.** A single primary `Restaurant.cuisineCategory`
   (`CuisineCategory` enum) selects one mark-up **coefficient** from the reference view
   `View_UberEstimationPolicy` (FAST_FOOD 1.30 / PIZZA 1.35 / TRADITIONAL 1.40 / BISTRONOMIC 1.45 /
   FOOD_TRUCK 1.35, ADR-0024). Global split/fee assumptions (Uber commission, courier base/per-km, avg
   delivery fee, platform fee) live in a second reference view `View_UberSplitPolicy`. Both are
   **calibratable reference data** (like `View_PricingPolicy`) — tunable without code, always labelled.
2. **`cuisineCategory` is deliberately single-valued.** A restaurant can belong to several cuisines for
   **discovery** — that is the existing multi-valued `Restaurant.tags`. But the estimate needs exactly one
   coefficient, so `cuisineCategory` is the single primary/representative bucket used only for pricing.
3. **Real Uber prices (ADR-0023) are opt-in; ingestion is deferred.** `Restaurant.uberPricesOptIn` +
   a `ComparisonBasis` (ESTIMATED | REAL) on every comparison model the opt-in and provenance now. V0
   always shows ESTIMATED; the per-offer real-price ingestion via the HubRise ACL lands with the runtime.
   **No scraping — ever** (only labelled estimates or restaurant-owned HubRise data).
4. **Computed in projections, exposed as read fields — no new commands/events.**
   - Product level (ADR-0022): each offer in `View_Catalog` carries a derived `uberPrice` + `uberPriceBasis`
     (→ `Offer.uberPrice`/`uberPriceBasis`).
   - Cart level (ADR-0022): `View_Cart.uber_comparison` → `Cart.uberComparison` (`UberComparison` VO).
   - Receipt (ADR-0025): `View_OrderTracking` derives `uber_total/restaurant/rider/platform_cents` +
     `uber_basis` → `Order.uberComparison`. The Captain side is the existing exact `breakdown`; the client
     derives "you save" = Captain total − Uber total.
5. **`UberComparison` VO** = `{ total, restaurantShare, riderShare, platformShare, basis }`. Estimated split
   (ADR-0025): restaurantShare = uberFood·(1 − commission); riderShare ≈ rider_base (per-km **not** modelled
   in V0 — distance unknown); platformShare = total − restaurantShare − riderShare.

## Alternatives considered
- **Scraping Uber Eats** — illegal / ToS violation / fragile. Rejected (ADR-0022/0023).
- **Multi-valued `cuisineCategory`** — ambiguous coefficient selection ("which of 1.30/1.45?"). Rejected;
  discovery multi-cuisine stays in `tags`.
- **Real prices only (HubRise)** — leaves most of the catalog uncovered. Estimate is the default; real is
  the opt-in enhancement.
- **New comparison command/event** — the comparison is derived read state, not a fact anyone asserts.
- **Modelling courier distance for the rider estimate** — no distance in V0; base-only is honest for a
  labelled estimate.

## Consequences
### Positive
- The core promise ("cheaper + transparent than Uber") becomes concrete at product, cart and receipt.
- Coefficients/assumptions calibrate without code; estimates always labelled (misleading-advertising safe).
- Works for every restaurant (estimate), better for opted-in HubRise partners (real), no new write surface.
### Negative
- `Restaurant` grew two fields; `Offer`/`Cart`/`Order` each grew comparison fields; projections carry extra
  computation. Estimates can diverge from reality (mitigated by labelling + calibration).
### Follow-up actions (runtime, deferred)
- Per-offer real Uber-price ingestion through the HubRise ACL (flips basis to REAL for opted-in partners).
- Seed + calibrate `View_UberEstimationPolicy` / `View_UberSplitPolicy` before launch (simulation).
- The CGU/onboarding opt-in checkbox and the "you save" / disclaimer UI copy.

## References
ADR-0022/0023/0024/0025 (product); complements ADR-0028 (the exact Captain split it contrasts against).
`specs/scalars.yaml` (`CuisineCategory`, `ComparisonBasis`), `specs/entities.yaml#/UberComparison` +
`Restaurant.cuisineCategory`/`uberPricesOptIn`, `specs/views.yaml`
(`View_UberEstimationPolicy`/`View_UberSplitPolicy`, `View_Catalog`/`View_Cart`/`View_OrderTracking`),
`specs/api.yaml` (`Offer.uberPrice`, `Cart.uberComparison`, `Order.uberComparison`,
`Restaurant.cuisineCategory`).
