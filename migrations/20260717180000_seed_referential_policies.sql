-- Seed the referential policy tables (ADR-0016/0017/0024/0025/0030) with the indicative values
-- specified in specs/database/tables/referential.yaml. These are reference/configuration tables
-- (ADR-0037) — seeded here, NOT projected from domain_events. Idempotent: re-applying upserts the
-- rows back to the current spec values. effective_from is a fixed deterministic timestamp.

-- Captain service-fee policy (ADR-0016/0017): fee_rate 5.0, buyer_share 60.0, margin_low 55.0,
-- margin_high 70.0 — one active row per currency.
INSERT INTO pricingpolicy (currency, fee_rate, buyer_share, margin_low, margin_high, effective_from)
VALUES ('EUR', 5.0, 60.0, 55.0, 70.0, TIMESTAMPTZ '2026-01-01 00:00:00+00')
ON CONFLICT (currency) DO UPDATE SET
  fee_rate = EXCLUDED.fee_rate,
  buyer_share = EXCLUDED.buyer_share,
  margin_low = EXCLUDED.margin_low,
  margin_high = EXCLUDED.margin_high,
  effective_from = EXCLUDED.effective_from;

-- Per-cuisine Uber Eats price mark-up coefficients (ADR-0024/0030) — one row per CuisineCategory,
-- keyed by the declaration-order ordinal (ADR-0037): FAST_FOOD=0 1.30, PIZZA=1 1.35,
-- TRADITIONAL=2 1.40, BISTRONOMIC=3 1.45, FOOD_TRUCK=4 1.35.
INSERT INTO uberestimationpolicy (cuisine_category, price_coefficient, effective_from)
VALUES
  (0, 1.30, TIMESTAMPTZ '2026-01-01 00:00:00+00'),
  (1, 1.35, TIMESTAMPTZ '2026-01-01 00:00:00+00'),
  (2, 1.40, TIMESTAMPTZ '2026-01-01 00:00:00+00'),
  (3, 1.45, TIMESTAMPTZ '2026-01-01 00:00:00+00'),
  (4, 1.35, TIMESTAMPTZ '2026-01-01 00:00:00+00')
ON CONFLICT (cuisine_category) DO UPDATE SET
  price_coefficient = EXCLUDED.price_coefficient,
  effective_from = EXCLUDED.effective_from;

-- Uber Eats split/fee assumptions for the estimated comparison (ADR-0024/0025/0030):
-- uber_commission_pct 30.0, rider_base_cents 285, rider_per_km_cents 80, avg_delivery_fee_cents 399,
-- platform_fee_pct 10.0 — one row per currency.
INSERT INTO ubersplitpolicy (currency, uber_commission_pct, rider_base_cents, rider_per_km_cents,
                             avg_delivery_fee_cents, platform_fee_pct, effective_from)
VALUES ('EUR', 30.0, 285, 80, 399, 10.0, TIMESTAMPTZ '2026-01-01 00:00:00+00')
ON CONFLICT (currency) DO UPDATE SET
  uber_commission_pct = EXCLUDED.uber_commission_pct,
  rider_base_cents = EXCLUDED.rider_base_cents,
  rider_per_km_cents = EXCLUDED.rider_per_km_cents,
  avg_delivery_fee_cents = EXCLUDED.avg_delivery_fee_cents,
  platform_fee_pct = EXCLUDED.platform_fee_pct,
  effective_from = EXCLUDED.effective_from;
