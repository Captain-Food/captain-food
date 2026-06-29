#!/usr/bin/env bash
# Self-imposed WEEKLY time budget for autonomous loops/routines.
# Claude Code has NO native "minutes per week" cap, so we track it ourselves in a committed state file
# (.claude/loop-budget.json) that persists across cloud-routine runs. Budget resets each ISO week.
#
# Usage:
#   loop-budget.sh check    # exit 0 if budget remains this week, exit 2 if exhausted (skip the run)
#   loop-budget.sh start    # check + stamp a start time
#   loop-budget.sh stop     # add elapsed-since-start to the weekly total; exit 2 once over budget
#
# Configure the cap by editing "weeklyBudgetSeconds" in .claude/loop-budget.json (default 1800 = 30 min).
set -uo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # .claude
export BUDGET_FILE="$DIR/loop-budget.json"
export BUDGET_CMD="${1:-check}"

node <<'NODE'
const fs = require('fs');
const file = process.env.BUDGET_FILE, cmd = process.env.BUDGET_CMD;
function isoWeek(date) {
  const d = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
  const dayNum = (d.getUTCDay() + 6) % 7;
  d.setUTCDate(d.getUTCDate() - dayNum + 3);
  const firstThursday = d.getTime();
  d.setUTCMonth(0, 1);
  if (d.getUTCDay() !== 4) d.setUTCMonth(0, 1 + ((4 - d.getUTCDay()) + 7) % 7);
  const week = 1 + Math.ceil((firstThursday - d.getTime()) / 604800000);
  return new Date(firstThursday).getUTCFullYear() + '-W' + String(week).padStart(2, '0');
}
let s = { weeklyBudgetSeconds: 1800, week: '', secondsUsed: 0, startedAt: 0 };
try { s = Object.assign(s, JSON.parse(fs.readFileSync(file, 'utf8'))); } catch (e) {}
const now = Date.now();
const wk = isoWeek(new Date(now));
if (s.week !== wk) { s.week = wk; s.secondsUsed = 0; s.startedAt = 0; } // weekly reset
const budget = Number(s.weeklyBudgetSeconds) || 1800;
const mins = (x) => (x / 60).toFixed(1);
const over = () => s.secondsUsed >= budget;
const save = () => fs.writeFileSync(file, JSON.stringify(s, null, 2) + '\n');

if (cmd === 'check' || cmd === 'start') {
  if (over()) { console.error(`⛔ weekly loop budget exhausted: ${mins(s.secondsUsed)}m / ${mins(budget)}m (week ${wk}); resets Monday.`); save(); process.exit(2); }
  if (cmd === 'start') s.startedAt = now;
  save();
  console.error(`✓ loop budget OK: ${mins(s.secondsUsed)}m / ${mins(budget)}m used (week ${wk}).`);
  process.exit(0);
}
if (cmd === 'stop') {
  if (s.startedAt) { s.secondsUsed += Math.round((now - s.startedAt) / 1000); s.startedAt = 0; }
  save();
  console.error(`• loop budget: ${mins(s.secondsUsed)}m / ${mins(budget)}m used (week ${wk}).`);
  process.exit(over() ? 2 : 0);
}
console.error('usage: loop-budget.sh check|start|stop');
process.exit(64);
NODE
