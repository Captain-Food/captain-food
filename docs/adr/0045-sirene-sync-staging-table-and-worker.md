# ADR-0045 — SIRENE sync: raw-ingestion staging table + on-app worker (decouple CI from the domain)

## Status
Accepted — **supersedes the interim direct-write `sirene_sync` design** shipped 2026-07-17. The specs/code
realization is a follow-up (see Follow-up actions); the interim binary stays until the worker lands.

## Context
ADR-0019/0020/0027 established SIRENE + Google pre-registration, an automated sync cron, and the
prospection domain model. The **first implementation** (2026-07-17) is a GitHub Actions job
(`.github/workflows/sirene-sync.yml`) that:

1. builds the full server binary — `cargo build --release -p server --bin sirene_sync` (pulls in
   `domain` + `application` + `infrastructure`);
2. fetches active food-service établissements (NAF 56.10A/B/C + 56.30Z, Tours commune 37261) from the
   INSEE SIRENE API;
3. runs the SIRENE **ACL** (`etablissement_to_command`) then the **aggregate** command handler
   (`register_restaurant` → `RestaurantRegistered`, defaulting `NON_PARTNER`);
4. appends events **directly** to the production `domain_events` via the `DATABASE_URL` secret.

That implementation works (idempotent by `restaurantId = UUIDv5(SIRET)`), but has three structural
problems surfaced in review:

- **CI runs the domain.** It compiles the whole workspace to execute the ACL + aggregate — slow builds,
  and the version-sensitive logic runs in CI.
- **Version skew (the key one).** CI builds `main` HEAD; production — specifically the in-process
  projector that folds the emitted events — runs the *last-deployed* commit. The two sides communicate
  only through events. Additive changes are safe (events carry `#[serde(rename_all)]` + `#[serde(default)]`,
  **no** `#[serde(deny_unknown_fields)]`; `domain_events` stores the full payload; projections are
  rebuildable folds with a checkpoint → skew **self-heals** on the next deploy). But a **new event type**
  (`DomainEvent` is an adjacently-tagged enum with no `#[serde(other)]`) or a **breaking payload change**
  written by a CI ahead of production would make the deployed projector **stall** on those events until
  prod catches up.
- **Prod DB write credentials live in CI.**

## Decision
Split **ingestion** from **domain translation**, mirroring the app-layer projector pattern (ADR-0040).

1. **CI stays thin and "dumb".** The scheduled job only fetches from the INSEE SIRENE API and **UPSERTs
   raw records** into a new **raw-integration staging table `external_sirene_restaurants`** (raw INSEE
   payload as `jsonb`, keyed by SIRET, stamped with `last_seen_at` / `sync_run_id`). It runs **no ACL, no
   aggregate, no domain crates** → a small dependency graph and a fast build. On completion it **pings an
   internal endpoint** to wake/trigger the worker.
2. **On-app worker `sync_sirene_worker`** (runs on the deployed server, versioned exactly like the
   in-process projector). It reads `external_sirene_restaurants` past its **checkpoint**, runs the SIRENE
   **ACL** + calls the **actor** (`register_restaurant`), and advances the checkpoint. Idempotent as today.

**Effect:** all domain logic (ACL + aggregate) now runs **only on the deployed version** → writer of
events == reader == production, so the **version-skew hazard is eliminated by construction**. CI no longer
compiles or executes the domain. The batch scheduler stays free (GitHub Actions), and the heavy work stays
off the request path.

### Deletion policy
SIRENE never sends deletes: the active-only query means a closed établissement simply **disappears** from
results (detection by absence). The domain **already models this** — reuse the existing
`MarkRestaurantClosed` command → `RestaurantMarkedClosed` event (`commands.yaml`/`events.yaml`, documented
as "Sirene closure; issued by the sync ACL"). **No new DSL for deletion.**

- **Detect** in the worker by reconciling the staging mirror: rows whose `last_seen_at` predates the latest
  run have disappeared. **Prefer the explicit signal**: stop hard-filtering to active-only and read
  `etatAdministratifEtablissement` — an explicit `F` (*fermé*) is a confident closure; bare absence is a
  weaker signal.
- **Debounce**: require absence/closure across **N consecutive runs** (grace period) before acting, to
  absorb transient API/filter gaps.
- **Gate by partnership funnel** (`RestaurantListingStatus`):
  - **NON_PARTNER** prospect → auto-issue `MarkRestaurantClosed` (it is only a lead; low risk).
  - **PASSIVE/ACTIVE_PARTNER** (onboarded) → **never auto-close** on a registry signal; raise for **manual
    review**, protecting a live partner from a bad SIRENE datum or a SIRET change.
- **Never hard-delete** — closure is a new event; the projection folds it (RestaurantStatus/closed, dropped
  from active listings). Audit trail preserved, replayable.
- **SIRET change** (relocation → new SIRET → new `UUIDv5`) looks like "close + new prospect"; cross-SIRET
  dedup/merge is explicitly **out of scope for V0**.

## Alternatives considered
- **Keep the direct-write CI binary (status quo)** — simplest, but the version-skew, slow-build, and
  domain-in-CI problems above. Rejected as the steady state; kept as the interim until the worker lands.
- **CI calls production GraphQL to register** — one running logic instance, no rebuild, no prod-DB creds in
  CI; but needs a secured bulk `EXTERNAL` mutation and drives load onto the sleepy free-tier instance, and
  the SIRENE fetch + ACL code still has to live somewhere. Rejected for V0.
- **Move the whole sync into an in-app scheduled worker (no CI)** — cleanest conceptually and the eventual
  target, but needs a **paid** Render worker/cron; the free web instance sleeps. CI stays the free
  scheduler for now.
- **Treat SIRENE as inbound events (record facts, no command)** — rejected per CLAUDE.md: we *orchestrate*
  the SIRENE import and can *reject* records via the ACL, so it stays **command-driven** (like
  `ImportCatalog`), not an inbound integration event.

## Consequences
### Positive
- **Version skew eliminated** — the ACL + aggregate run only on the deployed version.
- **Faster CI** — the ingestion binary needs only an HTTP client + DB driver + the raw DTO, not the domain.
- **Architecturally consistent** — the worker mirrors the projector (checkpoint + drain).
- **Deletion for free** — reuses the already-modeled `MarkRestaurantClosed`/`RestaurantMarkedClosed`.
- **Natural retries/idempotency** via a per-row staging status.

### Negative / caveats
- A **new table + worker + CI split** to build (follow-up DSL + code).
- CI still writes to the DB (the **staging table** only, not `domain_events`) — mitigate with a
  **limited-privilege DB role** scoped to that table.
- The worker runs on the **free-tier** instance: it is woken by the CI ping and must finish before the
  instance sleeps (trivial at Tours scale, but a scaling consideration).
- A **second checkpoint** to operate, and an **internal ping endpoint** to secure.

## Follow-up actions
- Realize in the specs via the normal **plan-mode / validation** path (specs are not edited by execution
  loops): add `external_sirene_restaurants` as a **raw-integration staging** table under
  `specs/database/tables/` — a new table category, **not** projected from events and **not** referential.
- Wire `sync_sirene_worker` in `application`/`infrastructure` analogous to the projector; add the internal
  **ping** endpoint (EXTERNAL/internal auth).
- **Extract the raw SIRENE fetch + DTO** into a minimal crate so the CI binary drops its `domain`/
  `application`/`infrastructure` dependency.
- Implement the **deletion reconciliation** (`last_seen_at` + explicit `F` state + debounce + partner-status
  gate) on top of `MarkRestaurantClosed`; add behaviour tests + rule links for the register and close paths
  (ADR-0032 completeness).
- Consider the **limited-privilege DB role** for CI.
- **Retire** the interim direct-write path in `crates/server/src/bin/sirene_sync.rs` once the worker ships.

## References
Refines the delivery mechanism of **ADR-0019/0020/0027** (SIRENE pre-registration / sync cron / prospection
model). Mirrors **ADR-0040** (app-layer projector). Motivated by the version-skew analysis around
**ADR-0042/0043** (hosting + release strategy) and the **command vs inbound-event** rule in CLAUDE.md.
Existing DSL reused: `commands.yaml#/RegisterRestaurant`, `commands.yaml#/MarkRestaurantClosed`,
`commands.yaml#/ChangeRestaurantListingStatus`; `events.yaml#/RestaurantRegistered`,
`events.yaml#/RestaurantMarkedClosed`, `events.yaml#/RestaurantListingStatusChanged`;
`scalars.yaml#/RestaurantStatus`, `scalars.yaml#/RestaurantListingStatus`.
