# ADR-20260720-213024 — Value-first issue ordering: foundations first, then features by value stream

## Status

Accepted — amends ADR-20260720-143000 §4 (prioritisation only; sizing, pre-task documentation and
the PR-as-post-task-record rules are unchanged).

## Context

ADR-20260720-143000 ordered the backlog "cheapest-first among the impactful" and stamped a
`Rank N/17 (simplest → largest)` in every issue header. Working the queue showed the flaw: the open
issues are not very small (mostly S–XXL), so an effort-first ordering front-loads *small* work
rather than *valuable* work, and the ordering criterion stops discriminating once everything is
multi-session. The same ADR reserved the fix: the queue is "re-ordered only when new information
arrives" — the product owner has now directed that ordering be by **value, not effort**:

> the value is first the foundations / cross-functional or non-functional, then the features in
> order of value stream.

## Decision

### 1. Two value tiers, replacing simplest-first

- **Tier 1 — foundations & cross-functional/non-functional.** Work every later change stands on:
  API/write contracts, security (ACL), correctness invariants, observability, data
  retention/compliance, and the codegen operating-model wave (which cheapens everything after it).
  Value here = risk retired × what it unblocks.
- **Tier 2 — features, in value-stream order.** The V0 value stream runs
  **customer ordering** (the PMF funnel — nothing else matters until a Tours customer can order)
  → **restaurant onboarding** (supply side, self-serve HubRise connect)
  → **delivery automation** (post-V0; manual/out-of-band in V0).

Within a tier, ties break dependency-consistently (an issue never ranks above one it needs).
Effort/size labels remain — they answer "what will this cost?", no longer "what comes first?".

### 2. The ordering (15 open issues, 2026-07-20)

| Value rank | Issue | Size | Why it sits here |
|---|---|---|---|
| 1 | #14 orderStatusChanged convention | S | Last unstable API contract; every client line written against `correlationId` is paid-twice rework — must precede #17/#21. |
| 2 | #22 per-field ACL on nav edges | M | Structural data-exposure hole that widens with every FK; must precede #20's token tables and #21's client edge assumptions. |
| 3 | #15 journal WORKER command sends | S | Restores the "ALL command submissions converge on `command_journal`" invariant (idempotency, traceability); #16's channel metrics are meaningless without it. |
| 4 | #16 command-acceptance observability | M | The most critical workflow currently violates the "every critical workflow has an observability contract" non-negotiable; supplies #19's decision data. |
| 5 | #19 checkout latency watch | XXS | Standing guard on the one funnel V0 lives or dies on; keeps the pre-made sync-exception decision from being re-litigated. |
| 6 | #18 retention policy | S | Unbounded journal/mirror growth + raw-payload PII (GDPR) — pure risk retirement, cheapest while tables are small. |
| 7 | #27 PM state-store emitters | M | Opens the codegen wave: proven conventions, first "spec-only" building block. |
| 8 | #26 service-catalog emitters | L | Makes `services.yaml` executable; every hand-written port before it is migration debt. |
| 9 | #24 generated behaviour-test harness | L | The spec becomes the test suite; the parity oracle #25/#23 prove themselves against. |
| 10 | #25 generated PM orchestrators | L | Spec-only sagas — the biggest per-feature Rust reduction; directly cheapens #20/#28. |
| 11 | #23 lifecycle completion | L | Ends the split-brain between machine-checked and hand-written transitions; step 4 rides on #24. |
| 12 | #17 two-step client write model | L | First slice of the customer stream: the correctness convention (async rejections) the renderer must be born with, not retrofitted. |
| 13 | #21 Leptos/WASM SDUI renderer | XXL | The customer value stream itself — the PMF clock does not start until it ships. |
| 14 | #20 HubRise connect flow | L | Restaurant onboarding — turns onboarding from an env-edit deployment event into self-serve. |
| 15 | #28 Avelo37 delivery adapter | XL | Delivery automation, explicitly post-V0; waits for the codegen wave by design. |

The codegen wave (7–11) outranks the renderer deliberately, per the product-owner directive:
it is operating-model foundation, and its internal order (#27 → #26 → #24 → #25 → #23) follows the
dependency chain the issues themselves state.

### 3. Mechanics

- Each open issue's header line now carries `**Value rank N/15** (foundations → value stream,
  ADR-20260720-213024)` in place of the old `Rank N/17 (simplest → largest)` fragment.
- New issues get a value rank at triage using the two-tier test above; re-rank only when new
  information arrives (unchanged rule).
- The denominator is the count of open ranked issues at stamping time; it is not rewritten across
  all issues on every open/close — the order, not the denominator, is the contract.

## Alternatives considered

- **Keep simplest-first** — rejected by the product owner: with no small issues left, it optimizes
  time-to-*a*-merge, not time-to-value.
- **WSJF / cost-of-delay scoring** — rejected: numeric scoring theatre for a 15-item queue; the
  two-tier rule reproduces the intent with zero ceremony (consistent with 143000's "no Scrum").
- **Pure feature-first (renderer immediately)** — rejected: builds the biggest client on unstable
  contracts (#14, #17) and skips the codegen wave, hand-writing exactly what #24–#27 would then
  migrate — paying twice.

## Consequences

### Positive
- The queue now answers "what is most valuable next?", which is the question actually asked.
- Contract/security fixes land before any client hardcodes the wrong shape.
- Feature work arrives on generated rails (harness, ports, orchestrators) instead of hand copies.

### Negative
- The most visible artifact (the customer app) starts later than a feature-first order would allow;
  accepted — it starts once, on stable contracts, instead of starting twice.
- Rank denominators across issues drift from `/17` to `/15`-era stamps as issues close; accepted
  (see §3).

### Follow-up actions
- All 15 open issues re-stamped with the value rank (this change).
- ADR-20260720-143000 status annotated as amended by this ADR.
