# ADR-0011 — Agent topology: generator / reviewer / observability (separation of duties)

## Status
Accepted

## Context
One agent that generates, reviews its own work, and acts on infrastructure has no independent check and
tends to rationalize its own output.

## Decision
Three specialized agents with explicit read/write boundaries (`.claude/agents/*.md`):
- **Generator** — reads approved DSL + C4, writes only `tools/codegen/out` and ADR drafts; never writes
  `specs/**`.
- **Reviewer** — reads the whole repo, emits pass/fail reports with file-level evidence; never rewrites
  sources.
- **Observability agent** — reads traces/logs/metrics/BAM + loop state, writes incident/diagnostic
  reports; never acts on infrastructure without policy approval.

## Alternatives considered
- Single all-purpose agent — no independent review; conflicts of interest.

## Consequences
### Positive
- Review is independent of generation; permissions are least-privilege per role.
### Negative
- More configuration to maintain.
### Follow-up actions
- Wire the agents into the loop stages (Execute → Review → Observe).
