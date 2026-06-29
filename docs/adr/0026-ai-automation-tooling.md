# ADR-0026 — AI automation tooling: Perplexity Computer + Claude Code

## Status
Proposed

## Context
Captain.Food runs with a solo CTPO. Delegating repetitive work to autonomous AI agents is a strategic
necessity, not optional. (Source: ADR-NEW-006.)

## Decision
Two tools, two complementary roles:

| Tool | Role | Captain.Food use |
|---|---|---|
| **Perplexity Computer** | multi-agent orchestration, research, document workflows | competitive watch, social content, market studies, SEO landing pages, financial reporting |
| **Claude Code** | autonomous coding agent in the terminal | cron jobs, Stripe scripts, auto-tests, prospection pipeline, social publishing via API |

A Perplexity Computer skill `CAPTAIN.FOOD – CORE CONTEXT` (business, economic model, stack, brand/tone,
production constraints) is activated first for all Captain.Food Computer tasks. **Human validation
(Johnny) is mandatory before publishing any external/social content.**

## Alternatives considered
- One tool for everything — overlap and confusion over responsibilities.
- No automation — doesn't scale with a solo team.

## Consequences
### Positive
- More capacity without hiring; consistent outputs (shared context); less time on low-value tasks.
### Negative
- Third-party dependency (Perplexity Pro/Max, Claude Code); output quality depends on the skill/prompts
  (maintenance); no automation replaces strategy or the human restaurant relationship.
### Follow-up actions
- Claude Code automation in this repo runs under the weekly time budget (ADR-0014); keep the Core-Context
  skill current.
