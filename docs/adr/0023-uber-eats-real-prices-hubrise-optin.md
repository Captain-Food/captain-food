# ADR-0023 — Real Uber Eats prices via HubRise (restaurant opt-in)

## Status
Proposed

## Context
HubRise is the integration hub between the POS, the delivery platforms (Uber Eats, Deliveroo, Just Eat)
and Captain.Food. A restaurant connected via HubRise authorizes Captain.Food to read its menus — which
include the prices it set on Uber Eats. Uber price-parity clauses exist but are contested and weakly
enforced. (Source: ADR-NEW-008; complements the HubRise-as-hub ADR.)

## Decision
When (1) the restaurant is connected via HubRise, (2) Uber Eats menus are present in HubRise, and (3) the
restaurant has **explicitly opted in**, Captain.Food may display the **real Uber Eats price** per product
(and a real "Uber" cart total) — no estimate. Formalized by: a CGU clause ("the Restaurant authorizes
Captain.Food to use, for comparison, the menu prices it entered/validated in HubRise — including
third-party platforms such as Uber Eats/Deliveroo — solely to show the customer the savings of ordering
via Captain.Food"); and an onboarding checkbox ("I authorize Captain.Food to show my Uber Eats/Deliveroo
prices for comparison").

## Alternatives considered
- Scraping Uber Eats pages — illegal/fragile; rejected (we read only restaurant-owned HubRise data).
- Estimates only — less compelling; kept as the fallback (ADR-0024).

## Consequences
### Positive
- Irrefutable, legally-defensible comparison; a selling point for the HubRise integration; no scraping.
### Negative
- Only covers HubRise-connected restaurants (a subset); some fear Uber contractual tension → opt-in must
  be very clear; needs a dedicated legal review.
### Follow-up actions
- DSL: a per-offer `uberEatsPrice` (real) on the catalog read model gated by a restaurant
  `uberComparisonOptIn` flag + the consent command/event.
