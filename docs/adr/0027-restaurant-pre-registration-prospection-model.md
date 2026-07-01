# ADR-0027 — Restaurant pre-registration & prospection domain model

## Status

Accepted

## Context

ADR-0019/0020/0021 proposed pre-registering Touraine restaurants from public data (INSEE Sirene + Google
Maps), a claim flow, and a B2B prospection pipeline. Realizing them in the DSL forced concrete
domain-modeling choices — and a Perplexity note
(`incoming_news_from_perplexity/2026-07-01-…restaurant list…`) argued for a materially different shape (a
separate `RestaurantDirectory` bounded context with SIRENE as the single source of truth). This ADR
records what we actually decided, so the model's rationale is transparent and not re-litigated each time.

The dominant constraint is **dark kitchens**: several independent restaurant brands operate under one
legal entity (one SIRET) at one address. Sirene cannot separate them; Google Business Profile usually can.

## Decision

1. **Extend the `Restaurant` aggregate** rather than introduce a separate listing/directory aggregate. A
   single generic `RegisterRestaurant` serves owners, admins and the sync; a `listingStatus`
   (`NON_PARTNER → PASSIVE_PARTNER → ACTIVE_PARTNER`) carries the partnership funnel. `accountId` is
   nullable until claim/conversion.
2. **Generic `externalIdentifiers` [{key,value}]** (siret, naf, google_place_id, hubrise_ref, …) identify a
   listing — NOT a source-specific unique key. External ids are **not assumed unique**: a SIRET is shared
   by dark-kitchen brands, so several restaurants may carry the same siret while differing by
   google_place_id.
3. **Sirene and Google are independent peer sources**, each an **Anti-Corruption Layer that calls the
   generic `Restaurant` commands "as the owner."** A restaurant may be seeded from either source; Google
   can seed the dark-kitchen brands Sirene can't. No dedicated "pre-register" command.
4. **A source/integration event carries only that source's own data.**
   `RestaurantGoogleBusinessProfileUpdated` holds only GBP-specific metrics (place_id, rating, reviews);
   general restaurant info (name, address, hours, website, tags, geo) flows through
   `Register`/`UpdateRestaurant` whatever the source — so the two sources feed the same restaurant
   side-by-side with no coupling.
5. **Claim/opt-out ownership proof is delegated to Google (GBP verification)**, validated server-side
   (mirrors the email-token verification pattern).
6. **Prospection score is a read-model projection, never an event** — a derived `score` column in
   `View_ProspectionPipeline` (formula in the view `rules`). Outreach is worker-issued commands recording
   facts on a dedicated **`Prospect`** aggregate; anti-spam (≤3 contacts, ≥7 days) are domain invariants.
7. **The sync/prospection worker is an ACL/integration, not a domain process** — it orchestrates external
   systems and issues commands. Runtime workers are deferred until app code exists; the DSL captures the
   contracts.

## Alternatives considered

- **Separate `RestaurantDirectory` bounded context + `DirectoryEntry`** (a SIRET-unique reference record,
  not an aggregate; claiming *promotes* it into a new `RestaurantActor`) — the Perplexity proposal.
  **Rejected**: a SIRET-unique `DirectoryEntry` **cannot represent dark kitchens** (the key constraint);
  treating **SIRENE as the sole source of truth** misses brands only Google sees; and it introduces a
  cross-BC data-translation handoff on claim. Our extend-Restaurant + `externalIdentifiers` + dual-source
  model keeps the sound parts (upstream ACL, weekly sync, upsert/never-delete, claim) while remaining
  dark-kitchen-capable.
- **Storing Google photos/menus** — rejected (photographer copyright; no license-clean menu feed; Maps
  ToS). Google *factual* fields (place_id, lat/long, name, address, hours) are stored under an explicit
  CTPO risk-acceptance; see `specs/integrations/google-maps.md` §5. Photos remain a UI-time, attributed,
  non-stored display.

## Consequences

### Positive
- One coherent aggregate; no cross-context handoff on claim; **dark-kitchen-capable**; multiple
  independent data sources feed the model without coupling; prospection scoring is transparent (derivable,
  auditable) and never stale in an event.

### Negative
- The `Restaurant` aggregate carries listing + operational concerns together; pre-registered listings are
  full aggregates rather than lightweight records (accepted trade-off for simplicity of the claim path).
- **De-duplicating a listing seen by both Sirene and Google** (same physical place via two ids) is not yet
  solved — future work (a reconciliation step keyed on address/geo + external ids).

### Follow-up actions
- When the app exists, build the Sirene/Google sync + prospection workers as ACLs per the contracts
  (`specs/integrations/{sirene,google-maps}.md`, the `prospection` observability contract).
- Revisit dedup once real dual-source data is observed.

## References
ADR-0019 (pre-registration), ADR-0020 (sync + prospection), ADR-0021 (GBP order button);
`specs/{entities,events,commands,actors,views,observability}.yaml`;
`specs/integrations/sirene.md` + `specs/integrations/google-maps.md`.
