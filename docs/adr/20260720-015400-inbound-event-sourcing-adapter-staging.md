# ADR-20260720-015400 — Inbound event sourcing: adapter-owned `external_*` staging + the `inbound_events` handoff

## Status

Accepted — landed with ADR-20260720-015300/-015500. Amends ADR-20260718-145856 (the webhook path
gains a durable inbox; ACK-then-drain replaces inline append) and generalizes ADR-0045 (the SIRENE
staging + drain-worker pattern becomes the standard adapter shape).

## Context

Stripe/HubRise webhooks were verified and translated **in-process**, appending straight to
`domain_events` (ADR-20260718-145856). External systems deliver duplicates, delays and reorders;
without a durable record of what was received we lose the exact inbound payload, its verification
outcome, and whether it was already handled — ADR-145856 explicitly left "replay/idempotency
storage" open. Meanwhile SIRENE already proved the durable-first shape: a raw staging table plus an
on-app drain worker (ADR-0045).

Boundary rule agreed with the product owner: adapters (SIRENE/Stripe/HubRise) live **outside** the
core domain and may own tables for the exclusive needs of adaptation, but what crosses into the
domain must already speak the domain's language.

## Decision

Two layers, with a hard vocabulary boundary between them:

1. **Adapter-owned raw staging — `external_*` tables** (`staging: true`, in
   `specs/database/tables/integration_staging.yaml`): `external_stripe_events` (pk the Stripe
   `evt_…` id) and `external_hubrise_callbacks` (pk the callback id, else a UUIDv5 of the raw
   body), alongside the existing `external_sirene_restaurants`. The webhook endpoint verifies the
   partner signature (unchanged, fail-closed, raw-body HMAC), **UPSERTs the raw payload** —
   redelivery dedupes on the pk and ACKs immediately — then translates. `processed_at` is the
   translation high-water mark, enabling replay/backfill without asking the provider to resend.
   These tables are adapter-private: never projected, never a GraphQL `reads` target.

2. **The domain handoff — table `inbound_events`** (`specs/database/tables/journals.yaml`):
   contains ONLY **adapted business-domain events** (events.yaml vocabulary — `PaymentCaptured`,
   `PaymentFailed`, `PaymentRefunded`…), never external shapes. One row per fact: pk
   `inbound_event_id` (UUIDv7), `source` + `external_id` (composite-unique — the delivery-level
   dedupe), `correlation_id` (UUIDv5 of the provider event id — the existing ACL convention),
   `event_type`, the serialized domain event as `payload`, lifecycle
   `RECEIVED → DELIVERED | FAILED` (`InboundEventStatus`), `received_at`/`delivered_at`.

3. **Drain, not direct append**: an `InboundEventsDrainWorker` (modeled on the SIRENE worker —
   single-flight, keyset pagination, per-row processing, summary) delivers each `RECEIVED` row
   through the **normal write path** (`application::payments::record_inbound_payment_event` →
   `Repository` → `domain_events`) with an EXTERNAL `Actor` whose `cause_id = inbound_event_id`.
   The aggregate's own fold-based dedupe stays **authoritative** (an `AlreadyRecorded` outcome
   still marks the row `DELIVERED`); the journal's `(source, external_id)` unique is the cheap
   first line. Journals never write `domain_events` directly.

4. **HubRise nuance — the request/report split holds**: HubRise callbacks carry no state and drive
   an OAuth pull that yields **commands** (`ImportCatalog`, stock updates). Commands are not
   inbound events, so the HubRise leg = raw `external_hubrise_callbacks` dedupe + the existing
   enricher path (its command sends journal into `command_journal` with `channel: WORKER`,
   ADR-20260720-015300). `inbound_events` stays events-only.

## Alternatives considered

- **Keep inline append (status quo)** — rejected: no durable receipt, no replay, dedupe invisible
  outside the aggregate fold, incident analysis starts from nothing.
- **One journal for raw AND adapted payloads** — rejected: it would either leak external vocabulary
  into a domain-facing table or force adapters to give up their raw record; the two layers have
  different owners, retention and shapes.
- **Migrate SIRENE onto `inbound_events`** — rejected (product decision): SIRENE is a bulk UPSERT
  mirror sync, not an event feed; its purpose-built staging table stays.

## Consequences

### Positive
- External retries are safe, inspectable, and replayable from our own storage.
- Incident analysis starts from the exact raw payload AND the exact adapted fact.
- The ADR-145856 open point (replay/idempotency storage) is closed; the ACL boundary is now also a
  durability boundary.
- Webhook endpoints get faster and dumber: verify → persist → ACK; interpretation is async.

### Negative
- Two more tables per push-partner and a worker to operate; payload retention needs governance.
- A small delivery lag (drain tick) between webhook ACK and the domain fact — acceptable, the
  Payment PM already tolerates asynchronous Stripe outcomes.

### Follow-up actions
- Nudge-on-ingest (fire `run_once` after staging) keeps the lag near zero; keep the poll as backstop.
- Delivery-partner (Avelo37, post-V0) adopts the same two-layer shape on arrival.
