# 📋 Backlog method — how work is picked, claimed and released

> METHOD lives here; STATE lives on the [GitHub issues](https://github.com/Captain-Food/captain-food/issues)
> and the org Project board — never in this file (ADR-20260720-213024, ADR-20260720-233000).

## Order

Work open issues in ascending **`Value rank N/…`** (each issue's body header; foundations → value
stream). Only the product owner re-ranks (board/issue edit). Sizing method: ADR-20260720-143000.

## Claim protocol (multi-session safety)

1. **Before any work**: add the **`status/in-progress`** label to the issue AND post a claim
   comment naming the branch (`NN-slug`) you are opening. The label is the atomic, API-visible
   claim; the comment covers the window before the PR exists.
2. **Never work an issue that carries `status/in-progress`** — pick the next unclaimed rank.
3. Branch names are **`NN-slug`** (issue number first); the PR body carries **`Closes #NN`** —
   from then on GitHub's Development sidebar shows everyone the branch + PR for the issue.
4. Merge (or close) ends the claim naturally (the issue closes). Abandoning? Remove the label.
5. **Board mirror (one-way, label → Status)**: the org Project has a Workflow "label
   `status/in-progress` added → Status: In Progress" (+ built-in "item closed → Done"), so the
   board SHOWS the claim in its In Progress column. Sessions never write the Status column
   directly — single writer, the label; the board is a view (needs org-project scopes agents'
   tokens don't have, which is exactly why the label is the claim).

## Stale-claim reaper

`.github/workflows/stale-claim-reaper.yml` (hourly): a `status/in-progress` issue with **>24h**
of no activity (issue comments, linked-PR references — the reaper ignores its own comments)
loses the label and gets a "claim expired" comment → back to the queue. A crashed session can
never hold an issue hostage.

## Issue anatomy

Pre-task sections (Why now / What & why / Impact / Sequence diagram / Estimation), `size/*`
label, Priority/Effort fields, value-rank header — see ADR-20260720-143000. Issue = pre-task
contract; PR = post-task record; divergence between them is reviewable signal.
