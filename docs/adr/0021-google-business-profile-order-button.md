# ADR-0021 — Google Business Profile "Order online" button

## Status
Accepted

## Context
Google Maps/Search show an **"Order online"** button on Google Business Profile (GBP) listings, pointing
to a link the restaurant configures — an aggregator (Uber Eats) or the restaurant's own ordering system.
Since mid-2024 Google pushes a direct link; in 2026 blue buttons can go straight to the restaurant's
site. Captain.Food already builds branded per-restaurant pages `{slug}.captain.food` (ADR-014) — exactly
what the GBP button can point to. (Source: `20260627-ADR-019 …`, status ACCEPTED.)

## Decision
Integrate GBP "Order online" configuration into **restaurant onboarding from MVP**: point each
restaurant's GBP order link at `{slug}.captain.food` (or its custom domain). Dashboard flow: (1) deep
link to the restaurant's GBP management page with step-by-step help; (2) auto-generate the
copy-paste-ready `{slug}.captain.food` URL; (3) **verify** (ping) the GBP link is live — status shown in
the restaurant dashboard. Google's *"Preferred by business"* badge favors the restaurant-set link over
aggregators.

## Alternatives considered
- Rely on aggregator GBP links — sends Google traffic (and data + 30%) to Uber Eats. Rejected.
- Wait for a GBP management API to fully automate — deferred to post-MVP (manual guided config for MVP).

## Consequences
### Positive
- Native Google Maps visibility with no ad spend and **0% Google commission**; customer data stays in
  Captain.Food; strong B2B acquisition argument; SEO lift for direct-order links.
### Negative
- Some restaurateurs have low GBP literacy (mitigate with a 2-min tutorial + first-week phone support);
  relies on a Google feature that has changed before (the proprietary `{slug}.captain.food` URL is the
  hedge).
### Follow-up actions
- Onboarding UI step (~4–5 dev-days, frontend); a dashboard link-verifier; post-MVP automate via a GBP API
  if stable. DSL: optionally record GBP link configured/verified on the Restaurant.
