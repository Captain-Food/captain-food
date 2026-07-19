## What & why

<!-- What does this change, and which use case / decision drives it? Link specs & ADRs. -->

## Gate checklist

- [ ] `make validate` passes with **0 errors**
- [ ] `make generate` ran — regenerated artifacts are committed, no spec↔generation drift
- [ ] No hand edits under `specs/generated/**` (spec/emitter changed instead)
- [ ] Completeness (ADR-0032): new commands/events/errors have behaviour tests (+ `rules:` links); new mutations/queries have story-map steps
- [ ] Cross-cutting decisions recorded as an ADR (`ADR-YYYYMMDD-HHMMSS`) in this same change
- [ ] `docs/STATUS.md` updated if the change is substantive
- [ ] All content in English
