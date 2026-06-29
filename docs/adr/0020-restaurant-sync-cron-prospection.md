# ADR-0020 — Automated restaurant sync cron + B2B prospection scoring

## Status
Proposed

## Context
Hundreds of listings cannot be kept current by hand, and B2B prospecting must be automated to scale with
a tiny team. (Source: ADR-NEW-005; builds on ADR-0019.)

## Decision
A scheduled pipeline (GitHub Actions cron + backend worker):
- **Weekly (Mon 03:00) — Sirene sync:** new establishments → INSERT `non_partenaire`; closures → mark
  `fermé`; address/name changes → UPDATE; log to `sync_logs`; **immediate Slack alert** if an *active
  partner* turns `fermé` in Sirene.
- **Monthly (1st, 04:00) — Google enrichment:** refresh rating/reviews/hours for `non_partenaire` only
  (active partners come from HubRise).
- **Prospection scoring (0–10) on each INSERT:** food-truck NAF 56.10C +3, Google rating ≥4.0 +2,
  reviews <20 +2, created <12 mo +2, no website +1, already on Uber/Deliveroo −2, national franchise −3.
  If **score ≥ 5**: create HubSpot lead, send J+0 email ("your restaurant has a Captain.Food page"),
  J+7 relance, mark `froid` at J+21. **Anti-spam: ≤ 3 contacts, ≥ 7 days apart.** Idempotent (no dupes on
  re-run). Env: `SIRENE_API_KEY`, `GOOGLE_MAPS_API_KEY`, `HUBSPOT_API_KEY`, `RESEND_API_KEY`,
  `SLACK_WEBHOOK_OPS`, `SLACK_WEBHOOK_PROSPECTION`, `DATABASE_URL`.

## Alternatives considered
- Manual maintenance — doesn't scale.
- Real-time webhooks from Sirene — not offered; Sirene is polled (updated ~daily).

## Consequences
### Positive
- Zero-touch first-touch prospecting; always-fresh listings; scales 50→5000 restaurants.
### Negative
- Depends on Sirene/Google availability (needs retry/graceful degradation); auto-emails need careful copy;
  human follow-up still required once a prospect replies.
### Follow-up actions
- Model the sync as a process-manager/integration with inbound import events + an observability contract;
  reuse the repo's weekly-budget guard (ADR-0014) for any Claude-driven loop parts.
