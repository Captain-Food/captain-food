# ADR-0042 — Hosting: Render + Supabase, both in Frankfurt (EU)

## Status
Accepted

## Context
The full-stack Rust workspace (ADR-0034/0035) needs a runtime home for two things: the **Axum BFF /
Leptos SSR app** (`crates/server` + `crates/web`) and the **managed PostgreSQL** that backs the event
store and `View_*` read models (CLAUDE.md: "managed PostgreSQL (e.g. Supabase)"). Supabase is already the
identity provider (ADR-0015, phone-OTP / email magic-link wrapped behind our GraphQL).

Constraints/forces:
- **EU data residency (GDPR).** V0 targets **Tours, France**; customers, restaurants and riders are all in
  France, so both compute and data must stay in the EU.
- **Low app↔DB latency.** The read side folds `View_*` over `domain_events` on read (V0, ADR-0005/0039)
  and the write side appends events transactionally — chatty enough that the app and the database should
  sit in the **same region**.
- **Minimize ops for a pre-PMF team.** No appetite to run our own Kubernetes/Postgres in V0.
- Supabase bundles a lot we already lean on: managed Postgres, **Auth**, plus storage/realtime available
  later — reducing the number of vendors and glue.

## Decision
Host on **two managed platforms co-located in Frankfurt**:

- **Application → Render, Frankfurt region.** Render hosts the Axum + Leptos SSR service (and later any
  worker/cron for the restaurant-sync jobs, ADR-0020). PaaS: git-push deploys, zero-downtime rollouts,
  platform-managed TLS and HTTP health checks.
- **Database + Auth → Supabase, Frankfurt region** (AWS `eu-central-1`). Managed Postgres for
  `domain_events` + projections, and Supabase Auth per ADR-0015.

Both live in **Frankfurt**: geographically close (low cross-provider latency) while keeping **all compute
and data inside the EU**.

## Alternatives considered
- **Fly.io / Railway** — comparable Rust-friendly PaaS with EU (e.g. `cdg`/Amsterdam/Frankfurt) regions.
  Viable; Render chosen for its straightforward service model and health-check/deploy ergonomics. Not a
  one-way door — the app is a standard Axum binary, portable across any container PaaS.
- **Self-managed Kubernetes** (the P-04 probe contract's original assumption) — rejected for V0: too much
  ops for pre-PMF. Revisit only if scale/BAM/multi-service topology demands it.
- **Vercel / Netlify** — JS/edge-oriented; a poor fit for a long-running Rust Axum + WASM SSR server.
- **Scaleway / OVH (French clouds)** — strongest data-residency story and French-hosted, but more IaaS
  ops than a PaaS. Kept as a fallback if residency requirements tighten beyond "EU".
- **Colocate DB inside Render** (Render Postgres) — rejected: we specifically want Supabase for Auth +
  the bundled Postgres features, so the DB lives in Supabase, not Render.

## Consequences
### Positive
- **GDPR-friendly**: compute and data both in the EU (Frankfurt), close to the V0 user base.
- **Low app↔DB latency** from same-city co-location — good for the on-read projection folds.
- **Low ops**: two managed platforms, no cluster/DB to operate; one fewer vendor by reusing Supabase for
  Auth (ADR-0015) and later storage/realtime.
- **Portable**: the app is a plain Axum container, so Render is replaceable without domain/API changes.

### Negative
- **Cross-provider network hop** app↔DB (Render → Supabase over public TLS, not a single private VPC).
  Same-region keeps latency small, but plan for **connection pooling via Supabase Supavisor/pgBouncer**
  and enforce TLS; watch SQLx pool sizing against Supabase connection limits.
- **Reconcile with P-04 (K8s probe contract).** Render is PaaS, not raw Kubernetes: it does its own
  HTTP health checks and SIGTERM-based zero-downtime deploys, so we won't expose
  startup/liveness/readiness probes the K8s way. P-04's *intent* (health + graceful drain) still holds;
  its *mechanism* becomes "Render health-check endpoint + honour SIGTERM drain". To be settled when
  `crates/server` exists.
- **Two vendors to operate/bill/monitor** rather than one integrated platform.

### Operational notes
- **Deployment (current): Render service configured in the dashboard, deployed by the CI hook.** The Render
  **Blueprint** that formerly drove this from [`render.yaml`](../../render.yaml) was **retired 2026-07-21**
  (History below): once the service became image-backed + hook-deployed, the Blueprint kept failing to sync
  and its `image.url` management fought the digest-pinned deploys. `render.yaml` is now a documentation-only
  record of the intended config (not applied); the `captain-food` service is configured directly in the
  Render dashboard.
  - Retired Blueprint ID (reference): `exs-d9d8q058nd3s73dosclg` · repo `Captain-Food/captain-food` · branch `main` · path `render.yaml`.
  - Service config (build model **per ADR-20260721-175411** — see below): originally
    `runtime: docker` + `dockerfilePath: ./Dockerfile` (cargo-chef cached build **on Render**) with
    `autoDeployTrigger: checksPass`. Render metering build-pipeline minutes at a $0 cap made building Rust
    on Render unsustainable, so the build moved to CI: the image is now compiled in GitHub Actions and
    pushed to GHCR, and the service is `runtime: image` + `autoDeploy: false`, pulling the pre-built image
    (deploys triggered by a Render deploy hook pinning `sha-<commit>`). ADR-0043 still keeps migrations
    out-of-band and the `/health` schema-version gate still holds a deploy that races ahead of a migration.
  - Deploys are driven **only** by the `build-image` workflow's Render **deploy hook**, which pins the exact
    image **by digest** (ADR-20260721-175411); a git push does not deploy (`autoDeploy: false`). The
    deployed-version source of truth is Render's deploy/event history + the app's `X-VERSION` response header
    and `/health` `version`. Secrets stay dashboard-managed (`sync: false`) and are never committed.
  - History: linked 2026-07-17; first sync (commit `5a9e2f5`) moved the service off a manually-configured
    native `cargo build` onto the cargo-chef **Docker** runtime built **on Render** (verified live
    2026-07-17, `/health` → `db:up`). On 2026-07-21 (**ADR-20260721-175411**) the build moved off Render
    entirely to **GitHub Actions + GHCR** and the service became `runtime: image` — Render now only pulls a
    pre-built image, spending zero build-pipeline minutes. The Blueprint could no longer reconcile the
    manually-switched image source (dashboard status "Failed sync") and was **deleted 2026-07-21**; the
    service now runs on dashboard config + the CI deploy hook, verified live end-to-end at commit `503a1a7`.
- **Build tuning.** The workspace `[profile.release]` (root `Cargo.toml`) sets `lto = "thin"`,
  `codegen-units = 1`, `strip = true` for the deployed binary — runtime-perf tuning, independent of where
  the build runs; `panic = "abort"` deliberately NOT set (keeps per-request panic isolation), `target-cpu`
  left generic (the build host — now **GitHub Actions** — and the Render run host are different machines, so
  a `native`-tuned binary could target the wrong CPU). The image is built in **CI** from the cargo-chef
  Dockerfile with a buildx `type=gha` layer cache (ADR-20260721-175411), so the slow optimized compile is
  cached across runs. **Open optimization**: the Dockerfile's `cargo chef cook` is not scoped to `-p server`,
  so it cooks the whole workspace (incl. `web`/`desktop`/`codegen`); scoping it to `-p server` would shrink
  the cached layer and speed cold builds (no behaviour change).
- **DNS & custom domains (Dynadot → Render).** The service is reachable **only via custom domains** — the
  `onrender.com` URL is disabled. `*.captain.food` is a Render custom domain with an **issued wildcard TLS
  cert** (Let's Encrypt DNS-01 via `_acme-challenge.captain.food` CNAME → `<service>.verify.renderdns.com`;
  on Dynadot, so no Cloudflare `_cf-custom-hostname`). DNS: `*` CNAME → `captain-food.onrender.com` (covers
  `api`/`live`/`restos`/`riders`/`system` + every `{slug}`); apex + `www` 301 → `join` (marketing on GitHub
  Pages, off-Render); explicit `join`/`www` records override the wildcard. Host-based routing:
  `crates/server/src/hosts.rs`. Full topology + the realized-DNS amendment: **ADR-0036 (2026-07-18)**.
- **Supabase Data API (PostgREST) is intentionally DISABLED** — all access is via the BFF + direct sqlx
  (ADR-0006), so PostgREST is unused and its REST surface is not exposed. Known, benign side effect: with
  the Data API off, PostgREST still runs and logs `schema "pg_pgrst_no_exposed_schemas" does not exist`
  (SQLSTATE `3F000`) on its ~30s schema-cache reload, which tanks the dashboard "success rate" metric.
  This is **expected noise, not an app fault** — ignore it. (To silence it, one would expose an empty
  schema to PostgREST; not worth re-adding a REST surface.) Verified via Supabase Postgres logs 2026-07-17.

### Follow-up actions
- ✅ Done — `crates/server` exposes `/health` (readiness gate) + `/ping` (liveness) and drains on SIGTERM
  (`shutdown_signal` in `main.rs`); Render's Health Check Path is `/health`. Still open: reconcile **P-04**
  in `docs/adr/README.md` with the realized PaaS mechanism (Render health check + SIGTERM drain).
- ✅ Superseded build/deploy mechanism — see **ADR-20260721-175411** (CI-built image on GHCR, Render pulls
  by digest). ADR-0042's hosting decision (Render + Supabase, Frankfurt) is unchanged.
- Provision Supabase in **`eu-central-1` (Frankfurt)** and the Render service in **Frankfurt**; verify
  region parity at setup.
- Configure **Supavisor** pooling + TLS for the Render→Supabase connection; size the SQLx pool to
  Supabase limits.
- Reflect the deployment nodes in the C4 model (`specs/architecture/*.yaml`) if/when a deployment view is
  added.
