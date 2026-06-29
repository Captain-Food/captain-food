# ADR-0018 — Transparent service-fee display at checkout

## Status
Proposed

## Context
The new model (ADR-0016) makes the Captain service fee a **separate visible line**, not a commission
baked into the price. French (DGCCRF) and EU rules require the all-in TTC price before validation; and
research shows customers tolerate fees that are explained and perceived as fair — opacity, not amount,
drives distrust. (Source: ADR-NEW-003; replaces the Nov-2025 "transparent pricing" ADR.)

## Decision
Show **three distinct lines** at checkout:
```
Articles                  XX.XX €
Delivery                   X.XX €
Captain service fee        X.XX €   [ℹ tooltip]
─────────────────────────────────
Total                     XX.XX €
```
Tooltip: *"These fees cover the Captain.Food service (matchmaking, app, support). They replace the
commissions platforms like Uber Eats take directly from your restaurant's dishes. Here, 0% commission on
your order."* A compact mobile variant collapses delivery+service into one "Fees (detail)" line that
expands on tap.

## Alternatives considered
- Fee hidden inside item prices — less robust legally; undermines the transparency narrative. Rejected.

## Consequences
### Positive
- Regulatory-robust; reinforces the "transparent where Uber Eats isn't" narrative; educates the customer.
### Negative
- One more line to explain; needs careful copy/UX (and a compact mobile layout).
### Follow-up actions
- Client work; backend exposes the fee breakdown on the order/checkout read model (pricing phase).
