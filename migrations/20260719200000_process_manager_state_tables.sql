-- 20260719200000 process_manager_state_tables (ADR-20260719-172821) — the four saga state tables,
-- copied from specs/generated/schema.generated.sql (source: specs/database/tables/process_managers.yaml).
-- One row = one saga run; pk = the run's correlation identity. PRIVATE to their process manager: no
-- projection reads them, no query serves them. Enum-scalar columns are INTEGER declaration-order
-- ordinals (ADR-0037); last_update_utc is maintained by the runtime envelope (stamped now() on upsert).
CREATE TABLE payment_process_manager (
  cart_id UUID PRIMARY KEY,
  order_id UUID NOT NULL,
  payment_intent_id TEXT NOT NULL UNIQUE,
  process_status INTEGER NOT NULL,
  payment_status INTEGER NOT NULL,
  last_processed_stripe_event_id TEXT NULL,
  last_update_utc TIMESTAMPTZ NOT NULL
);

CREATE TABLE refund_process_manager (
  order_id UUID PRIMARY KEY,
  payment_intent_id TEXT NULL,
  refund_id TEXT NULL,
  process_status INTEGER NOT NULL,
  approved_amount_cents BIGINT NULL,
  reason TEXT NULL,
  last_update_utc TIMESTAMPTZ NOT NULL
);

CREATE TABLE cart_binding_process_manager (
  session_id UUID PRIMARY KEY,
  customer_id UUID NOT NULL,
  last_update_utc TIMESTAMPTZ NOT NULL
);

CREATE TABLE delivery_dispatch_process_manager (
  order_id UUID PRIMARY KEY,
  restaurant_id UUID NOT NULL,
  delivery_job_id UUID NOT NULL UNIQUE,
  process_status INTEGER NOT NULL,
  last_update_utc TIMESTAMPTZ NOT NULL
);
