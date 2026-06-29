# ADR-0013 — Structurizr DSL + Mermaid as generated C4 targets

## Status
Accepted (realizes the former backlog item P-11)

## Context
ADR-0008 makes C4 source-managed YAML (`specs/architecture/c4-l{2,3}.yaml`). To actually *see* the
architecture and export it, we need rendered views — without hand-drawing (drift) and without coupling
to one tool.

## Decision
Generate two complementary targets from the same C4 YAML + actor/view model (`tools/codegen/src/emit/c4.ts`):
- **Structurizr DSL** (`out/c4.generated.dsl`) — a workspace with SystemContext, Containers, and `api`
  component views (aggregates grouped by bounded context + the L3 technical components wired by the
  canonical CQRS/ES pipeline), with tag-based styling. Rendered/exported via Structurizr Lite or
  `structurizr-cli` (PNG/SVG/PlantUML/Mermaid). This is the proper auto-layout C4 target.
- **Mermaid** (`out/c4.generated.md`) — an L2 container diagram with real relationships and a domain
  diagram (bounded contexts → aggregates → the read models they feed). Renders on GitHub / VS Code /
  mermaid.live with no toolchain.

Both are GENERATED (never hand-edited). A `structurizr` Claude skill documents how to render/validate
and how to edit the source. The zero-dependency **interactive SVG map** in the HTML doc (§13) remains
the in-browser drill-down viewer; Structurizr/Mermaid are for auto-layout and export.

## Alternatives considered
- Structurizr only — needs Docker/CLI to see anything; no inline diagrams.
- Mermaid only — no real C4 semantics, weaker layout/export.
- Hand-drawn diagrams — drift, not derivable from the model.

## Consequences
### Positive
- One model → three views (interactive SVG, Mermaid, Structurizr); none can drift (all generated +
  validated for actor/view mapping).
### Negative
- Structurizr Lite/CLI must be installed to render the `.dsl` (Mermaid needs nothing).
### Follow-up actions
- Optionally emit per-bounded-context component views and dynamic (sequence) views for the sagas.
