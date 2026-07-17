-- 20260717170000 projection_checkpoint (ADR-0040/0043) — the app-layer projector's high-water mark.
-- One row per projector: the last domain_events.position it has folded into its read-model table. The
-- worker reads events WHERE position > checkpoint, applies them, and advances the checkpoint (idempotent
-- on restart). `position = 0` means nothing folded yet (domain_events.position identity starts at 1).
CREATE TABLE projection_checkpoint (
    projector  TEXT        PRIMARY KEY,
    position   BIGINT      NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
