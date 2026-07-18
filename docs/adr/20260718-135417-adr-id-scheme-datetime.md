# ADR-20260718-135417 — ADR identifiers are date-time based (concurrency-safe)

## Status
Accepted. First ADR to use the new scheme (it dogfoods the decision).

## Context
ADRs were numbered with a zero-padded, monotonic counter (`0001`, `0002`, … `0047`). That counter is a
**shared mutable value**: to add an ADR you take "the next number". When **two Claude Code sessions run
concurrently** on the same repo — which we do — both independently pick the same next number and collide.
This happened on 2026-07-17: two sessions each authored an **ADR-0046** (`…-graphql-write-side-…` and an
auth ADR), producing a filename/index/merge conflict that had to be hand-resolved (the auth ADR was
renumbered to 0047). The number carries no information the title/date don't, yet forces coordination.

## Decision
**New ADRs use a UTC date-time identifier** — no shared counter, so parallel sessions never collide:

- **File:** `docs/adr/YYYYMMDD-HHMMSS-kebab-title.md` (e.g. `20260718-135417-adr-id-scheme-datetime.md`).
- **ID in the title & cross-references:** `ADR-YYYYMMDD-HHMMSS` (e.g. `ADR-20260718-135417`).
- **Timestamp** = UTC at authoring time (`date -u +%Y%m%d-%H%M%S`). Chronologically sortable by filename.
- **Legacy ADRs `0001`–`0047` keep their sequential ids** — they are not renamed (they cannot collide, and
  are cross-referenced from hundreds of code comments, CLAUDE.md, and each other). Both id forms coexist;
  a cross-reference uses whatever id the target ADR carries.
- **Same-second tie-break** (rare): if two ADRs share a timestamp, append a short suffix — `…-HHMMSS-b`.

The `docs/adr/README.md` index is still appended per ADR; that is now only a **textual** merge (adjacent
lines), trivially resolvable — the hard *numbering* collision is gone.

## Alternatives considered
- **Keep the sequential counter** — rejected: it is exactly the shared-counter collision we hit.
- **Rename all existing ADRs to date-time** — rejected: breaks hundreds of `ADR-00XX` references across
  code/docs and gives **no** concurrency benefit (existing ADRs can't collide). Pure churn and risk.
- **Random/UUID ids** — rejected: not human-readable and not chronologically sortable.
- **Date-only (`YYYYMMDD`)** — rejected: two ADRs on the same day still collide.

## Consequences
### Positive
- **Concurrency-safe:** independent sessions mint non-colliding ids with no coordination.
- Chronologically **sortable** by filename; the id encodes *when* the decision was taken.
### Negative
- Ids are **longer** and less pretty than `ADR-0006`.
- A **mixed scheme** during the transition (sequential legacy + date-time new) — documented here and in
  `docs/claude/adr.md`.
- The README index append can still **textually** conflict across sessions (trivial to merge; not a
  numbering collision).

### Follow-up actions
- Update `docs/claude/adr.md` (naming) and `docs/adr/_template.md` (title placeholder) to the new scheme. *(done in this change)*
- Add a scheme note to `docs/adr/README.md`. *(done)*

## References
Supersedes the "zero-padded, monotonic" naming rule in `docs/claude/adr.md`. Prompted by the 0046
double-allocation on 2026-07-17.
