# ADR-0029 — Multi-recipient tips domain model

## Status

Accepted

## Context

The specs modelled **rider-only** tipping (`TipRider` → `RiderTipped` → `Order.riderTip`). kDrive
**ADR-012** (Nov-2025, Accepted MVP) wants optional tips to **rider / restaurant / Captain**, possibly
split across several, **separate from the core 3-way split** (ADR-0028), with **Captain skimming 0%**
(100% passes through) and **all tips logged to Open Collective**. In addition, the **restaurant** (not
only the customer) may tip — e.g. thanking the courier or Captain. This records the generalized model.

## Decision

1. **Generic `Tip {recipient: TipRecipient, amount: Money}` list** on one **`TipOrder`** command →
   one **`OrderTipped`** fact. Covers single-recipient, percentage-resolved, or fixed-amount
   distributions (ADR-012 options 1–3). Replaces `TipRider`/`RiderTipped`.
2. **`TipRecipient` = RIDER | RESTAURANT | CAPTAIN.**
3. **Tipper dimension**: `OrderTipped.tippedBy: Tipper` (CUSTOMER | RESTAURANT), **derived server-side
   from the caller's role** (not client-supplied). The **restaurant may tip the rider / Captain** (not
   itself → `InvalidTipRecipient`); the customer may tip any of the three. Mutation `tipOrder` roles:
   `[CUSTOMER, RESTAURANT, RESTAURANT_ACCOUNT]`.
4. **Additive**: multiple `OrderTipped` accumulate; `View_OrderTracking` sums per recipient
   (`rider_tip_cents`, `restaurant_tip_cents`, `captain_tip_cents`, across all tippers). Supports the
   ADR-012 "at checkout OR post-delivery" timing. No double-tip guard (the old `RiderAlreadyTipped` is removed).
5. **Tips are OUT of `PaymentBreakdown`** (separate; Captain 0% skim) — their own event + per-recipient
   columns feed Open-Collective totals; `Order` exposes `riderTip` / `restaurantTip` / `captainTip`.

## Alternatives considered
- **Keep rider-only tipping** — contradicts ADR-012 (restaurant + Captain recipients) and the
  restaurant-tips-courier request. Rejected.
- **Discrete per-recipient tip commands** — more surface, less flexible than one `Tip` list. Rejected.
- **Fold tips into `PaymentBreakdown`** — breaks the "separate, Captain-0%-skim" principle. Rejected.
- **Client-supplied tipper** — untrustworthy; derived from the authenticated role instead.

## Consequences
### Positive
- One flexible tip flow for both customers and restaurants; per-recipient Open-Collective transparency;
  Captain never skims tips; additive tips fit checkout-or-later timing.
### Negative
- `Order` grew three tip fields; `tippedBy` attribution lives on the event (not aggregated in the view).
### Follow-up actions (runtime, deferred)
- At-checkout tip capture, the Stripe **100%-pass-through** tip transfers, and the actual Open-Collective
  logging land with the payments runtime (like the core transfers, ADR-0028).

## References
kDrive ADR-012; `specs/scalars.yaml` (`TipRecipient`, `Tipper`), `specs/entities.yaml#/Tip`,
`specs/{commands,events}.yaml` (`TipOrder`/`OrderTipped`), `specs/views.yaml#/View_OrderTracking`,
`specs/api.yaml` (`tipOrder`, `Order.*Tip`); complements ADR-0028 (core split).
