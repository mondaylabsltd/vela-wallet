#!/usr/bin/env bash
#
# The signing sheet's verdict rule, measured on the real window.
#
# The simulation's verdict is the one part of a signing sheet a site cannot
# write, so nothing of it may be hidden (PR 3 device round):
#
#   1. its place is a least height, there from the first frame, so the usual
#      verdict lands and moves nothing;
#   2. a taller verdict is shown whole — the place is as tall as the verdict,
#      every row and the warning at their own height, no scroll or clip of
#      its own;
#   3. the confirm is pinned at the bottom of the column, outside the
#      scrolling body: its position is the same with no verdict yet, with
#      each kind of verdict and with a tall one, and it is always wholly on
#      screen; when the sheet is taller than the window the body scrolls;
#   4. a verdict that lands partly outside the body is brought into view;
#   5. the confirm WAITS for the verdict (PR 3 fix C), and that costs no
#      movement either: while the answer is out a line under the confirm says
#      so, and once an answer is in — or the wait has run out, and the
#      verdict's place says "couldn't check" — the line keeps its place with
#      nothing to say, so the confirm stands where it stood. A request
#      nothing simulates has no such line at all.
#
# None of that is arithmetic a unit test can do: it is where gpui puts the
# boxes. So this opens the drawn send (gallery, cs1) under every simulation
# answer (VELA_SIM) — at the design size, in a taller window and, for a sheet
# too tall for its window, at the largest text size — and reads the layout's
# own account of it (VELA_LAYOUT_PROBE, src/dev_probe.rs).
#
# Like sweep-gallery.sh it opens real windows, so it is a local gate: a CI
# runner has no display. The windows never take the keyboard
# (VELA_NO_ACTIVATE) and run on scratch state; nothing here opens a camera.
#
# Usage: scripts/check-signing-sheet.sh [seconds-per-window]
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=target/debug/vela-wallet
DWELL="${1:-3}"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

cargo build

mkdir -p "$WORK/standard" "$WORK/largest"
echo '{ "vela.textScale": "standard" }' >"$WORK/standard/wallet.json"
echo '{ "vela.textScale": "xlarge" }' >"$WORK/largest/wallet.json"

failed=0
fail() {
  echo "FAIL: $*"
  failed=$((failed + 1))
}

# open <name> <text: standard|largest> <seconds> [ENV=VAL...] — one window,
# in the background, kept for that long after its first frame. Its probe
# lands in $WORK/<name>.txt, and what that first frame said (before a pinned
# answer lands) in $WORK/<name>.early.
open() {
  local name="$1" text="$2" dwell="$3"
  shift 3
  (
    env VELA_PAGE=gallery VELA_SIGNING_STATE=cs1 VELA_LANG=en VELA_THEME=light \
      VELA_NO_ACTIVATE=1 VELA_SKIP_LAUNCH_ANIMATION=1 \
      VELA_STATE_DIR="$WORK/$text" VELA_LAYOUT_PROBE="$WORK/$name.txt" "$@" \
      "$BIN" >"$WORK/$name.log" 2>&1 &
    pid=$!
    # The first frame, whenever a busy machine gets to it — whole: the
    # confirm is the last box of the column it reports.
    for _ in $(seq 1 300); do
      grep -q '^signing-confirm ' "$WORK/$name.txt" 2>/dev/null && break
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    cp "$WORK/$name.txt" "$WORK/$name.early" 2>/dev/null || true
    sleep "$dwell"
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "died" >"$WORK/$name.died"
    fi
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  ) &
}

# value <probe file> <mark> <field> — fields of a box: 2 x, 3 y, 4 width,
# 5 height; of `panel-body`: 2 scrolled, 3 can scroll, 4 height, 5 top.
value() {
  awk -v mark="$2" -v field="$3" '
    $1 == mark { print $field; found = 1 }
    END { if (!found) print "none" }
  ' "$1"
}

# Device pixels round a box by half a point at most.
same() { awk -v a="$1" -v b="$2" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(d <= 0.51) }'; }
more() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a > b + 0.51) }'; }

# outside <probe file> <mark of the box to be inside> — the verdict's rows
# and its note that are not wholly inside that box, by name.
outside() {
  awk -v within="$2" '
    BEGIN { n = 0 }
    $1 == within && within == "panel-body" { top = $5; bottom = $5 + $4 }
    $1 == within && within != "panel-body" { top = $3; bottom = $3 + $5 }
    $1 ~ /^verdict-(row-[0-9]+|note)$/ { name[n] = $1; y[n] = $3; h[n] = $5; n++ }
    END {
      for (i = 0; i < n; i++)
        if (y[i] < top - 0.51 || y[i] + h[i] > bottom + 0.51) printf "%s ", name[i]
    }
  ' "$1"
}

ANSWERS="out nothing send swap caution danger three unverified tall waited"

echo "opening the drawn send under every answer, three ways (${DWELL}s each)"
n=0
for answer in none $ANSWERS; do
  for way in design tall-window largest-text; do
    case "$way" in
      design) set -- standard ;;
      tall-window) set -- standard VELA_WINDOW=1280x1000 ;;
      largest-text) set -- largest ;;
    esac
    if [ "$answer" != none ]; then set -- "$@" "VELA_SIM=$answer"; fi
    text="$1"
    shift
    open "$way-$answer" "$text" "$DWELL" "$@"
    n=$((n + 1))
    if [ $((n % 6)) -eq 0 ]; then wait; fi
  done
done
# The sheet too tall for its window, with the body left where a pin puts it
# (no bringing into view), and an answer that lands after the window opens.
open short-top largest "$DWELL" VELA_SIM=tall VELA_PANEL_SCROLL=0
open short-bottom largest "$DWELL" VELA_SIM=tall VELA_PANEL_SCROLL=bottom
# The pinned answer lands 2.5 s after the window opens
# (`signing::fixtures::SIM_PIN_LANDS_AFTER`), then the body glides.
LANDS=$(awk -v dwell="$DWELL" 'BEGIN { print dwell + 3 }')
open short-lands largest "$LANDS" "VELA_SIM=out>tall"
# The held confirm's three ways out (5.): the verdict lands, the wait runs
# out with none, and an answer comes after that.
open held-lands standard "$LANDS" "VELA_SIM=out>send"
open held-waits standard "$LANDS" "VELA_SIM=out>waited"
open late-answer standard "$LANDS" "VELA_SIM=waited>send"
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

printf '\n%-13s %-11s %9s %8s %9s %10s %13s %7s\n' way answer "confirm y" bottom "place h" "content h" "scrolled/max" line
for way in design tall-window largest-text; do
  reference=$(value "$WORK/$way-out.txt" signing-confirm 3)
  least=$(value "$WORK/$way-out.txt" verdict-place 5)
  # The held confirm's line, as it is said while the answer is out.
  line_y=$(value "$WORK/$way-out.txt" signing-note 3)
  line_h=$(value "$WORK/$way-out.txt" signing-note 5)
  reference_foot=$(value "$WORK/$way-out.txt" panel-foot 3)
  reference_end=$(awk -v y="$reference_foot" -v h="$(value "$WORK/$way-out.txt" panel-foot 5)" 'BEGIN { print y + h }')
  for answer in none $ANSWERS; do
    probe="$WORK/$way-$answer.txt"
    if [ ! -s "$probe" ]; then
      fail "$way/$answer: the layout said nothing"
      continue
    fi
    y=$(value "$probe" signing-confirm 3)
    h=$(value "$probe" signing-confirm 5)
    foot_y=$(value "$probe" panel-foot 3)
    foot_h=$(value "$probe" panel-foot 5)
    place=$(value "$probe" verdict-place 5)
    content=$(value "$probe" verdict-content 5)
    scrolled=$(value "$probe" panel-body 2)
    can=$(value "$probe" panel-body 3)
    bottom=$(awk -v y="$y" -v h="$h" 'BEGIN { print y + h }')
    said=$(value "$probe" signing-note 3)
    kept=$(value "$probe" signing-note-held 3)
    line=none
    [ "$kept" = none ] || line=kept
    [ "$said" = none ] || line=said
    printf '%-13s %-11s %9s %8s %9s %10s %13s %7s\n' "$way" "$answer" "$y" "$bottom" "$place" "$content" "$scrolled/$can" "$line"

    # …wholly inside the column's foot, which ends where the window does.
    column_end=$(awk -v y="$foot_y" -v h="$foot_h" 'BEGIN { print y + h }')
    if more "$bottom" "$column_end"; then
      fail "$way/$answer: the confirm ends at $bottom, under the column's end $column_end"
    fi
    same "$column_end" "$reference_end" || fail "$way/$answer: the foot ends at $column_end, not at $reference_end"
    if [ "$answer" = none ]; then
      # 5. A request nothing simulates waits for no verdict: no line under
      # its confirm, said or kept. Its foot is shorter by exactly that, and
      # holds the confirm the same way.
      [ "$line" = none ] || fail "$way/none: a line under a confirm that waits for no verdict"
      [ "$line_y" != none ] || fail "$way/out: the held confirm says no line (this case measures nothing)"
      same "$(awk -v y="$y" -v f="$foot_y" 'BEGIN { print y - f }')" \
        "$(awk -v y="$reference" -v f="$reference_foot" 'BEGIN { print y - f }')" ||
        fail "$way/none: the confirm does not stand in its foot as it does under an answer"
      more "$y" "$reference" || fail "$way/none: the held confirm's line takes no room (this case measures nothing)"
      continue
    fi

    # 3. One position for the confirm, whatever stands above it…
    same "$y" "$reference" || fail "$way/$answer: the confirm stands at $y, not at $reference"
    # 5. …and whatever the line under it says: said while the answer is
    # out, kept — in the same box, with nothing to say — once one is in or
    # the wait has run out.
    if [ "$answer" = out ]; then
      [ "$line" = said ] || fail "$way/out: the held confirm's line is not said"
    else
      [ "$line" = kept ] || fail "$way/$answer: the confirm's line is $line, not kept in its place"
      same "$kept" "$line_y" || fail "$way/$answer: the line's place is at $kept, not at $line_y"
      same "$(value "$probe" signing-note-held 5)" "$line_h" ||
        fail "$way/$answer: the line's place is not as tall as the line ($line_h)"
    fi

    # 1. Never less than the least room…
    if more "$least" "$place"; then
      fail "$way/$answer: the place is $place tall, under its least $least"
    fi
    # 2. …and never less than what stands in it: nothing is cut.
    if more "$content" "$place"; then
      fail "$way/$answer: the place ($place) is shorter than the verdict ($content)"
    fi
    cut=$(outside "$probe" verdict-place)
    [ -z "$cut" ] || fail "$way/$answer: not wholly inside the place: $cut"
    # A verdict taller than the least room: the place is exactly as tall.
    if more "$content" "$least"; then
      same "$place" "$content" || fail "$way/$answer: the place ($place) is not the verdict's height ($content)"
    fi
    # 4. However tall the sheet, the landed verdict is in the body's view.
    hidden=$(outside "$probe" panel-body)
    [ -z "$hidden" ] || fail "$way/$answer: outside the body's view: $hidden"
  done
done

# The tall verdict is four rows and the warning, each at a row's own height.
for way in design tall-window largest-text; do
  probe="$WORK/$way-tall.txt"
  row=$(value "$WORK/$way-send.txt" verdict-row-0 5)
  for index in 0 1 2 3; do
    got=$(value "$probe" "verdict-row-$index" 5)
    if [ "$got" = none ] || ! same "$got" "$row"; then
      fail "$way/tall: row $index is $got tall, a row is $row"
    fi
  done
  note=$(value "$probe" verdict-note 5)
  if [ "$note" = none ] || ! more "$note" "$row"; then
    fail "$way/tall: the warning is $note tall"
  fi
done

# In the taller window the whole sheet fits: the body does not scroll.
can=$(value "$WORK/tall-window-tall.txt" panel-body 3)
same "$can" 0 || fail "tall-window/tall: the body scrolls by $can in a window the sheet fits"

# At the largest text the sheet does not fit: the body scrolls…
can=$(value "$WORK/largest-text-tall.txt" panel-body 3)
more "$can" 0 || fail "largest-text/tall: the body does not scroll (the sheet fits; this case measures nothing)"
# …and left at its top the verdict really is partly under the fold — which is
# what the bringing into view, checked above, is for…
hidden=$(outside "$WORK/short-top.txt" panel-body)
[ -n "$hidden" ] || fail "short/top: nothing of the verdict is under the fold (this case measures nothing)"
# …every row and the warning can be brought into view…
hidden=$(outside "$WORK/short-bottom.txt" panel-body)
[ -z "$hidden" ] || fail "short/bottom: still outside the body's view: $hidden"
# …and the confirm has not moved, wherever the body stands.
reference=$(value "$WORK/largest-text-out.txt" signing-confirm 3)
for name in short-top short-bottom; do
  y=$(value "$WORK/$name.txt" signing-confirm 3)
  same "$y" "$reference" || fail "$name: the confirm stands at $y, not at $reference"
done

# An answer that lands late: "Checking…" first, the tall verdict after.
[ -s "$WORK/short-lands.early" ] || {
  echo "FAIL: short/lands: the first frame was never measured"
  exit 1
}
before=$(value "$WORK/short-lands.early" signing-confirm 3)
after=$(value "$WORK/short-lands.txt" signing-confirm 3)
rows=$(value "$WORK/short-lands.txt" verdict-row-3 5)
[ "$rows" != none ] || fail "short/lands: the tall verdict never landed"
same "$before" "$after" || fail "short/lands: the confirm moved from $before to $after when the verdict landed"
hidden=$(outside "$WORK/short-lands.txt" panel-body)
[ -z "$hidden" ] || fail "short/lands: landed outside the body's view and left there: $hidden"
printf '%-13s %-11s %9s -> %s (scrolled %s -> %s)\n' largest-text "out>tall" "$before" "$after" \
  "$(value "$WORK/short-lands.early" panel-body 2)" "$(value "$WORK/short-lands.txt" panel-body 2)"

# 5. The wait run out: the verdict's place says "couldn't check" as the
# node that could not check says it — the same block, so the same box.
for way in design tall-window largest-text; do
  same "$(value "$WORK/$way-waited.txt" verdict-content 5)" "$(value "$WORK/$way-caution.txt" verdict-content 5)" ||
    fail "$way/waited: the could-not-check sentence is not drawn as the notice is"
  [ "$(value "$WORK/$way-waited.txt" verdict-note 5)" = none ] ||
    fail "$way/waited: \"Checking…\" still stands in the verdict's place"
done

# 5. The held confirm's three ways out, each measured before and after:
# what the first frame held, what stands there afterwards, and a confirm
# that did not move.
changed() { # <name> <the mark that must be absent before> <and present after>
  local name="$1" early="$WORK/$1.early" late="$WORK/$1.txt"
  if [ ! -s "$early" ] || [ ! -s "$late" ]; then
    fail "$name: the layout said nothing"
    return
  fi
  local before after
  before=$(value "$early" signing-confirm 3)
  after=$(value "$late" signing-confirm 3)
  [ "$(value "$early" "$3" 3)" = none ] || fail "$name: the first frame already had $3 (this case measures nothing)"
  [ "$(value "$early" "$2" 3)" != none ] || fail "$name: the first frame had no $2"
  [ "$(value "$late" "$3" 3)" != none ] || fail "$name: $3 never came"
  same "$before" "$after" || fail "$name: the confirm moved from $before to $after"
  same "$before" "$(value "$WORK/design-out.txt" signing-confirm 3)" ||
    fail "$name: the confirm stands at $before, not where the design size puts it"
  same "$(value "$early" verdict-place 5)" "$(value "$late" verdict-place 5)" ||
    fail "$name: the verdict's place changed height"
  printf '%-13s %-11s %9s -> %s\n' design "$name" "$before" "$after"
}
# The verdict lands: the line said, then kept; the card's row in the place.
changed held-lands signing-note signing-note-held
[ "$(value "$WORK/held-lands.txt" verdict-row-0 5)" != none ] || fail "held-lands: the verdict never landed"
# The wait runs out: the line said, then kept; the caution in the place.
changed held-waits signing-note signing-note-held
same "$(value "$WORK/held-waits.txt" verdict-content 5)" "$(value "$WORK/design-caution.txt" verdict-content 5)" ||
  fail "held-waits: the place does not hold the could-not-check sentence"
# An answer after that: the line kept throughout; the card replaces the caution.
changed late-answer signing-note-held verdict-row-0
[ "$(value "$WORK/late-answer.early" signing-note 3)" = none ] ||
  fail "late-answer: the confirm was held after its wait had run out"

# The design size is the confirm's one position at either text size — with
# no line under it: a line is as tall as its text, and the foot grows upward.
same "$(value "$WORK/design-none.txt" signing-confirm 3)" "$(value "$WORK/largest-text-none.txt" signing-confirm 3)" ||
  fail "the confirm's position depends on the text size"

if [ "$failed" -gt 0 ]; then
  echo
  echo "$failed check(s) failed"
  exit 1
fi
echo
echo "ok: the confirm has one position per window — held, released and waited out — every verdict is whole, and a landed verdict is in view"
