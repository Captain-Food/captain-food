#!/usr/bin/env bash
# Captain.Food acceptance gate (Claude Code Stop hook).
# Blocks loop/turn completion unless the DSL model is valid and generated artifacts are in step.
# Covers schema + behaviour + observability + C4 (all via the codegen validator). App-level gates
# (unit tests, lint, build) run only when they exist, so this is safe in a specs-only repo.
# Exit 0 = gates pass (allow stop); exit 2 = gates fail (block, stderr is fed back).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The codegen is the Rust tool (ADR-0034); make sure cargo is reachable in the hook's shell.
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "stop-gate: cargo not found on PATH — install the Rust toolchain (rustup)." >&2; exit 2; }
MANIFEST="$ROOT/tools/codegen-rs/Cargo.toml"
[ -f "$MANIFEST" ] || { echo "stop-gate: tools/codegen-rs not found" >&2; exit 2; }

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

fail=0
step() { echo "→ $*"; "$@" || fail=1; }

# `cargo run --check` builds first (the compiler is the type gate) then runs the full validator
# (§1–§11: schema + actor wiring + behaviour/rules coverage + observability + C4); exits 1 on errors.
step "${CARGO[@]}" run --quiet --manifest-path "$MANIFEST" -- --check --specs "$SPECS"

# Optional app-level gates — only if a root package.json defines them (no-op until apps/ exists).
if [ -f "$ROOT/package.json" ]; then
  if grep -q '"test"' "$ROOT/package.json"; then ( cd "$ROOT" && npm test --silent ) || fail=1; fi
  if grep -q '"lint"' "$ROOT/package.json"; then ( cd "$ROOT" && npm run --silent lint ) || fail=1; fi
fi

if [ "$fail" -ne 0 ]; then
  echo "stop-gate: acceptance gates FAILED — fix before completing (see output above)." >&2
  exit 2
fi
echo "stop-gate: all acceptance gates passed."
exit 0
