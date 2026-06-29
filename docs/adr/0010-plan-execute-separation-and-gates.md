# ADR-0010 — Plan/execute separation + validation-gated loop (Stop hook)

## Status
Accepted

## Context
An autonomous loop that can both redefine the model and fix code will, under pressure to "make it pass,"
silently rewrite the business contract — a drift-amplifying machine. Loop completion must not be
declarable until objective gates pass.

## Decision
Separate **plan mode** (read-only: understand, analyze impact, classify breaking vs non-breaking,
propose, await approval — may propose DSL changes) from **execution mode** (apply approved changes,
generate, validate, fix implementation/generator defects — DSL is frozen input). The canonical loop is
`Spec → Plan → Execute → Review → Validate → Publish → Observe → Learn`. A **Stop hook**
(`.claude/hooks/stop-gate.sh`, wired in `.claude/settings.json`) blocks completion unless the acceptance
gates pass (typecheck + `npm run validate`, which covers schema/behaviour/observability/C4); app-level
gates are skipped gracefully until they exist. A **PostToolUse hook** re-validates after `specs/**`
writes and forbids hand-edits to generated output.

## Alternatives considered
- Trust the loop to self-declare success — premature/false "done".
- CI-only gates — too late; the loop should not finish red.

## Consequences
### Positive
- Loops cannot finish on a broken model; the DSL cannot be edited by execution loops.
### Negative
- Hooks add per-turn validation cost; gates must stay fast and green.
### Follow-up actions
- Add a `loop-state` operational-memory file for night loops (P-13).
