#!/usr/bin/env bash
# Captain.Food acceptance gate (Claude Code Stop hook).
# Blocks loop/turn completion unless the DSL model is valid and generated artifacts are in step.
# Covers schema + behaviour + observability + C4 (all via the codegen validator). App-level gates
# (unit tests, lint, build) run only when they exist, so this is safe in a specs-only repo.
# Exit 0 = gates pass (allow stop); exit 2 = gates fail (block, stderr is fed back).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/tools/codegen" 2>/dev/null || { echo "stop-gate: tools/codegen not found" >&2; exit 2; }

fail=0
step() { echo "→ $*"; "$@" || fail=1; }

step npm run --silent typecheck
step npm run --silent validate   # schema + behaviour coverage + observability + C4 (cli exits 1 on errors)

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
