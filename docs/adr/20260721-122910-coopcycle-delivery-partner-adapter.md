# ADR-20260721-122910 — CoopCycle delivery-partner adapter: a federated (per-instance) PARTNER path

## Status

Accepted — implements issue #58. The **third** `DeliveryProvider = PARTNER` adapter, applying the
Avelo37 pattern (ADR-20260721-104233): the outbound `DeliveryService` half (ADR-20260719-214500 /
issue #26), the two-layer webhook inbox (ADR-20260720-015400), the partner-adapter-crate shape
(ADR-20260718-213352), and the bounded re-offer policy (ADR-20260720-004556). Sibling of #57 (Uber
Direct); both await the multi-partner ranking foundation for true selection *among* partners.

## Context

Avelo37 (#28, ADR-20260721-104233) established a reusable delivery-partner shape: a self-contained
adapter crate, a verified-webhook two-layer inbox (`external_<partner>_events` → `inbound_events` →
drain onto the `DeliveryJob` stream, routed by event type — already partner-generic), and a
fail-closed, env-gated outbound gateway behind the generated `DeliveryService` port. The three inbound
facts (`DeliveryAcceptedByPartner` / `DeliveryRejectedByPartner` / `DeliveryStatusUpdated`) and
`application::deliveries::record_inbound_delivery_event` are **partner-agnostic**. So a new partner is
"mostly one adapter crate + one `services.yaml` entry + one staging table" — no new events, commands,
errors, or drain routing.

CoopCycle is the open-source, cooperatively-owned bike-delivery federation — mission-aligned with
Captain.Food's local-first, independent-restaurant focus (a courier co-op in Tours is a plausible real
partner). It differs from Avelo37 in exactly **one** structural way:

- **Federation.** CoopCycle is **many self-hosted co-op instances** (one per city/co-op), NOT a single
  central API. The outbound base URL, credentials, and webhook secret are **per-instance**, and auth is
  **OAuth2 client-credentials** (per-instance client), not Avelo37's single static bearer key. So the
  adapter config is an *instance registry*, and a job must be *routed to* an instance.

## Decision

**1. A self-contained `crates/adapters/coopcycle` crate**, mirroring `crates/adapters/avelo37`:
`acl.rs` (framework-free per-instance signature verification + partner→domain mapping + the two-layer
`CoopCycleWebhookIngestor`), `raw.rs` (`PgRawCoopCycleEvents` over the staging table), `outbound.rs`
(`CoopCycleDeliveryGateway` + the instance registry + OAuth2 token manager), `http.rs`
(`POST /adapters/coopcycle/{instance}/webhooks`), `main.rs` (standalone binary). Mountable into the
monolith or deployable as its own web service.

**2. Inbound: the two-layer inbox, reused.** A new adapter-owned staging table
`external_coopcycle_events` (`staging: true`) mirrors the verbatim verified webhook body and records
its originating `instance_id`; the ACL translates it into one of the three (existing) delivery facts
and stages it in `inbound_events` with `source = 'coopcycle'`; the existing drain delivers it through
the normal write path — **no drain/journal DSL change** (`inbound_events.source` is already an open
partner tag). No new events/commands/errors ⇒ no new ADR-0032 behaviour-test obligations on that axis.

**3. Federation config = a per-instance registry, env-gated, fail-closed.** `COOPCYCLE_INSTANCES` (a
JSON array of `{ id, base_url, oauth {client_id, client_secret, token_url}, webhook_secret, coverage }`)
configures the registry out-of-repo; unset ⇒ the composition root keeps the logged `NoopDeliveryService`
stand-in (jobs stay open to independent riders), exactly like Avelo37's `AVELO37_API_KEY`. Secrets stay
out of the repo, and the fail-closed principle holds.

**4. Outbound resolution by coverage area.** `offer_job` resolves a job to an instance by matching the
**dropoff postal-code prefix** against each instance's declared `coverage`, fetches/refreshes that
instance's **OAuth2** token (cached per instance), then POSTs a create-delivery carrying our
`deliveryJobId` as the read-back key the instance echoes on every webhook. No covering instance ⇒
`DomainError::Repository` (surfaced on `/saga`, fail-closed). The offer's return is only "received";
acceptance/decline arrive asynchronously as inbound facts.

**5. Inbound routing by instance.** Webhooks arrive per-instance at
`POST /adapters/coopcycle/{instance}/webhooks`; the `{instance}` path segment selects that instance's
`webhook_secret` for verification (the Stripe timestamped-HMAC scheme, ±300s replay window,
fail-closed) and is recorded as `instance_id` on the mirror row. Idempotency key is
`"{instance_id}:{provider event id}"` — federation makes provider ids unique only per instance, so the
ACL namespaces the staging pk.

**6. Observability contract.** `coopcycle-webhook-ingestion` mirrors `avelo37-webhook-ingestion`
(verify → external.persist → acl.translate → inbound.persist → inbound.drain.deliver →
event.store.append), binding the DeliveryJob aggregate + the three inbound events, and adds a
`business.instance_id` attribute on the verify/persist spans — the federation dimension is diagnosable,
not just implemented.

## Alternatives considered

- **A single endpoint + static key (copy Avelo37 verbatim)** — rejected: CoopCycle has no central API.
  A single-endpoint adapter could not reach more than one co-op, defeating the integration's purpose.
- **Configure the instance registry as a seeded DSL table (`referential.yaml`)** — rejected for V0:
  heavier (DSL + seed surface), and per-instance OAuth secrets in a table need careful handling. The
  env-JSON registry matches the established fail-closed partner-config pattern and keeps secrets out of
  the repo. A config-table promotion stays open if instance management becomes a product surface.
- **Resolve the instance by an explicit per-restaurant mapping** — deferred: coverage-area resolution
  (postal prefix) matches "a co-op serves a city" without per-restaurant config; an explicit override
  map can layer on later without changing the port.
- **A single shared webhook endpoint with the instance in the payload** — rejected: the path segment is
  the unambiguous, tamper-evident instance selector for picking the right verification secret *before*
  parsing the body.

## Consequences

### Positive
- Mission-aligned cooperative courier coverage; validates the **multi-instance** config path the issue
  called for (the registry supports N co-ops; Tours is the first entry).
- Proves the Avelo37 partner shape generalizes to a *federated* provider with near-zero domain change —
  no new events/commands/errors/drain routing.
- Unconfigured deployments (V0 Tours) are unchanged: no `COOPCYCLE_INSTANCES`, no behaviour change.

### Negative
- The real CoopCycle wire contract (task event-type names, `data` shape, OAuth scopes, status
  vocabulary) is **assumed** from a best-effort reading — reconciliation against a partnering co-op's
  actual API is mapping-only, isolated to `acl.rs` / `outbound.rs`.
- CoopCycle is **not selected *among*** Avelo37/Uber until the multi-partner ranking foundation lands
  (ADR-20260720-004556's named extension point); until then it plugs in as the configured partner.

### Follow-up actions
- Reconcile the assumed wire shapes + OAuth scopes with a real CoopCycle instance on partnering.
- Multi-partner ranking/selection in the re-offer step (shared foundation issue with #57).
- A `deliveryStatusChanged` subscription for live customer tracking (specs/integrations/coopcycle.md §6).
