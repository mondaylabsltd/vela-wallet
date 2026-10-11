#!/usr/bin/env bash
#
# Settings does not drop when the keys block loads, measured on the real
# window (PR 4 item 13.1).
#
# Settings → Account lists the passkeys that control the wallet. Which they
# are is the registry's to say, and the registry answers a second or so after
# the page opens. The block used to be ONE skeleton row until then; when the
# answer landed it grew to the wallet's keys and everything under it — the
# copy-to-Ethereum row, Sign Out, Erase — dropped: about 250 pt on the
# iPhone, 151 px on the Xiaomi, and 177 pt here for a wallet of three keys.
#
# The block now opens on the rows this device's own record gives (how many
# keys, what they are called, which one signs here), each at its final size,
# and the registry's answer fills them in:
#
#   1. asked or answered, the block is one height, every key's row is one
#      box, and the copy-record row and the first row under the block stand
#      in one place — for a wallet of one key and of three;
#   2. the same when the answer LANDS on an open page: the first frame
#      against the settled one;
#   3. a registry that never answers leaves every row where it stood and adds
#      one sentence under them ("Couldn't reach the registry…") — the block
#      grows by that line and nothing else;
#   4. the copy-record row is one height through "Checking…" and every answer
#      that replaces it (its status is one line).
#
# What a row SAYS while the registry is asked — nothing the registry may
# contradict — is `settings::live::key_row`'s, and unit-tested there. Where
# gpui puts the boxes is not arithmetic a unit test can do. So this opens the
# drawn Account page with its keys block pinned to each state (VELA_KEYS,
# over VELA_BACKUP_STATE for the copy-record row) — at the narrowest window
# the app allows and at a wider one, in English and in Chinese — and reads
# the layout's own account of it (VELA_LAYOUT_PROBE, src/dev_probe.rs).
#
# Like check-signing-sheet.sh it opens real windows, so it is a local gate: a
# CI runner has no display. The windows never take the keyboard
# (VELA_NO_ACTIVATE) and run on scratch state with no wallet in it: nobody's
# registry is asked, and nothing here opens a camera. They open BEHIND
# whatever is in front, and a window that is wholly covered is drawn once and
# not again — the landings (rule 2, and rule 3's) watch a second frame, so
# they need a desktop on which some of the window shows; the pinned states
# (rules 1 and 4, and rule 3's silent block against the asked one) read the
# first frame, and are measured either way.
#
# Usage: scripts/check-settings-keys.sh [seconds-per-window]
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

LANGS="en zh"
WINDOWS="narrowest wide"
# The keys' states, then the copy-record row's answers under three keys.
STATES="asking1 1 asking3 3 silent3"
BACKUPS="backed_up could_not_check not_copyable"
LANDINGS="asking1>1 asking3>3 asking3>silent3"

# open <name> <lang> <window> <landing: yes|no> ENV... — one window, in the
# background, kept for the dwell after its first whole frame; a landing is
# kept until its answer is in (or half a minute has gone by).
open() {
  local name="$1" lang="$2" window="$3" landing="$4"
  shift 4
  mkdir -p "$WORK/state-$name"
  (
    # The narrowest window is the app's least size, which is its design size.
    size=1280x800
    if [ "$window" = wide ]; then size=1440x900; fi
    env VELA_PAGE=gallery VELA_GALLERY_TAB=DST1 VELA_LANG="$lang" VELA_THEME=light \
      VELA_NO_ACTIVATE=1 VELA_SKIP_LAUNCH_ANIMATION=1 VELA_WINDOW="$size" \
      VELA_STATE_DIR="$WORK/state-$name" VELA_LAYOUT_PROBE="$WORK/$name.txt" \
      "$@" "$BIN" >"$WORK/$name.log" 2>&1 &
    pid=$!
    for _ in $(seq 1 300); do
      grep -q '^keys-next ' "$WORK/$name.txt" 2>/dev/null && break
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    cp "$WORK/$name.txt" "$WORK/$name.early" 2>/dev/null || true
    if [ "$landing" = yes ]; then
      for _ in $(seq 1 300); do
        grep -q '^keys-answered ' "$WORK/$name.txt" 2>/dev/null && break
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
      done
    fi
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
more() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a > b + 0.51) }'; }
minus() { awk -v a="$1" -v b="$2" 'BEGIN { print a - b }'; }

echo "opening the Account page's keys block in every state, two windows, two languages (${DWELL}s each)"
n=0
batch() {
  n=$((n + 1))
  if [ $((n % 6)) -eq 0 ]; then wait; fi
}
for lang in $LANGS; do
  for window in $WINDOWS; do
    for state in $STATES; do
      open "$lang-$window-$state" "$lang" "$window" no "VELA_KEYS=$state" VELA_BACKUP_STATE=not_backed_up
      batch
    done
    for backup in $BACKUPS; do
      open "$lang-$window-backup-$backup" "$lang" "$window" no VELA_KEYS=3 "VELA_BACKUP_STATE=$backup"
      batch
    done
    for landing in $LANDINGS; do
      open "$lang-$window-lands-${landing/>/-}" "$lang" "$window" yes "VELA_KEYS=$landing" VELA_BACKUP_STATE=not_backed_up
      batch
    done
  done
done
wait

for died in "$WORK"/*.died; do
  [ -e "$died" ] || continue
  name=$(basename "$died" .died)
  fail "the window for $name did not survive"
  sed 's/^/    /' "$WORK/$name.log" | tail -8
done
if grep -l "camera: opening the default camera" "$WORK"/*.log >/dev/null 2>&1; then
  fail "a camera was opened"
fi

# alike <probe> <reference> <what> <rows> — the block, each of its rows, the
# copy-record row and the row under the block: the same boxes in both.
alike() {
  local probe="$1" reference="$2" what="$3" rows="$4" mark field got want marks
  marks="keys-block keys-backup keys-next"
  for row in $(seq 0 $((rows - 1))); do marks="$marks keys-row-$row"; done
  for mark in $marks; do
    for field in 3 5; do
      got=$(value "$probe" "$mark" "$field")
      want=$(value "$reference" "$mark" "$field")
      if [ "$got" = none ] || [ "$want" = none ]; then
        fail "$what: $mark was not measured"
        continue 2
      fi
      same "$got" "$want" || fail "$what: $mark is $got (field $field), not $want"
    done
  done
}

printf '\n%-4s %-9s %-22s %8s %7s %7s %7s %9s %9s %8s\n' \
  lang window state "block h" "row0 h" "row1 h" "row2 h" "backup y" "backup h" "next y"
show() { # <probe> <lang> <window> <state>
  printf '%-4s %-9s %-22s %8s %7s %7s %7s %9s %9s %8s\n' "$2" "$3" "$4" \
    "$(value "$1" keys-block 5)" "$(value "$1" keys-row-0 5)" "$(value "$1" keys-row-1 5)" \
    "$(value "$1" keys-row-2 5)" "$(value "$1" keys-backup 3)" "$(value "$1" keys-backup 5)" \
    "$(value "$1" keys-next 3)"
}
for lang in $LANGS; do
  for window in $WINDOWS; do
    at="$WORK/$lang-$window"
    for state in $STATES; do
      if [ ! -s "$at-$state.txt" ]; then
        fail "$lang/$window/$state: the layout said nothing"
        continue 2
      fi
      show "$at-$state.txt" "$lang" "$window" "$state"
    done

    # 1. Asked or answered: one height, one place for everything.
    alike "$at-1.txt" "$at-asking1.txt" "$lang/$window: one key, answered against asked" 1
    alike "$at-3.txt" "$at-asking3.txt" "$lang/$window: three keys, answered against asked" 3
    # (These cases measure something: the states are what their names say,
    # and three keys are three rows.)
    [ "$(value "$at-asking3.txt" keys-asking 3)" != none ] ||
      fail "$lang/$window/asking3: the block is not asking"
    [ "$(value "$at-asking3.txt" keys-answered 3)" = none ] ||
      fail "$lang/$window/asking3: the block already has its answer"
    [ "$(value "$at-3.txt" keys-answered 3)" != none ] ||
      fail "$lang/$window/3: the block has no answer"
    [ "$(value "$at-asking3.txt" keys-row-2 5)" != none ] ||
      fail "$lang/$window/asking3: the third key has no row while asking"
    [ "$(value "$at-asking1.txt" keys-row-1 5)" = none ] ||
      fail "$lang/$window/asking1: a wallet of one key has a second row"
    more "$(value "$at-asking3.txt" keys-next 3)" "$(value "$at-asking1.txt" keys-next 3)" ||
      fail "$lang/$window: three keys take no more room than one"

    # 2. …and when the answer lands on an open page.
    for landing in $LANDINGS; do
      name="$at-lands-${landing/>/-}"
      early="$name.early"
      late="$name.txt"
      if [ ! -s "$early" ] || [ ! -s "$late" ]; then
        fail "$lang/$window/$landing: the layout said nothing"
        continue
      fi
      [ "$(value "$early" keys-answered 3)" = none ] ||
        fail "$lang/$window/$landing: the first frame already had its answer (this case measures nothing)"
      if [ "$(value "$late" keys-answered 3)" = none ]; then
        # Nothing to compare: the settled frame was never drawn. gpui stops
        # drawing a window nobody can see (its display link is stopped while
        # the window is wholly covered or on another Space), so a landing is
        # only seen on a desktop where this window shows.
        fail "$lang/$window/$landing: the answer never landed (is the window covered? a window nobody can see is not redrawn)"
        continue
      fi
      rows=3
      [ "$landing" != "asking1>1" ] || rows=1
      if [ "$landing" = "asking3>silent3" ]; then
        # 3. Silent: every key's row where it stood; the block and what is
        # under the rows lower by the one sentence, and by nothing else.
        for row in 0 1 2; do
          for field in 3 5; do
            same "$(value "$late" "keys-row-$row" "$field")" "$(value "$early" "keys-row-$row" "$field")" ||
              fail "$lang/$window/$landing: row $row moved"
          done
        done
        grew=$(minus "$(value "$late" keys-block 5)" "$(value "$early" keys-block 5)")
        more "$grew" 0 || fail "$lang/$window/$landing: the sentence took no room (this case measures nothing)"
        same "$(minus "$(value "$late" keys-next 3)" "$(value "$early" keys-next 3)")" "$grew" ||
          fail "$lang/$window/$landing: the row under the block moved by more than the sentence"
        same "$(minus "$(value "$late" keys-backup 3)" "$(value "$early" keys-backup 3)")" "$grew" ||
          fail "$lang/$window/$landing: the copy-record row moved by more than the sentence"
        awk -v grew="$grew" 'BEGIN { exit !(grew < 60) }' ||
          fail "$lang/$window/$landing: the block grew by $grew — more than a sentence"
        printf '%-4s %-9s %-22s next y %s -> %s (+%s: the one sentence)\n' "$lang" "$window" "$landing" \
          "$(value "$early" keys-next 3)" "$(value "$late" keys-next 3)" "$grew"
      else
        alike "$late" "$early" "$lang/$window/$landing: settled against the first frame" "$rows"
        printf '%-4s %-9s %-22s next y %s -> %s, block h %s -> %s\n' "$lang" "$window" "$landing" \
          "$(value "$early" keys-next 3)" "$(value "$late" keys-next 3)" \
          "$(value "$early" keys-block 5)" "$(value "$late" keys-block 5)"
      fi
    done
    # 3. The same statically: silent against asked.
    for row in 0 1 2; do
      same "$(value "$at-silent3.txt" "keys-row-$row" 3)" "$(value "$at-asking3.txt" "keys-row-$row" 3)" ||
        fail "$lang/$window/silent3: row $row is not where it stood while asking"
      same "$(value "$at-silent3.txt" "keys-row-$row" 5)" "$(value "$at-asking3.txt" "keys-row-$row" 5)" ||
        fail "$lang/$window/silent3: row $row changed height"
    done

    # 4. The copy-record row: one height through "Checking…" (while the keys
    # are asked) and every answer.
    checking=$(value "$at-asking3.txt" keys-backup 5)
    for backup in not_backed_up $BACKUPS; do
      probe="$at-backup-$backup.txt"
      [ "$backup" != not_backed_up ] || probe="$at-3.txt"
      if [ ! -s "$probe" ]; then
        fail "$lang/$window/backup $backup: the layout said nothing"
        continue
      fi
      got=$(value "$probe" keys-backup 5)
      same "$got" "$checking" ||
        fail "$lang/$window/backup $backup: the copy-record row is $got tall, $checking while checking"
      same "$(value "$probe" keys-backup 3)" "$(value "$at-asking3.txt" keys-backup 3)" ||
        fail "$lang/$window/backup $backup: the copy-record row moved"
    done
    # The paragraph under the row goes when the chain says this wallet can
    # never be copied — an answer nobody has before it is asked. Said, not
    # failed: it is the one thing under the block that still moves.
    printf '%-4s %-9s %-22s next y %s (the paragraph is gone: %s)\n' "$lang" "$window" \
      "backup not_copyable" "$(value "$at-backup-not_copyable.txt" keys-next 3)" \
      "$(minus "$(value "$at-backup-not_copyable.txt" keys-next 3)" "$(value "$at-3.txt" keys-next 3)")"
  done
done

if [ "$failed" -gt 0 ]; then
  echo
  echo "$failed check(s) failed"
  exit 1
fi
echo
echo "ok: the keys block is one height asked and answered, every row and the rows under it stand in one place, and a silent registry adds one sentence"
