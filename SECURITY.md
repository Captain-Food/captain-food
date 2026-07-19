# Security Policy

Captain.Food handles orders, customer contact data and payments (via Stripe), so we take
vulnerability reports seriously — thank you for disclosing responsibly.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Use GitHub's private vulnerability reporting instead: go to the repository's
**Security** tab → **Report a vulnerability** (or
<https://github.com/Captain-Food/captain-food/security/advisories/new>). Reports go privately to
the maintainers.

Please include what you can of: the affected area (API, checkout/payment flow, auth, integrations),
reproduction steps, and impact. We'll acknowledge the report, keep you informed of progress, and
credit you in the advisory if you wish.

## Scope notes

- Payment card data never transits our systems — payment is handled by Stripe. Issues in the
  payment *flow* (order/refund state, webhooks) are absolutely in scope.
- The deployed V0 service is at `live.captain.food`; please keep testing non-destructive and
  proportionate (no DoS, no bulk data extraction).

## Supported versions

Only the latest state of `main` (which is what is deployed) receives security fixes.
