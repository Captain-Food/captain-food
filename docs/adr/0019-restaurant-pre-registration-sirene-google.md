# ADR-0019 — Restaurant pre-registration via INSEE Sirene + Google Maps enrichment

## Status
Proposed

## Context
Marketplace chicken-and-egg: no restaurants → no customers, and vice-versa. Captain.Food pre-registers
Touraine food establishments before any formal agreement, to launch with instant coverage and a B2B
prospecting pipeline. (Source: ADR-NEW-004.)

## Decision
Three restaurant listing statuses:

| Status | Data shown | Orderable | Agreement |
|---|---|---|---|
| `non_partenaire` | public Sirene + Google | ❌ | ❌ |
| `partenaire_passif` | public + HubRise menu | ❌ | ✅ HubRise sync |
| `partenaire_actif` | full profile | ✅ | ✅ signed contract |

**Sources:** (1) **API Recherche d'Entreprises / Sirene INSEE** (Etalab open data, commercial use OK) —
SIRET, name, address, active/closed, creation date, NAF; target NAF `56.10A/B/C, 56.21Z, 56.29A/B`, postal
codes `37000/37100/37200/37300/37270/37250/37400`. (2) **Google Maps Places API** (enrichment only:
rating, reviews, hours, phone, website, photo).

**Forbidden:** scraping Uber Eats/Deliveroo; importing third-party menus/prices/photos without the
restaurant's consent; implying a non-partner is an active partner. Every non-partner card carries an
**opt-out**: *"This is my restaurant — edit / remove my listing."*

## Alternatives considered
- Manual onboarding only — too slow to solve chicken-and-egg.
- Scraping aggregators — legal/ToS risk; explicitly excluded.

## Consequences
### Positive
- Instant Touraine coverage; auto-fed B2B pipeline; SIRET speeds Stripe Connect onboarding on conversion.
### Negative
- Must honor opt-out (edit/remove); non-partner cards without a menu don't convert to orders; depends on
  Sirene freshness (closure lag).
### Follow-up actions
- DSL: a Restaurant `listingStatus` + pre-registration/claim/opt-out commands & events; `sirene` and
  `google-maps` external systems + ACL; the sync cron is ADR-0020.
