# ADR-0022 — Uber Eats price comparison in the client (product + cart)

## Status
Proposed

## Context
Aggregators take 25–30% commission, so most restaurants mark Uber Eats prices up 30–40% vs dine-in
(a €12 dish ≈ €17.15 to absorb commission). Captain.Food's "0% commission on food + separate service fee"
becomes tangible if the customer can *see* what the same order would cost on Uber Eats. (Source:
ADR-NEW-007; modifies the Nov-2025 transparent-pricing ADR.)

## Decision
A built-in price-comparison module at two levels:
- **Product (menu):** per dish show Captain price, an *"Uber Eats price"* line (real if available via
  HubRise — ADR-0023; else estimated — ADR-0024), and a *"You save"* line (€ and %).
- **Cart (checkout/receipt):** *"On Uber Eats this order would cost ≈ XX.XX € → you save YY.YY € (ZZ%)"*,
  plus a restaurant/rider/platform split block (ADR-0025).

Guardrails: a restaurant CGU clause authorizing multi-platform price comparison when data comes from
HubRise; estimates clearly labelled *"Estimate based on average delivery-platform mark-ups in France"*
with a link to an explainer. Nominative price comparison is legally sensitive → conservative, factual
wording.

## Alternatives considered
- No comparison — leaves the core promise abstract.
- Scraping Uber Eats — illegal/ToS; excluded (only restaurant-owned HubRise data or labelled estimates).

## Consequences
### Positive
- Makes the value proposition concrete; strong differentiation; ready-made marketing content.
### Negative
- Legally sensitive (needs prudent wording + factual basis); estimates must be clearly marked; product
  complexity (analytics + per-category config + UX tests).
### Follow-up actions
- DSL: per-offer/per-cart comparison fields on the catalog/cart read models; restaurant opt-in
  (ADR-0023); estimation coefficients reference data (ADR-0024).
