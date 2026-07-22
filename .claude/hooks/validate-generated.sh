#!/usr/bin/env bash
# Captain.Food file-write guard (Claude Code PostToolUse hook on Write|Edit).
# - Refuses hand-edits to GENERATED output (specs/generated/** and the database.md GENERATED region).
# - After a spec change (specs/**), re-runs validation and returns contextual feedback.
# Exit 0 = ok; exit 2 = block with feedback (stderr is fed back to the model).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The codegen is the Rust tool (ADR-0034); make sure cargo is reachable in the hook's shell.
export PATH="$HOME/.cargo/bin:$PATH"
MANIFEST="$ROOT/tools/codegen-rs/Cargo.toml"
# Under Cygwin the rustup `cargo` proxy mis-detects its own argv[0] and runs as `rustup`, so any
# `cargo run` fails with "invalid value 'run' for '[+toolchain]'"; route through `rustup run` there.
CARGO=(cargo)
case "$(uname -s 2>/dev/null)" in
  CYGWIN*) CARGO=(rustup run "${RUST_CHANNEL:-stable}" cargo) ;;
esac
# ...and a native Windows cargo cannot read Cygwin/MSYS `/cygdrive/...` paths: hand it `C:/...`.
winpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
MANIFEST="$(winpath "$MANIFEST")"
SPECS="$(winpath "$ROOT/specs")"
payload="$(cat 2>/dev/null || true)"
# Best-effort extract of the written path from the tool-input JSON (no jq dependency).
path="$(printf '%s' "$payload" | grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed -E 's/.*"file_path"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/')"
[ -z "$path" ] && exit 0

case "$path" in
  */specs/generated/*|specs/generated/*)
    echo "Refusing: '$path' is GENERATED output. Change the spec or emitter and run 'make generate' instead." >&2
    exit 2 ;;
esac

case "$path" in
  */specs/*|specs/*)
    if ! command -v cargo >/dev/null 2>&1; then exit 0; fi  # no toolchain → skip (CI still gates)
    if ! "${CARGO[@]}" run --quiet --manifest-path "$MANIFEST" -- --check --specs "$SPECS" ; then
      echo "The spec change did not validate — fix the model (see errors above) before continuing." >&2
      exit 2
    fi ;;
esac
exit 0
