#!/usr/bin/env bash
#
# The home hero's one line, measured on the real window (PR 4 item 4).
#
# Under the total the hero has ONE line: what the wallet has to say on its
# leading side — "Checking…", "Can't reach 3 networks", a read that failed
# inside Vela, or nothing — and at its trailing edge the control that reads
# the figure again, with when it was last read. They were two lines: the
# control alone on the first (a lone ↻ over "Checking…" before the first
# read), and a slot under it that stood empty whenever there was nothing to
# say.
#
#   1. the hero is one height in every state, so is its figure's place, and
#      so is the first row under it: nothing that arrives, changes or goes
#      on the line moves the page;
#   2. the line is one line tall, and the same box in every state;
#   3. the refresh control is ON that line — the same top and height as the
#      line, inside it — never on a line of its own;
#   4. the control's trailing edge is the line's, in every state: its glyph
#      is the last thing on the line, so it does not move when the words
#      beside it change ("Updated 2m" → "Updating…" → nothing yet);
#   5. the line ends where the rows under it end, so the control stands over
#      their trailing actions.
#
# Where gpui puts the boxes is not arithmetic a unit test can do. So this
# opens the home with its hero pinned to each state a session's can be in
# (VELA_HERO, every view the real balance core's, the control drawn as a
# session draws it) and reads the layout's own account of it
# (VELA_LAYOUT_PROBE, src/dev_probe.rs) — in English and in Chinese, whose
# words are other widths.
#
# Like check-signing-sheet.sh it opens real windows, so it is a local gate: a
# CI runner has no display. The windows never take the keyboard
# (VELA_NO_ACTIVATE) and run on scratch state; nothing here opens a camera.
#
# Usage: scripts/check-hero-line.sh [seconds-per-window]
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=target/debug/vela-wallet
DWELL="${1:-2}"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

cargo build

failed=0
fail() {
  echo "FAIL: $*"
  failed=$((failed + 1))
}

STATES="checking updated updating failed offline hidden"
LANGS="en zh"

# open <lang> <state> — one window, in the background, kept for the dwell
# after the first frame that has the whole hero in it.
open() {
  local lang="$1" state="$2" name="$1-$2"
  mkdir -p "$WORK/state-$name"
  (
    env VELA_PAGE=gallery VELA_HERO="$state" VELA_LANG="$lang" VELA_THEME=light \
      VELA_NO_ACTIVATE=1 VELA_SKIP_LAUNCH_ANIMATION=1 \
      VELA_STATE_DIR="$WORK/state-$name" VELA_LAYOUT_PROBE="$WORK/$name.txt" \
      "$BIN" >"$WORK/$name.log" 2>&1 &
    pid=$!
    for _ in $(seq 1 300); do
      grep -q '^home-actions ' "$WORK/$name.txt" 2>/dev/null && break
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    sleep "$DWELL"
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "died" >"$WORK/$name.died"
    fi
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  ) &
}

# value <probe file> <mark> <field> — fields of a box: 2 x, 3 y, 4 width,
# 5 height.
value() {
  awk -v mark="$2" -v field="$3" '
    $1 == mark { print $field; found = 1 }
    END { if (!found) print "none" }
  ' "$1"
}

# Device pixels round a box by half a point at most.
same() { awk -v a="$1" -v b="$2" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(d <= 0.51) }'; }
sum() { awk -v a="$1" -v b="$2" 'BEGIN { print a + b }'; }

echo "opening the home's hero in every state, in two languages (${DWELL}s each)"
for lang in $LANGS; do
  for state in $STATES; do
    open "$lang" "$state"
  done
  wait
done

for died in "$WORK"/*.died; do
  [ -e "$died" ] || continue
  name=$(basename "$died" .died)
  fail "the window for $name did not survive"
  sed 's/^/    /' "$WORK/$name.log" | tail -8
done
if grep -l "camera: opening the default camera" "$WORK"/*.log >/dev/null 2>&1; then
  fail "a camera was opened"
fi

printf '\n%-4s %-9s %8s %9s %8s %8s %10s %10s %10s\n' \
  lang state "hero h" "figure y" "line y" "line h" "refresh y" "refresh r" "next row y"
for lang in $LANGS; do
  reference="$WORK/$lang-updated.txt"
  for state in $STATES; do
    probe="$WORK/$lang-$state.txt"
    if [ ! -s "$probe" ]; then
      fail "$lang/$state: the layout said nothing"
      continue
    fi
    hero_h=$(value "$probe" hero 5)
    figure_y=$(value "$probe" hero-figure 3)
    line_x=$(value "$probe" hero-status 2)
    line_y=$(value "$probe" hero-status 3)
    line_w=$(value "$probe" hero-status 4)
    line_h=$(value "$probe" hero-status 5)
    refresh_x=$(value "$probe" hero-refresh 2)
    refresh_y=$(value "$probe" hero-refresh 3)
    refresh_w=$(value "$probe" hero-refresh 4)
    refresh_h=$(value "$probe" hero-refresh 5)
    next_x=$(value "$probe" home-actions 2)
    next_y=$(value "$probe" home-actions 3)
    next_w=$(value "$probe" home-actions 4)
    for got in "$hero_h" "$figure_y" "$line_y" "$refresh_y" "$next_y"; do
      if [ "$got" = none ]; then
        fail "$lang/$state: a box of the hero was not measured"
        continue 2
      fi
    done
    line_end=$(sum "$line_x" "$line_w")
    refresh_end=$(sum "$refresh_x" "$refresh_w")
    printf '%-4s %-9s %8s %9s %8s %8s %10s %10s %10s\n' \
      "$lang" "$state" "$hero_h" "$figure_y" "$line_y" "$line_h" "$refresh_y" "$refresh_end" "$next_y"

    # 1. One height, one place for the figure, one place for what follows.
    same "$hero_h" "$(value "$reference" hero 5)" ||
      fail "$lang/$state: the hero is $hero_h tall, not $(value "$reference" hero 5)"
    same "$figure_y" "$(value "$reference" hero-figure 3)" ||
      fail "$lang/$state: the figure stands at $figure_y"
    same "$next_y" "$(value "$reference" home-actions 3)" ||
      fail "$lang/$state: the row under the hero stands at $next_y, not $(value "$reference" home-actions 3)"
    # 2. The line: the same box in every state.
    same "$line_y" "$(value "$reference" hero-status 3)" ||
      fail "$lang/$state: the line stands at $line_y"
    same "$line_h" "$(value "$reference" hero-status 5)" ||
      fail "$lang/$state: the line is $line_h tall"
    same "$line_w" "$(value "$reference" hero-status 4)" ||
      fail "$lang/$state: the line is $line_w wide"
    # 3. The control is on it: same top, same height — not a line of its own.
    same "$refresh_y" "$line_y" ||
      fail "$lang/$state: the refresh control stands at $refresh_y, the line at $line_y"
    same "$refresh_h" "$line_h" ||
      fail "$lang/$state: the refresh control is $refresh_h tall, the line $line_h"
    # 4. Its trailing edge is the line's.
    same "$refresh_end" "$line_end" ||
      fail "$lang/$state: the refresh control ends at $refresh_end, the line at $line_end"
    # 5. And the line ends where the rows under it do.
    same "$line_end" "$(sum "$next_x" "$next_w")" ||
      fail "$lang/$state: the line ends at $line_end, the rows at $(sum "$next_x" "$next_w")"
    # The hero ends with its line: nothing — no empty line — under it.
    same "$(sum "$(value "$probe" hero 3)" "$hero_h")" "$(sum "$line_y" "$line_h")" ||
      fail "$lang/$state: the hero does not end with its line"
  done
done

# One line: as tall as one line of its text at the body leading plus the
# button's 4 above and below — under two of them, in either language.
for lang in $LANGS; do
  line_h=$(value "$WORK/$lang-updated.txt" hero-status 5)
  awk -v h="$line_h" 'BEGIN { exit !(h > 20 && h < 40) }' ||
    fail "$lang: the line is $line_h tall (one line is about 26)"
done

if [ "$failed" -gt 0 ]; then
  echo
  echo "$failed check(s) failed"
  exit 1
fi
echo
echo "ok: the hero's status and its refresh control are one line, one height in every state, and nothing under it moves"
