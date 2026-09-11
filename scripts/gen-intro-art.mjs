#!/usr/bin/env node
/**
 * The three intro illustrations, as geometry (spec 020-intro-carousel).
 *
 * Source   <-  docs/design/onboarding-new-2/*.png   the founder's boards, MEASURED
 * Output   ->  specs/020-intro-carousel/contracts/intro-illustrations.json
 *
 * Why a generator and not three hand-written SVG files: the four apps cannot
 * share an SVG. Web inlines `<svg>`, Android builds an `ImageVector`, iOS hands
 * a document to vela-core's rasterizer, and the desktop tints a template and
 * runs it through resvg — the same split the lucide corpus already lives with
 * (specs/015-wallet-home-ui/contracts/icons.json). What they CAN share is one
 * list of `d` strings with a role per element, so a change to the artwork is a
 * change to this file and four mechanical re-ports, never four drawings that
 * drift.
 *
 * Every number below was read off the 392x852 boards with a pixel probe, not
 * eyeballed — the fingerprint's ridge gaps included. The boards are LIGHT mode;
 * the palette is expressed as roles (`line`/`accent`) so dark mode is a token
 * swap rather than a second drawing.
 *
 * Usage:  node scripts/gen-intro-art.mjs          write the contract
 *         node scripts/gen-intro-art.mjs --check  fail if it would change
 *         node scripts/gen-intro-art.mjs --preview <dir>   emit a preview .html
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(REPO_ROOT, 'specs/020-intro-carousel/contracts/intro-illustrations.json');

/**
 * One canvas for all three, so the slide never resizes as it pages. 160x128 is
 * the union of what the boards draw: the widest is the document + fingerprint
 * at 159x125, the tallest the key at 125.
 */
const VIEW_W = 160;
const VIEW_H = 128;

// --- path helpers ----------------------------------------------------------

const n = (v) => {
  const r = Math.round(v * 100) / 100;
  return Object.is(r, -0) ? 0 : r;
};
const pt = (x, y) => `${n(x)} ${n(y)}`;
const rad = (deg) => (deg * Math.PI) / 180;

/** Point on an ellipse at `deg`, screen convention: 0 = right, 90 = down. */
const onEllipse = (cx, cy, rx, ry, deg) => [cx + rx * Math.cos(rad(deg)), cy + ry * Math.sin(rad(deg))];

/**
 * Elliptical arc from `a1` to `a2`, sweeping in the direction of increasing
 * angle (clockwise on screen). A sweep of 360 is emitted as two half arcs,
 * because `A` cannot express a closed ellipse.
 */
function arc(cx, cy, rx, ry, a1, a2) {
  let sweep = ((a2 - a1) % 360 + 360) % 360;
  if (sweep === 0) sweep = 360;
  if (sweep >= 360) {
    const [sx, sy] = onEllipse(cx, cy, rx, ry, a1);
    const [mx, my] = onEllipse(cx, cy, rx, ry, a1 + 180);
    return `M${pt(sx, sy)}A${n(rx)} ${n(ry)} 0 0 1 ${pt(mx, my)}A${n(rx)} ${n(ry)} 0 0 1 ${pt(sx, sy)}`;
  }
  const [sx, sy] = onEllipse(cx, cy, rx, ry, a1);
  const [ex, ey] = onEllipse(cx, cy, rx, ry, a2);
  return `M${pt(sx, sy)}A${n(rx)} ${n(ry)} 0 ${sweep > 180 ? 1 : 0} 1 ${pt(ex, ey)}`;
}

const circle = (cx, cy, r) => arc(cx, cy, r, r, 180, 540);

/** Round-cornered rectangle, clockwise from the top-left corner's end. */
function roundedRect(x, y, w, h, r) {
  return (
    `M${pt(x + r, y)}H${n(x + w - r)}A${n(r)} ${n(r)} 0 0 1 ${pt(x + w, y + r)}` +
    `V${n(y + h - r)}A${n(r)} ${n(r)} 0 0 1 ${pt(x + w - r, y + h)}` +
    `H${n(x + r)}A${n(r)} ${n(r)} 0 0 1 ${pt(x, y + h - r)}` +
    `V${n(y + r)}A${n(r)} ${n(r)} 0 0 1 ${pt(x + r, y)}Z`
  );
}

const line = (x1, y1, x2, y2) => `M${pt(x1, y1)}L${pt(x2, y2)}`;
const dot = (cx, cy, r) => circle(cx, cy, r);
const poly = (points) => `M${points.map(([x, y]) => pt(x, y)).join('L')}Z`;

// --- 1 · no seed phrase: a form, and the finger that replaces it -----------
//
// Board origin (114.5, 331). The document's twelve rules fade top to bottom —
// the ladder is 0.7 down to 0.2, read straight off the board — so the page
// reads as something you were going to have to fill in and now will not.

const DOC = { x: 1, y: 3, w: 104.5, h: 122, r: 10 };
/** Row baseline y, then the two columns' widths. Measured, not patterned. */
const DOC_ROWS = [
  [25, 34, 28],
  [42, 38, 24],
  [59, 30, 32],
  [76, 36, 26],
  [93, 32, 22],
  [110, 28, 30],
];
const DOC_ROW_OPACITY = [0.7, 0.6, 0.5, 0.4, 0.3, 0.2];
const DOC_COL_X = [16, 58];
/**
 * Round caps overhang by half the stroke; the board's extents include them.
 * 2.5 rather than the 3 the extents suggest — a 3 covers three whole pixel rows
 * at this scale and the board's rules have one dark row between two lighter
 * ones, which is a 2.5 landing on a half pixel.
 */
const ROW_STROKE = 2.5;

const FP = { cx: 125.5, cy: 83 };
/**
 * Five ridges and their openings. Each `[rx, ry, ...[from, to]]` — the gaps all
 * sit in the upper-left quadrant, which is what makes it read as a fingerprint
 * rather than a target.
 */
const FP_RIDGES = [
  [6.5, 8.2, [228, 150]],
  [13, 16.4, [222, 186]],
  [19.5, 24.6, [225, 189]],
  [26, 32.8, [237, 132], [150, 222]],
  [32.5, 41, [231, 183]],
];

function noSeedPhrase() {
  const rows = DOC_ROWS.flatMap(([y, w1, w2], i) => {
    const inset = ROW_STROKE / 2;
    const d = [
      line(DOC_COL_X[0] + inset, y, DOC_COL_X[0] + w1 - inset, y),
      line(DOC_COL_X[1] + inset, y, DOC_COL_X[1] + w2 - inset, y),
    ].join('');
    return [{ role: 'line', mode: 'stroke', width: ROW_STROKE, cap: 'round', opacity: DOC_ROW_OPACITY[i], d }];
  });

  return {
    id: 'no-seed-phrase',
    note: 'A form whose rules fade out, and the fingerprint that replaces it.',
    elements: [
      {
        role: 'line',
        mode: 'stroke',
        width: 1.5,
        cap: 'round',
        opacity: 0.85,
        d: roundedRect(DOC.x, DOC.y, DOC.w, DOC.h, DOC.r),
      },
      ...rows,
      {
        role: 'accent',
        mode: 'stroke',
        width: 2,
        cap: 'round',
        opacity: 1,
        d: FP_RIDGES.flatMap(([rx, ry, ...gaps]) =>
          gaps.map(([a1, a2]) => arc(FP.cx, FP.cy, rx, ry, a1, a2)),
        ).join(''),
      },
      { role: 'accent', mode: 'fill', opacity: 1, d: dot(FP.cx, FP.cy, 1.6) },
    ],
  };
}

// --- 2 · the keys are yours ------------------------------------------------
//
// Board origin (116.5, 337). One ring, and three keys hanging off it: yours in
// the accent, two more in the same hand behind it. Three because the account is
// a set of passkeys, not one — the multi-key onboarding is the product, and an
// illustration with a single key would contradict the screen after it.

const RING = { cx: 79, cy: 33.5, r: 30.5 };
/**
 * The key glyph in its own frame: the bow at the origin, the blade running down
 * +Y, and the teeth on the blade's left as it points away from you. `teeth` is
 * `[distance along the blade, length]`.
 */
const KEY = { bow: 10, collar: 3.5, blade: 62, teeth: [[48, 8], [57, 10]] };
/** Where each key hangs, in degrees around the ring (90 = straight down). */
const KEY_ANGLES = [90, 45, 135];

/**
 * A key laid out along `deg`. Teeth sit on the blade's `(y, -x)` side — one
 * rule, so the two flanking keys mirror the middle one instead of being drawn
 * twice.
 */
function keyPaths(deg, scale) {
  const [ox, oy] = onEllipse(RING.cx, RING.cy, RING.r, RING.r, deg);
  const ax = Math.cos(rad(deg));
  const ay = Math.sin(rad(deg));
  const [tx, ty] = [ay, -ax];
  const along = (d) => [ox + ax * d * scale, oy + ay * d * scale];
  const blade = along(KEY.blade);
  // The blade starts on the bow's centreline, not at its middle: the bow is a
  // ring with a HOLE, and what belongs in that hole is the circle behind it.
  // Starting at 0 drew a chord across every bow (first render, 2026-09-01).
  const root = [ox + ax * KEY.bow, oy + ay * KEY.bow];
  return {
    // Full radius on every key. Only the blade shortens on the two behind —
    // the board draws three bows of one size and two shorter blades.
    bow: circle(ox, oy, KEY.bow),
    // The collar inside the bow. Only the front key has one; on the two behind
    // it would be four pixels of detail nobody reads.
    collar: circle(ox, oy, KEY.collar),
    blade: line(root[0], root[1], blade[0], blade[1]),
    teeth: KEY.teeth
      .map(([at, len]) => {
        const [px, py] = along(at);
        return line(px, py, px + tx * len * scale, py + ty * len * scale);
      })
      .join(''),
  };
}

function keysAreYours() {
  const mine = keyPaths(KEY_ANGLES[0], 1);
  const others = KEY_ANGLES.slice(1).map((deg) => keyPaths(deg, 0.75));
  return {
    id: 'keys-are-yours',
    note: 'One ring, three keys — yours in the accent, the others still yours.',
    elements: [
      { role: 'line', mode: 'stroke', width: 2, cap: 'round', opacity: 1, d: circle(RING.cx, RING.cy, RING.r) },
      {
        role: 'line',
        mode: 'stroke',
        width: 1.5,
        cap: 'round',
        opacity: 0.45,
        d: others.map((k) => k.bow + k.blade + k.teeth).join(''),
      },
      // Blade before bow: the bow is a 4-wide ring and has to close over the
      // blade's start, or the join shows through its hole.
      {
        role: 'accent',
        mode: 'stroke',
        width: 3.5,
        cap: 'round',
        opacity: 1,
        d: mine.blade + mine.teeth,
      },
      { role: 'accent', mode: 'stroke', width: 4, cap: 'round', opacity: 1, d: mine.bow },
      { role: 'accent', mode: 'stroke', width: 3, cap: 'round', opacity: 1, d: mine.collar },
    ],
  };
}

// --- 3 · one address, every chain ------------------------------------------
//
// Board origin (115.5, 310.5). A compass: one bearing that holds wherever you
// point it. The rose's dots are heavier at the cardinals and lighten toward the
// top, where the N sits in place of the twelfth.

const ROSE = { cx: 80, cy: 64, r: 60.5, ring: 49.5 };
/** `[degrees, radius, opacity]` — 270 is the N, and has no dot. */
const ROSE_DOTS = [
  [0, 3, 1],
  [30, 2.5, 0.6],
  [60, 2.5, 0.6],
  [90, 3, 1],
  [120, 2.5, 0.6],
  [150, 2.5, 0.6],
  [180, 3, 1],
  [210, 2, 0.55],
  [240, 1.75, 0.55],
  [300, 1.75, 0.55],
  [330, 2, 0.55],
];
/** The N, stroked rather than set: four platforms, no shared font metrics. */
const N_GLYPH = { x: 75.5, y: 10.5, w: 9, h: 13 };
/** The needle's bearing — off true north, so it reads as live rather than printed. */
const NEEDLE = { deg: -66, north: 42.1, south: 41.4, half: 10, hub: 4.5 };

function oneAddress() {
  const ax = Math.cos(rad(NEEDLE.deg));
  const ay = Math.sin(rad(NEEDLE.deg));
  const hub = [ROSE.cx, ROSE.cy + 0.5];
  const wing = (s) => [hub[0] + ay * NEEDLE.half * s, hub[1] - ax * NEEDLE.half * s];
  const tip = (d) => [hub[0] + ax * d, hub[1] + ay * d];

  const minor = ROSE_DOTS.filter(([, , o]) => o < 1);
  const cardinal = ROSE_DOTS.filter(([, , o]) => o === 1);
  const dotsFor = (list) =>
    list
      .map(([deg, r]) => {
        const [x, y] = onEllipse(ROSE.cx, ROSE.cy, ROSE.ring, ROSE.ring, deg);
        return dot(x, y, r);
      })
      .join('');

  return {
    id: 'one-address',
    note: 'A compass: one bearing, and it holds on every chain.',
    elements: [
      { role: 'line', mode: 'stroke', width: 1.5, cap: 'round', opacity: 0.85, d: circle(ROSE.cx, ROSE.cy, ROSE.r) },
      {
        role: 'line',
        mode: 'stroke',
        width: 1.8,
        cap: 'round',
        opacity: 0.85,
        d:
          `M${pt(N_GLYPH.x, N_GLYPH.y + N_GLYPH.h)}L${pt(N_GLYPH.x, N_GLYPH.y)}` +
          `L${pt(N_GLYPH.x + N_GLYPH.w, N_GLYPH.y + N_GLYPH.h)}L${pt(N_GLYPH.x + N_GLYPH.w, N_GLYPH.y)}`,
      },
      { role: 'line', mode: 'fill', opacity: 1, d: dotsFor(cardinal) },
      { role: 'line', mode: 'fill', opacity: 0.55, d: dotsFor(minor) },
      { role: 'accent', mode: 'fill', opacity: 1, d: poly([wing(1), tip(NEEDLE.north), wing(-1)]) },
      // The southern half is an outline over the page, not a second fill: a
      // solid one would read as a second needle rather than the same one. It is
      // drawn AFTER the northern half, because the two share their base corners
      // and the board shows the outline winning there.
      {
        role: 'line',
        mode: 'outline',
        width: 1.2,
        cap: 'round',
        opacity: 0.8,
        d: poly([wing(1), tip(-NEEDLE.south), wing(-1)]),
      },
      { role: 'accent', mode: 'outline', width: 2, cap: 'round', opacity: 1, d: circle(hub[0], hub[1], NEEDLE.hub) },
    ],
  };
}

// --- emit ------------------------------------------------------------------

const doc = {
  _meta: {
    source: 'docs/design/onboarding-new-2/*.png (392x852 light boards), measured with a pixel probe',
    generator: 'scripts/gen-intro-art.mjs — edit the generator, never this file',
    viewBox: `0 0 ${VIEW_W} ${VIEW_H}`,
    roles: {
      line: 'color.fg.subtle — the drawing',
      accent: 'color.accent.base — the one thing the slide is about',
    },
    modes: {
      stroke: 'stroke the path in the role colour; no fill',
      fill: 'fill the path in the role colour; no stroke',
      outline: 'fill with color.bg.base, then stroke in the role colour — it sits OVER the drawing',
    },
    consumers: 'web inline <svg> · android ImageVector · ios vela-core rasterizer · desktop resvg',
  },
  viewBox: { width: VIEW_W, height: VIEW_H },
  illustrations: [noSeedPhrase(), keysAreYours(), oneAddress()],
};

const json = JSON.stringify(doc, null, 2) + '\n';

if (process.argv.includes('--check')) {
  const current = readFileSync(OUT, 'utf8');
  if (current !== json) {
    console.error('gen-intro-art: specs/020-intro-carousel/contracts/intro-illustrations.json is stale.');
    process.exit(1);
  }
  console.log('gen-intro-art: contract up to date.');
} else {
  const previewFlag = process.argv.indexOf('--preview');
  if (previewFlag !== -1) {
    const dir = process.argv[previewFlag + 1];
    const svg = (ill) =>
      `<svg viewBox="0 0 ${VIEW_W} ${VIEW_H}" width="${VIEW_W}" height="${VIEW_H}">` +
      ill.elements
        .map((el) => {
          const color = el.role === 'accent' ? '#E8572A' : '#8C887E';
          const common = `d="${el.d}" opacity="${el.opacity}"`;
          if (el.mode === 'fill') return `<path ${common} fill="${color}"/>`;
          const strokeAttrs = `stroke="${color}" stroke-width="${el.width}" stroke-linecap="${el.cap}" stroke-linejoin="round"`;
          const fill = el.mode === 'outline' ? '#FAFAF8' : 'none';
          return `<path ${common} fill="${fill}" ${strokeAttrs}/>`;
        })
        .join('') +
      '</svg>';
    writeFileSync(
      join(dir, 'intro-art.html'),
      `<!doctype html><meta charset="utf-8"><body style="margin:0;background:#FAFAF8;display:flex">` +
        doc.illustrations.map(svg).join('') +
        `</body>`,
    );
    console.log(`gen-intro-art: preview -> ${join(dir, 'intro-art.html')}`);
  }
  writeFileSync(OUT, json);
  console.log(`gen-intro-art: ${doc.illustrations.length} illustrations -> ${OUT}`);
}
