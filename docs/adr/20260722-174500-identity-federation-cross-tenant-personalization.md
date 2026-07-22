# ADR-20260722-174500 — Federated customer identity & consent-gated cross-tenant personalization

<!-- Filename: docs/adr/20260722-174500-identity-federation-cross-tenant-personalization.md -->

## Status

**Proposed** — pending product-owner acceptance **and DPO / CNIL legal review**. This ADR fixes the
*technical* framework only; the lawful-basis determination is explicitly deferred to counsel (see
Decision §6). Nothing here is legal advice. Complements **ADR-0015** (Supabase Auth wrapped behind
GraphQL), **ADR-0036** (single-origin identity), **ADR-0006** (role=path ACL), **ADR-0042** (EU/Frankfurt
residency); relates to **ADR-20260722-091500/-160000** (the two customer front offices).

## Context

Captain.Food serves two customer-facing front offices split by host (ADR-20260722-091500/-160000): the
**marketplace** (`live.captain.food` → bare `captain.food`) and each restaurant **storefront**
(`{slug}.captain.food`). The product owner wants:

1. **One identity** — a customer must not create a separate account per restaurant; the same login works on
   the marketplace and on every storefront.
2. **Cross-restaurant suggestions** — the marketplace uses a customer's order history *across all
   restaurants* to recommend places ("better suggestions").
3. **Restaurant isolation** — restaurants remain isolated from one another (no restaurant sees another's
   customers).

…and asked whether (2) is legally permissible, and whether a Supabase **"Login with Captain.Food"** would
help.

Two halves, very different maturity:

**The identity half is already the architecture.**
- Single-origin identity (**ADR-0036**): the WebAuthn RP-ID and checkout run on `captain.food`, which covers
  the whole `*.captain.food` space — one passkey / one OTP identity everywhere.
- Supabase Auth is wrapped behind our GraphQL (**ADR-0015**); the domain `Customer` is a single **global
  aggregate keyed by phone / `authRef`** (`specs/actors.yaml` `Customer`; `CustomerRegistered`/
  `CustomerIdentified` in `specs/events.yaml`). `View_Customer`
  (`specs/database/tables/projection_tables.yaml`) is **one row per phone/`auth_ref`**, already holding
  cross-restaurant favorites, preferences, ratings and addresses. There is **no per-restaurant customer**
  today; dedup is by phone/email at the auth boundary (`PhoneAlreadyInUse`/`EmailAlreadyInUse`).
- Isolation already holds: `orders`/`order` ownership is "enforced server-side" (`specs/api.yaml`); a
  CUSTOMER sees their own orders across restaurants, a RESTAURANT sees only its own — reinforced by the
  nav-edge field ACL (ADR-20260720-230000 / #22).

**The personalization/consent half is greenfield.**
- There is **no customer consent concept** anywhere in the specs (the only "consent" in the repo is a
  *restaurant's* listing consent, ADR-0019 / `sirene.md`). `CustomerPreferencesSet` carries dietary/cuisine
  *discovery* prefs, not consent.
- No privacy/GDPR/DPIA/data-controller document exists — only **data residency** (EU/Frankfurt, ADR-0042).
- The personalization surface today is `restaurants(list: RECOMMENDED | ORDER_AGAIN)`
  (`specs/scalars.yaml#/RestaurantListKey`), "resolved by the read model" at query time off the one global
  Customer row — with **no consent gate**.

So the identity goal is essentially met; the risk is concentrated in (2). Using a customer's orders *across
restaurants* to build a profile and rank recommendations is **behavioural profiling of personal data** under
the GDPR (V0 is Tours, France → CNIL). That is a lawful-basis and controllership question, not a technical
one — but the system should be *built* so that whatever counsel decides is cheap to honour.

## Decision

**1. One identity, platform-wide — make the existing invariant explicit.**
A customer has a single Captain.Food identity (Supabase Auth user, keyed by phone/`authRef`) → one global
`Customer` aggregate → usable on `captain.food`, `live.captain.food`, and every `{slug}.captain.food`. **The
`Customer` aggregate and `View_Customer` read model must never be partitioned per tenant**, and no new
per-restaurant customer record may be introduced. Reaffirms ADR-0015/0036; recorded here as a standing
invariant so it can't silently regress as the storefront/marketplace renderers land.

**2. Data-controller boundary (recommended; final classification pending DPO).**
- **Captain.Food is the controller** of the platform identity/account and the **cross-restaurant marketplace
  profile** — the aggregated order history, favorites, preferences, and any suggestion/ranking derived from
  them.
- **Each restaurant is a controller of its own fulfilment data only** — the orders placed *with it* and the
  PII it needs to cook/deliver. Whether the restaurant is a *separate* or a *joint* controller (Art. 26)
  with Captain.Food for the order transaction is a **DPO determination**, flagged, not decided here.
- **No restaurant→restaurant data flow is created.** Isolation stays enforced by the existing server-side
  ownership checks and nav-edge field ACL (#22).

**3. Two personal-data uses, gated differently.**
- **(a) A customer's own account & history across restaurants** — `me`, the customer's own `orders`,
  favorites — is data shown to the data subject themselves and necessary to provide the service
  (performance-of-contract basis). **No new consent** introduced for (a).
- **(b) Cross-restaurant behavioural personalization** — using order history *across restaurants* to
  rank/recommend (`RECOMMENDED`, "places you might like") — is **profiling and is consent-gated, default OFF
  (opt-in)**. With consent absent, `RECOMMENDED` degrades to a non-behavioural ranking and only
  `ORDER_AGAIN` (the customer's own explicit re-order history) is used — the basis for `ORDER_AGAIN` itself
  to be confirmed with the DPO.

**4. Model personalization consent as a first-class, event-sourced fact — proposed DSL, deferred to a
follow-up issue.**
A `CustomerPersonalizationConsentGranted` / `…Revoked` event on the `Customer` aggregate, a
`SetPersonalizationConsent` command, a `personalization_consent` column on `View_Customer`, and a consent
check in the `restaurants(list: RECOMMENDED)` resolver. Event-sourced so both the current consent state
**and its full history** are provable for a DPIA / CNIL audit — the same rigor the write path already gives
every other fact. This ADR records the direction; the concrete spec change carries its own ADR-0032
completeness (behaviour test + `rules:` link + story-map step) in a separate issue.

**5. "Login with Captain.Food" (OIDC) — roadmap position, not V0.**
Within `*.captain.food`, single-origin identity (ADR-0036) already yields SSO by construction — one identity
origin, one session/passkey covering every subdomain — so **no OAuth/OIDC provider is needed for V0**. A
first-class **"Login with Captain.Food" OIDC provider** (which Supabase can host) becomes worthwhile only to
extend identity to origins **outside** `*.captain.food`: the **custom restaurant domains** ADR-0036
anticipates (e.g. `monresto.fr`), native mobile shells, or third-party surfaces. **Deferred, post-V0.**

**6. The legal determination is out of scope of this ADR and pending DPO / CNIL review.**
This ADR deliberately fixes only the *technical* framework so that either legal outcome is cheap: the
consent gate (§4) is present if counsel requires consent for (b); its default/scope can be adjusted if a
lighter basis (e.g. a documented legitimate-interest assessment) is approved. **This ADR must not be read as
asserting the right to profile across restaurants** — that right depends on the controllership model (§2), a
lawful basis (realistically consent for §3b), purpose-limitation/transparency (a privacy notice covering
cross-restaurant use), and likely a DPIA — all owned by the DPO/counsel.

## Alternatives considered

- **Per-restaurant customer records linked by a global identity** (federated data, not just federated
  login). Maximal isolation, but it fights the existing global-`Customer` model, complicates
  `me`/idempotency/dedup, and makes cross-restaurant suggestions harder — for no privacy gain over §2's
  "one controller-held profile + server-side per-tenant visibility". Rejected.
- **No consent gate; rely on legitimate interest for all personalization.** Simplest to build, but
  cross-merchant behavioural profiling under CNIL leans toward requiring consent; shipping without the gate
  would be expensive to retrofit and risky. Rejected in favour of building the gate now, default-off.
- **A full "Login with Captain.Food" OAuth/OIDC provider in V0.** Unnecessary while every surface is under
  `*.captain.food` (single-origin SSO already covers it) and adds an IdP to operate/secure. Deferred to when
  identity must cross to external origins (§5).
- **Shared parent-domain session cookie (`Domain=.captain.food`) for SSO** instead of the ADR-0036
  redirect-to-single-origin model. Simpler cross-subdomain login, but a `.captain.food` cookie is readable
  by every subdomain (weaker isolation, worse if restaurants ever get injectable/custom surfaces). ADR-0036's
  model is retained.

## Consequences

### Positive
- Unified-login UX is **unblocked now** (it is already the architecture); this ADR just protects the
  invariant and names the boundary.
- Marketplace cross-restaurant personalization can ship **behind a provable, event-sourced consent gate**,
  default-off — auditable for a DPIA/CNIL.
- Restaurant-to-restaurant isolation is **preserved** and made explicit (controller boundary + existing #22
  ACL).
- Whatever the DPO decides, the change is cheap: the gate is present; only its default/scope moves.

### Negative / deferred
- The **controllership classification** (Captain.Food sole controller vs joint controller with restaurants),
  the **consent default/scope**, and whether **`ORDER_AGAIN`** needs consent are all **pending DPO** — this
  ADR recommends but does not finalize them.
- Real legal work remains outside code: **privacy notice**, **DPIA**, and **controller/processor contracts**
  with restaurants.
- The consent mechanism itself is **not yet implemented** — it is a follow-up spec change (below), so
  `RECOMMENDED` must not consume cross-restaurant history until that gate exists.

### Follow-ups (separate issues, not this change)
1. Implement the consent event/command/`View_Customer` column/`restaurants(list: RECOMMENDED)` gate with
   ADR-0032 completeness (test + rule + story).
2. Product/legal: privacy notice, DPIA, and the controller/processor arrangements with restaurants.
3. Post-V0: the "Login with Captain.Food" OIDC provider, when identity must reach non-`*.captain.food`
   origins (custom domains / native / third-party).

## References

- **ADR-0015** — Supabase Auth wrapped behind GraphQL · **ADR-0036** — single-origin identity (RP-ID
  `captain.food`) · **ADR-0006** — role=path GraphQL ACL · **ADR-20260720-230000** (#22) — nav-edge field
  ACL · **ADR-0042** — EU/Frankfurt data residency · **ADR-0019** — restaurant listing consent (naming
  precedent) · **ADR-0032** — completeness gate · **ADR-20260722-091500 / -160000** — the two customer
  front offices.
- Specs: `specs/api.yaml` (`orders`/`order`/`me`/`restaurants(list:)`), `specs/scalars.yaml`
  (`RestaurantListKey`, `UserType`), `specs/events.yaml` (Customer events),
  `specs/database/tables/projection_tables.yaml` (`Customer` view), `specs/integrations/supabase.md`.
