# Contributing to Captain.Food

Ahoy — thanks for considering coming aboard. This repository runs on a strict operating model;
reading this page first will save you (and the reviewers) a round trip.

## Ground rules

- **English only.** All repository content — code, comments, docs, commit messages, identifiers —
  is written in English (repository convention, see [`CLAUDE.md`](CLAUDE.md)).
- **The `specs/*.yaml` DSL is the source of truth.** Everything under [`specs/generated/`](specs/generated/)
  is generated — **never hand-edit generated output**. Change the spec or the emitter
  ([`tools/codegen-rs/`](tools/codegen-rs/)) and regenerate.
- **Decisions become ADRs.** Cross-cutting decisions land as an ADR in [`docs/adr/`](docs/adr/)
  **in the same change**, with a date-time id (`ADR-YYYYMMDD-HHMMSS`).
- **Completeness is part of every change** (ADR-0032): a new command/event/error also needs a
  behaviour test (+ its `rules:` link); a new mutation/query also needs a story-map step; a new
  business rule also needs a test. The validator blocks otherwise — extend the specs, don't weaken
  the gate.
- Substantive changes keep [`docs/STATUS.md`](docs/STATUS.md) current.

## Getting set up

You need a Rust toolchain (`cargo`; the version is pinned by [`rust-toolchain.toml`](rust-toolchain.toml)).

```bash
make help         # list every entrypoint
make validate     # the single blocking gate — must be 0 errors
make generate     # regenerate every artifact from the specs
make rust         # build + test + validate + generate
```

## Making a change

1. Read the relevant spec file(s) in [`specs/`](specs/) before touching anything — the map of the
   whole model is in [`CLAUDE.md`](CLAUDE.md) and the generated documentation
   (`specs/generated/documentation.generated.md`).
2. Make the spec change (with its tests/stories/rules), then `make generate` and commit the
   regenerated artifacts alongside it — CI fails on any spec↔generation drift.
3. Run `make validate` locally: it must report **0 errors**.
4. Open a pull request. The **ci** workflow runs the whole gate on every push: full workspace
   build, complete behaviour-test suite, spec validation, regeneration + drift check.

## License of contributions

Captain.Food is released under the **Captain.Food Coopyleft License** (AGPL-3.0-based, commercial
use reserved to cooperatives and non-/limited-profit organizations of the social and solidarity
economy — see [`LICENSE.md`](LICENSE.md)). By contributing, you agree that your contributions are
licensed under the same terms.
