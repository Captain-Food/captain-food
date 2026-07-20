# ADR-20260720-213024 — The backlog is worked in the project's value-ranked order

## Status

Accepted (product-owner directive, 2026-07-20; this file materializes the id already referenced by
issues #12–#28's "Value rank" headers)

## Context

ADR-20260720-143000 ordered the queue "simplest → largest". The product owner re-prioritized on the
org GitHub Project: ordering is by **value (foundations → value stream)**, stamped in each issue
body as `Value rank N/15` alongside Priority/Effort org fields. Sessions must follow THAT order —
not recompute their own.

## Decision

- **The prioritized backlog is authoritative**: work issues in ascending `Value rank` (ties/edits:
  the Priority then Effort org fields; the issue body header is the durable copy of the rank).
- Order at the time of recording: #14, #22, #15, #16, #19, #18, #27, #26, #24, #25, #23, #17, #21,
  #20, #28.
- Re-ranking is a product-owner edit to the issues/board; agents never reorder on their own —
  amending this ADR is not required for rank changes (the issues are the live source).
- Everything else in ADR-20260720-143000 (sizing labels, pre-task sections, issue=contract /
  PR=record) is unchanged.

## Consequences

- Any session (including a fresh one) derives "what's next" from the open issues' rank headers —
  no session memory required.
- The simplest→largest rank in ADR-20260720-143000 is superseded as the WORK ORDER; it remains the
  sizing methodology.
