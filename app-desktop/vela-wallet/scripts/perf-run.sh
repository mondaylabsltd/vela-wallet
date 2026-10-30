#!/usr/bin/env bash
#
# The desktop's performance harness (native parity Phase 4): launch, scroll,
# memory and core-dispatch numbers from the RELEASE build, read out of the
# app's own `[vela-perf]` lines (`src/perf.rs`).
#
#   scripts/perf-run.sh [--build] [--runs N] [--idle] [--out DIR]
#
#   --build   cargo build --release --features dev-fixtures first
#   --runs N  how many measured launches (default 5)
#   --idle    also hold one launch for 5 minutes and read its RSS (+5 min)
#   --out DIR where the logs and summary.txt go (default: a new temp dir)
#
# The binary is $BIN, else $CARGO_TARGET_DIR/release/vela-wallet, else
# target/release/vela-wallet. It must be a `dev-fixtures` build: the harness
# signs in through the parallel space (no authenticator) and seeds the
# 200-row history it scrolls — both developer switches a store build does not
# carry.
#
# What each step does, all against a throwaway VELA_STATE_DIR:
#
#   1. seed     one launch that writes the fixture wallet and 200 transfers,
#               and stays long enough for the balances to be read and stored
#               — so every later launch paints its home from the cache;
#   2. launch   N launches, each until its home shows its figure: the marks
#               first_paint / home_painted / home_figure, from process exec;
#   3. scroll   one launch in FRONT (an inactive window is held at 30 fps by
#               gpui): History opens and scrolls itself end to end and back
#               for 6 s; frame/build and scroll/interval percentiles, and
#               gpui's own `frame duration` (draw + present, ZED_MEASUREMENTS);
#   4. idle     (--idle) one launch left alone for 5 minutes: RSS at 10 s,
#               60 s and 300 s, and the dispatch histograms it gathered.
#
# Every launch but the scroll's opens without taking the keyboard
# (VELA_NO_ACTIVATE), so a run on a machine someone is using types nothing
# into it.
set -euo pipefail

APP="$(cd "$(dirname "$0")/.." && pwd)"
RUNS=5
IDLE=0
BUILD=0
OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) BUILD=1 ;;
    --runs) RUNS="$2"; shift ;;
    --idle) IDLE=1 ;;
    --out) OUT="$2"; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if [[ $BUILD == 1 ]]; then
  (cd "$APP" && cargo build --release --features dev-fixtures)
fi
BIN="${BIN:-${CARGO_TARGET_DIR:-$APP/target}/release/vela-wallet}"
[[ -x "$BIN" ]] || { echo "no binary at $BIN (try --build)" >&2; exit 1; }

OUT="${OUT:-$(mktemp -d -t vela-perf)}"
mkdir -p "$OUT"
STATE="$(mktemp -d -t vela-perf-state)"
trap 'rm -rf "$STATE"' EXIT
echo "logs: $OUT"

export VELA_STATE_DIR="$STATE" VELA_PARALLEL_SPACE=1 VELA_PERF=1

# run <log> <seconds> <until-pattern> [ENV=VALUE ...]: start the app, wait
# until the pattern is in its log or the time is up, stop it.
run() {
  local log="$OUT/$1" limit="$2" until="$3"
  shift 3
  env "$@" "$BIN" >"$log" 2>&1 &
  local pid=$! waited=0
  while (( waited < limit * 10 )); do
    if grep -q -- "$until" "$log" 2>/dev/null; then
      break
    fi
    sleep 0.1
    waited=$((waited + 1))
  done
  # The RSS from outside, beside the app's own figure.
  echo "[harness] ps_rss_kb=$(ps -o rss= -p $pid | tr -d ' ')" >>"$log"
  kill "$pid" 2>/dev/null || true
  sleep 0.3
  kill -9 "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

echo "1/4 seed"
run seed.log 25 "idle_10s" VELA_NO_ACTIVATE=1 VELA_PARALLEL_SEED=1 VELA_PERF_ACTIVITY=200

echo "2/4 $RUNS launches"
for i in $(seq 1 "$RUNS"); do
  run "launch-$i.log" 12 "mark home_figure" VELA_NO_ACTIVATE=1
  sleep 1
done

echo "3/4 scroll (the window comes to the front for ~12 s)"
run scroll.log 30 "report scroll" VELA_PERF_SCROLL=1 ZED_MEASUREMENTS=1

if [[ $IDLE == 1 ]]; then
  echo "4/4 idle 5 min"
  run idle.log 330 "report idle_300s" VELA_NO_ACTIVATE=1
fi

python3 -I - "$OUT" "$RUNS" <<'PY' | tee "$OUT/summary.txt"
import re, statistics, sys, pathlib
out, runs = pathlib.Path(sys.argv[1]), int(sys.argv[2])

def marks(text):
    found = {}
    for m in re.finditer(r"\[vela-perf\] mark (\S+) exec=([\d.]+)ms main=([\d.]+)ms rss=([\d.]+)MB", text):
        found.setdefault(m[1], (float(m[2]), float(m[3]), float(m[4])))
    return found

print("== launch (ms from process exec; median of %d, min..max)" % runs)
table = {}
for i in range(1, runs + 1):
    for name, (exec_ms, _main, rss) in marks((out / f"launch-{i}.log").read_text(errors="replace")).items():
        table.setdefault(name, []).append((exec_ms, rss))
for name in ["app_run", "window_opened", "first_paint", "home_painted", "home_figure"]:
    rows = table.get(name)
    if rows:
        ms = [r[0] for r in rows]
        print(f"  {name:<14} {statistics.median(ms):8.1f}  ({min(ms):.1f}..{max(ms):.1f})  rss {statistics.median(r[1] for r in rows):.1f} MB")

def hists(text, reason):
    block = text.split(f"[vela-perf] report {reason}", 1)
    if len(block) < 2:
        return []
    lines = []
    for line in block[1].splitlines()[1:]:
        if not line.startswith("[vela-perf] hist"):
            break
        lines.append(line.replace("[vela-perf] hist ", "  "))
    return lines

scroll = (out / "scroll.log").read_text(errors="replace")
print("== scroll (History, 200 rows)")
for line in scroll.splitlines():
    if "[vela-perf] scroll end" in line:
        print("  " + line.split("] ", 1)[1])
for line in hists(scroll, "scroll"):
    if line.strip().startswith(("frame/", "scroll/")):
        print(line)
gpui = [float(m[1]) * (1000 if m[2] == "s" else 1 if m[2] == "ms" else 0.001)
        for m in re.finditer(r"frame duration: ([\d.]+)(ms|µs|s)\b", scroll.split("scroll begin", 1)[-1].split("scroll end", 1)[0])]
if gpui:
    gpui.sort()
    q = lambda p: gpui[min(len(gpui) - 1, int(p * len(gpui)))]
    print(f"  gpui draw+present n={len(gpui)} p50={q(.5):.2f}ms p95={q(.95):.2f}ms p99={q(.99):.2f}ms max={gpui[-1]:.2f}ms")

print("== core dispatch (all launches + scroll)")
for log in sorted(out.glob("*.log")):
    if log.name.startswith("scroll"):
        text = log.read_text(errors="replace")
        for line in hists(text, "scroll"):
            if line.strip().startswith(("dispatch/", "resolve/", "view/", "store/")):
                print(line)
idle = out / "idle.log"
if idle.exists():
    text = idle.read_text(errors="replace")
    print("== idle")
    for m in re.finditer(r"\[vela-perf\] report (idle_\d+s) main=[\d.]+ms rss=([\d.]+)MB", text):
        print(f"  {m[1]:<10} rss {m[2]} MB")
    for line in hists(text, "idle_300s"):
        if line.strip().startswith(("dispatch/", "resolve/", "view/", "store/")):
            print(line)
PY
