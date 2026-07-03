# ADR-0036 — Domain topology & single-origin identity (WebAuthn passkey RP-ID)

## Status

Accepted. Extracted from the retired `specs/ARCHITECTURE_OVERVIEW.md` (the only part not already covered
by ADR-0005/0006/0015/0034/0035 or CLAUDE.md). Complements ADR-0015 (Supabase Auth) and ADR-0006 (role=path).

## Context

Captain.Food is multi-tenant: each restaurant is served from `{restaurantSlug}.captain.food` (wildcard
`*.captain.food`, tenant resolved at runtime from the `Host` header). Customers browse a restaurant's
catalog on that per-restaurant subdomain. Separately, returning customers authenticate with **WebAuthn
passkeys** (Face ID / Touch ID) via Supabase Auth, in addition to the phone-OTP path (ADR-0015).

WebAuthn passkeys are bound to a **Relying-Party ID (RP ID)** with a small, fixed number of allowed
origins (≤5). Enrolling passkeys per `{slug}.captain.food` would blow that limit and force customers to
re-enrol on every restaurant — a non-starter.

## Decision

**Identity and checkout run on a single origin — `captain.food` — never on a restaurant subdomain.**

- The passkey **RP ID is `captain.food`** (the bare domain), which also covers the whole `*.captain.food`
  space for the SMS-OTP path — one consistent identity everywhere, one passkey that works for every
  restaurant.
- Browsing/catalog stays on `{slug}.captain.food`, but the **checkout flow (cart → identify → pay)
  redirects to `captain.food`**, carrying the cart/restaurant context, so identification and payment
  always happen on the RP-ID origin.

**Domain topology** (V0):

| Host | Purpose |
|---|---|
| `captain.food` | Public marketing + customer web app **+ the single identity/checkout origin** |
| `{slug}.captain.food` | Per-restaurant ordering page (wildcard; `Host`-header tenant resolution) |
| `restos.captain.food` | Restaurant onboarding + dashboard (later) |
| `riders.captain.food` | Courier portal/docs (later) |
| `system.captain.food` | Internal admin / back-office |
| `api.captain.food` | GraphQL API, per-role paths (`/public`, `/customer`, `/restaurant`, `/rider`, `/admin`, `/external`) — see ADR-0006 |

A later custom domain (e.g. `monresto.fr`) can point at the same tenant; identity/checkout still redirects
to `captain.food`.

## Consequences
- **Positive**: one passkey RP ID → one enrolment per customer for the whole platform; small fixed origin
  set; consistent identity across OTP + passkey; checkout security concentrated on one audited origin.
- **Negative**: the cart/restaurant context must survive a cross-subdomain redirect at checkout (carry it
  in the redirect, rehydrate on `captain.food`); a per-restaurant custom domain adds a second redirect hop.

## References
Extracted from `ARCHITECTURE_OVERVIEW.md` §3 (removed). Relates to ADR-0015 (Supabase Auth wrapped behind
GraphQL), ADR-0006 (GraphQL role=path ACL), CLAUDE.md multi-tenant note.
