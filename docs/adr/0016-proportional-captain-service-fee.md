# ADR-0016 — Proportional Captain service fee (0% commission on food)

## Status
Proposed

## Context
The earlier model capped the buyer fee at a flat €3 regardless of basket size. That is mathematically
loss-making on large baskets: on a €200 order Stripe alone takes ≈ €3.25 (1.5% + €0.25), so a flat €3
yields a **negative** gross margin — and it penalizes exactly the high-basket group orders (Captain
Groups) the strategy wants to encourage. (Business decision from the 25–27 Jun 2026 session, file
`20260627-Captain.Food — Nouveaux ADRs Juin 2026.md`, ADR-NEW-001; supersedes the Nov-2025 fixed-10%
commission model.)

## Decision
Adopt a **Captain service fee proportional to the food sub-total (TTC)**, with **0% commission on the
food price** (the restaurant keeps 100% of the menu price). The service fee is a separate, transparent
line, split between:
- a **fixed buyer part**, and
- a **variable restaurant part proportional to the restaurant's real margin** (a food truck at 55% margin
  contributes less than a bistronomic restaurant at 70%).

Narrative preserved: *"0% commission on your food"* stays literally true — the service fee is a distinct
line, not a cut of the menu price. Calibration of the % is ADR-0017.

## Alternatives considered
- Flat €3 buyer cap — loss-making on large baskets; penalizes group orders. Rejected.
- Classic 10–30% commission on food — contradicts the core positioning.

## Consequences
### Positive
- Positive Captain margin on every basket (incl. €200+ group orders); strong B2B narrative
  ("we don't touch your base margin"); DGCCRF-friendly transparency.
### Negative
- More to explain (buyer/restaurant split); the restaurant now pays a (small) contribution; the % must
  be calibrated by simulation before launch.
### Follow-up actions
- Calibrate via ADR-0017; surface at checkout via ADR-0018. Realized in the DSL when the
  pricing/checkout phase is implemented (service-fee scalar + Order/checkout fields).
