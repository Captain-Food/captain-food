# ADR-20260722-225945 — The Captain Company umbrella: GitHub org rename + multi-product brand architecture

## Status

Accepted (product-owner directive, 2026-07-22). Pending the manual GitHub-side org rename (see
Sequencing). Establishes the parent-company layer above **Captain.Food** and every future
`Captain.X` product. Touches ADR-0036 (domain topology / single-origin identity) and
ADR-0015 (Supabase Auth wrapper) as the seams that make later shared identity cheap.

## Context

Captain.Food is the first product, but the intent is a **family** of products sharing the same
spec-driven framework (the YAML DSL → `make validate` → codegen operating model), e.g. **Captain.Jobs**,
**Captain.Voyage**. Today the GitHub **org** is named after the *product* (`Captain-Food`), which
conflates "the company that owns everything" with "the food product" — there is no room for a second
product to live as a peer.

The founder has purchased the corporate domain **`thecaptaincompany.com`**. This ADR records the
brand/structure decision so concurrent sessions do not diverge on naming, and separates the **cheap,
do-now** structural moves from the **expensive, defer** platform build (avoiding premature
platformization while PMF in Tours is still unvalidated).

## Decision

1. **The company (brand) is "Captain"; the legal/corporate entity is "The Captain Company".** Products
   keep the `Captain.X` pattern (`Captain.Food`, `Captain.Jobs`, `Captain.Voyage`). The corporate home is
   `thecaptaincompany.com`; customer-facing products keep their own domains (`captain.food`, …).

2. **Rename the GitHub org `Captain-Food` → `TheCaptainCompany`.** The org becomes the *company*; each
   product is a repo (or repo set) inside it. The `"The…"` prefix keeps the parent visually distinct from
   the `Captain-X`-shaped product repos, and the login exactly matches the corporate domain.

3. **The repo, crates and product domain are NOT renamed.** `captain-food` (repo), `captain-food-*`
   (crates), and `captain.food` (the multi-tenant product domain + `*.captain.food` wildcard) are the
   *product* identity and stay. Only the **owner segment** of URLs changes (`Captain-Food/` →
   `TheCaptainCompany/`).

4. **Company-level subdomain map (reserved, not built).** Under `thecaptaincompany.com`:
   `id.` = **Captain ID** (shared identity, product-neutral host), `studio.` = the spec-admin / "Captain
   Studio" internal tool. These are DNS reservations + intent, not services to stand up now.

5. **Shared identity ("Captain ID") is reserved, not built.** We keep the existing GraphQL wrapper over
   Supabase Auth (ADR-0015) as the swap seam. When identity is wired, the Supabase Auth project is scoped
   as a standalone **Captain ID** project (shared across products) rather than a food-specific one — a
   near-zero-cost naming decision taken now for large future payoff. No separate auth service/SSO gateway
   is built until product #2.

6. **The framework is NOT extracted yet.** `tools/codegen-rs` and the operating model stay in this repo.
   Extraction into a shared `captain-framework` repo happens when the **second consumer** (Captain.Jobs)
   actually exists — extracting before a real second consumer would harden the wrong abstractions.

## Consequences

- **GHCR image path moves** (registry namespace follows the org, lowercased):
  `ghcr.io/captain-food/captain-food` → `ghcr.io/thecaptaincompany/captain-food`. This is the only change
  that can break production: after the rename CI publishes to the new path while Render still pulls the
  old one, so Render's image URL must be repointed and the new GHCR package re-verified **public** in the
  same window. `.github/workflows/build-image.yml`, `render.yaml`, `Dockerfile` and the README rollback
  runbook are updated in this change.
- **GitHub redirects** cover web URLs, `git` remotes, API, org secrets, deploy hooks and authorized OAuth
  apps — so nothing breaks instantly — but the freed `Captain-Food` login is not permanently reserved, so
  in-repo references are updated deliberately rather than left to the redirect.
- **In-repo doc/config references** to the org path are updated (README badges, `SECURITY.md`,
  `CODE_OF_CONDUCT.md`, issue-template config, `docs/BACKLOG.md`, `docs/STATUS.md`). **Historical ADRs**
  (ADR-0042, ADR-20260721-175411) are left verbatim as immutable record; the `LicenseRef-Captain-Food-Coopyleft`
  SPDX identifier (ADR-0044) is a stable id and is **not** renamed.
- **Manual, out-of-band steps** (no tooling here): the org rename itself, the Render dashboard image-URL
  repoint + GHCR visibility check, updating local clone remotes, and deciding the fate of the
  `captain-food.github.io` Pages repo (its special `<login>.github.io` behaviour no longer matches after
  the rename).
- The **spec-admin "Captain Studio"** app (the request that started this thread) is reframed as the first
  *company-level* tool — it operates on the shared framework, not a single product's domain — and is the
  natural first tenant of company-level identity.

## Sequencing

1. Merge-ready PR with the in-repo reference updates (this change) is staged but **held**.
2. Product owner performs the GitHub org rename `Captain-Food` → `TheCaptainCompany`.
3. Immediately repoint Render's image URL to `ghcr.io/thecaptaincompany/captain-food` and confirm the new
   GHCR package is public.
4. Merge this PR so CI publishes to the new path and Render pulls it; verify one green deploy.
5. Update local remotes; resolve the Pages repo.

Doing it in this order means production is only ever pointed at one image path at a time.
