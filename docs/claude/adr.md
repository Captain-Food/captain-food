# Claude rules — ADRs (`docs/adr/`)

Architecture Decision Records capture significant decisions and their rationale.

## Naming & status

- File: `docs/adr/NNNN-kebab-title.md` (zero-padded, monotonic). Index: `docs/adr/README.md`.
- Status: `Proposed` → `Accepted` → `Superseded by ADR-XXXX` (or `Deprecated`). Never delete an ADR;
  supersede it.
- An ADR is **Accepted** only when the decision is realized in the repo (or explicitly ratified).
  Decisions about a runtime that does not exist yet stay **Proposed**.

## Template

Use `docs/adr/_template.md`. Sections: Status · Context · Decision · Alternatives considered ·
Consequences (Positive / Negative / Follow-up).

## Workflow

- Propose ADRs in **plan mode** (drafts may live under `docs/adr/` as `Proposed`).
- A new or changed cross-cutting decision must land as an ADR in the same change.
- **Every recurring agent/loop failure becomes a rule, a test, or an ADR** (non-negotiable). If a class
  of mistake repeats, write the ADR that prevents it and reference it from `CLAUDE.md` or a topic file.
- Keep ADRs short and decision-focused; link to the DSL/codegen artifacts that implement them.
