#!/usr/bin/env node
/**
 * Portable-subset gate for Lottie animation sources
 * (spec 012-launch-animation-lottie, contracts/portable-subset.md).
 *
 * Vela renders Lottie with THREE different engines — Airbnb's Core Animation
 * engine on iOS, Airbnb's Canvas renderer on Android, Airbnb's lottie_light
 * DOM/SVG renderer on web, and ThorVG on desktop. They agree on the core of the
 * format and diverge, silently and differently from each other, on its edges.
 * This script enforces the region where they agree.
 *
 * It also enforces the cross-file invariants that only exist because the same
 * animation ships in several framings: identical timing, matching palettes, and
 * — the load-bearing one — that the apps' single geometric constant is still a
 * DERIVATION from the assets rather than a number somebody typed.
 *
 * Usage:  node scripts/lint-lottie-assets.mjs
 *         node scripts/lint-lottie-assets.mjs --self-test
 *
 * `--self-test` runs the script against `scripts/__fixtures__/lottie/`, where
 * every file is named for the violation it must trigger (plus one legal
 * control). A linter with no failing input is not evidence of anything, so CI
 * runs both modes.
 */

import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const DESIGN_DIR = join(REPO_ROOT, 'docs', 'design');
const FIXTURE_DIR = join(REPO_ROOT, 'scripts/__fixtures__/lottie');
const CROSSFILE_DIR = join(REPO_ROOT, 'scripts/__fixtures__/lottie-crossfile');
/** Repo-relative, for the stray-copy guard's exemption. */
const FIXTURE_ROOT = 'scripts/__fixtures__/';
const CONTRACT = 'specs/012-launch-animation-lottie/contracts/portable-subset.md';

/**
 * `vela-wallet-{animation}-{formFactor}-{framing}-{appearance}.json`
 *
 * The animation NAME is a field, not a literal. It was hardcoded to `launch`
 * first, which meant a second animation could not be added without editing this
 * regex — a build-configuration edit in all but name, and exactly what FR-004
 * forbids. Found by actually dropping a second animation in and watching this
 * reject it (T058), not by reading the requirement.
 */
const NAME_RE = /^vela-wallet-([a-z0-9]+(?:-[a-z0-9]+)*?)-(phone|desktop)-(core|full)-(dark|light)\.json$/;

/**
 * Core canvas width ÷ full-bleed canvas width, per form factor — which is
 * exactly the `BOX_W_RATIO` each app multiplies the viewport width by
 * (research D1). Pinned here so a re-crop in After Effects fails CI instead of
 * silently changing how much of the screen the brand lockup occupies.
 *
 * The four apps assert the same two numbers in their own fit-rule unit tests;
 * this is the assertion that keeps them a derivation rather than a transcription.
 */
const EXPECTED_BOX_W_RATIO = { phone: 350 / 390, desktop: 680 / 1920 };

/** Vertical-centring tolerance, in canvas units. */
const CENTRE_TOLERANCE = 0.5;

/** Shape items that are outright rejected, with the reason each is rejected. */
const REJECTED_SHAPE_TYPES = {
  gf: 'gradient fill (ty:"gf") — interpolation and dithering differ between the four engines',
  gs: 'gradient stroke (ty:"gs") — same divergence as gradient fill',
  tm: 'trim path (ty:"tm") — multi-subpath trim ordering is a classic cross-renderer divergence',
  mm: 'merge paths (ty:"mm") — unsupported or approximated by several renderers',
};

/** Layer keys that are outright rejected. */
const REJECTED_LAYER_KEYS = {
  masksProperties: 'mask — mask modes beyond `add` are inconsistently implemented',
  hasMask: 'mask — mask modes beyond `add` are inconsistently implemented',
  tt: 'track matte (tt) — forces lottie-ios off the Core Animation engine onto the main thread',
  td: 'track matte source (td) — forces lottie-ios off the Core Animation engine',
  ef: 'effect (ef) — effectively renderer-specific',
  sk: 'skew (sk) — inconsistently implemented',
  tm: 'time remapping (layer tm) — nested time is a divergence source',
};

const LAYER_TYPE_NAMES = {
  0: 'precomp layer (ty:0) — nested time remapping diverges, and it defeats flat linting',
  1: 'solid layer (ty:1)',
  2: 'image layer (ty:2) — an external resource to bundle and resolve per platform',
  3: 'null layer (ty:3)',
  5: 'text layer (ty:5) — requires font resolution and shaping across four text stacks',
  6: 'audio layer (ty:6)',
};

// ---------------------------------------------------------------------------
// Geometry
//
// Vertices only for the centring measurement (that is what "where the artwork
// sits" means); vertices PLUS absolute bezier control points for the clipping
// check, because a curve can bulge past its own vertices and clipping must be
// judged conservatively.
// ---------------------------------------------------------------------------

const isNum = (v) => typeof v === 'number' && Number.isFinite(v);

/** Keyframed (`a:1`) or static (`a:0`) property → the values it takes. */
function propSamples(prop, fallback) {
  if (!prop || typeof prop !== 'object') return [fallback];
  if (prop.a === 1) {
    const out = [];
    for (const kf of prop.k ?? []) {
      if (Array.isArray(kf.s)) out.push(kf.s);
      else if (isNum(kf.s)) out.push([kf.s]);
      if (Array.isArray(kf.e)) out.push(kf.e);
    }
    return out.length ? out : [fallback];
  }
  const k = prop.k;
  if (Array.isArray(k)) return [k];
  if (isNum(k)) return [[k]];
  // A static path property: `k` is the {i,o,v,c} object itself, not a vector.
  if (k && typeof k === 'object') return [k];
  return [fallback];
}

/**
 * Affine transform from a Lottie transform group: translate(p) · rotate(r) ·
 * scale(s) · translate(-a). Returned as a point mapper.
 *
 * Eased interpolation between keyframes cannot overshoot the sampled extremes:
 * the permitted subset allows only bezier handles with y in [0,1] and hold
 * keyframes, neither of which leaves the endpoint range.
 */
function makeTransform(p, a, s, r) {
  const [px, py] = [p[0] ?? 0, p[1] ?? 0];
  const [ax, ay] = [a[0] ?? 0, a[1] ?? 0];
  const [sx, sy] = [(s[0] ?? 100) / 100, (s[1] ?? 100) / 100];
  const rad = ((r[0] ?? 0) * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  return (x, y) => {
    const dx = (x - ax) * sx;
    const dy = (y - ay) * sy;
    return [px + dx * cos - dy * sin, py + dx * sin + dy * cos];
  };
}

/** Cartesian product of each transform property's samples. */
function transformSamples(ks) {
  const ps = propSamples(ks?.p, [0, 0]);
  const as = propSamples(ks?.a, [0, 0]);
  const ss = propSamples(ks?.s, [100, 100]);
  const rs = propSamples(ks?.r, [0]);
  const out = [];
  for (const p of ps) for (const a of as) for (const s of ss) for (const r of rs) out.push(makeTransform(p, a, s, r));
  return out;
}

function emptyBox() {
  return { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
}

function extend(box, x, y) {
  if (x < box.x0) box.x0 = x;
  if (y < box.y0) box.y0 = y;
  if (x > box.x1) box.x1 = x;
  if (y > box.y1) box.y1 = y;
}

const isEmptyBox = (b) => !Number.isFinite(b.x0);

/**
 * Sweep one layer's shape geometry through every combination of its own and its
 * groups' transform samples. Returns { vertices, outline } boxes.
 */
function layerBoxes(layer) {
  const vertices = emptyBox();
  const outline = emptyBox();

  const walk = (items, mappers) => {
    for (const item of items ?? []) {
      if (item?.ty === 'gr') {
        const tr = (item.it ?? []).find((c) => c?.ty === 'tr');
        const inner = tr ? transformSamples(tr) : [(x, y) => [x, y]];
        const composed = [];
        for (const outer of mappers) {
          for (const t of inner) composed.push((x, y) => { const [ix, iy] = t(x, y); return outer(ix, iy); });
        }
        walk(item.it, composed);
        continue;
      }
      if (item?.ty !== 'sh') continue;
      for (const shape of propSamples(item.ks, null)) {
        // A path value is {i,o,v,c}; propSamples yields it wrapped or raw.
        const path = Array.isArray(shape) ? shape[0] : shape;
        const v = path?.v;
        if (!Array.isArray(v)) continue;
        for (let n = 0; n < v.length; n += 1) {
          const [vx, vy] = v[n];
          const [ix, iy] = path.i?.[n] ?? [0, 0];
          const [ox, oy] = path.o?.[n] ?? [0, 0];
          for (const m of mappers) {
            const [ax, ay] = m(vx, vy);
            extend(vertices, ax, ay);
            extend(outline, ax, ay);
            const [bx, by] = m(vx + ix, vy + iy);
            extend(outline, bx, by);
            const [cx, cy] = m(vx + ox, vy + oy);
            extend(outline, cx, cy);
          }
        }
      }
    }
  };

  walk(layer.shapes, transformSamples(layer.ks));
  return { vertices, outline };
}

/** Union of every layer's boxes for a document. */
function documentBoxes(doc) {
  const vertices = emptyBox();
  const outline = emptyBox();
  for (const layer of doc.layers ?? []) {
    const b = layerBoxes(layer);
    if (!isEmptyBox(b.vertices)) {
      extend(vertices, b.vertices.x0, b.vertices.y0);
      extend(vertices, b.vertices.x1, b.vertices.y1);
    }
    if (!isEmptyBox(b.outline)) {
      extend(outline, b.outline.x0, b.outline.y0);
      extend(outline, b.outline.x1, b.outline.y1);
    }
  }
  return { vertices, outline };
}

// ---------------------------------------------------------------------------
// Feature scan
//
// Three key collisions make a naive recursive key search reject every legal file
// in this repository. Each was found by scanning the real assets, not by reading
// the format spec (research D0/D6):
//
//   1. `x` is an EXPRESSION only as a sibling of `k`/`a` on a property object.
//      Every launch file contains 22 `x` keys and none is an expression — they
//      are the bezier ease handles `o:{x,y}` / `i:{x,y}`. A recursive search for
//      `x` fails all eight files while looking correct, because expressions
//      genuinely are on the rejected list.
//   2. `ao` is present in every legal file as `"ao": 0`. Check the VALUE.
//   3. `tm` and `sr` are overloaded by position: `tm` is a trim path as a shape
//      item but time remapping as a layer property; `sr` is a star as a shape
//      item but time stretch as a layer property. Walk shape-item arrays and
//      layer objects separately — never one recursive pass.
// ---------------------------------------------------------------------------

/** True when `node` is a Lottie property object carrying an expression. */
const hasExpression = (node) =>
  node && typeof node === 'object' && !Array.isArray(node) &&
  typeof node.x === 'string' && ('k' in node || 'a' in node);

function scanExpressions(node, report, path) {
  if (Array.isArray(node)) {
    node.forEach((child, i) => scanExpressions(child, report, `${path}[${i}]`));
    return;
  }
  if (!node || typeof node !== 'object') return;
  if (hasExpression(node)) report(`expression on property ${path} (\`x\` beside \`k\`/\`a\`)`);
  for (const [key, child] of Object.entries(node)) {
    // `i`/`o` on a property hold ease handles whose `x` is a number, not code.
    scanExpressions(child, report, `${path}/${key}`);
  }
}

function scanShapeItems(items, report, depth = 0) {
  for (const item of items ?? []) {
    if (!item || typeof item !== 'object') continue;
    const ty = item.ty;
    if (typeof ty === 'string' && REJECTED_SHAPE_TYPES[ty]) report(REJECTED_SHAPE_TYPES[ty]);
    if (ty === 'st' && item.d) report('dashed stroke (`d` on a stroke) — phase and cap handling differ between Canvas and ThorVG');
    if (ty === 'gr') scanShapeItems(item.it, report, depth + 1);
  }
}

function scanLayer(layer, report) {
  if (layer.ty !== 4) {
    report(LAYER_TYPE_NAMES[layer.ty] ?? `layer type ty:${layer.ty} is not a shape layer`);
  }
  for (const [key, reason] of Object.entries(REJECTED_LAYER_KEYS)) {
    if (key in layer) report(reason);
  }
  if (layer.ao === 1) report('auto-orient (ao:1) — rotation derivation differs between renderers');
  if (layer.ddd === 1) report('3-D layer (ddd:1) — no consistent camera model');
  if ((layer.bm ?? 0) !== 0) report(`blend mode bm:${layer.bm} — only normal (bm:0) composites identically`);
  scanShapeItems(layer.shapes, report);
  scanExpressions(layer.ks, report, 'ks');
  scanExpressions(layer.shapes, report, 'shapes');
}

// ---------------------------------------------------------------------------
// Per-file checks
// ---------------------------------------------------------------------------

const LOTTIE_KEYS = ['v', 'w', 'h', 'fr', 'ip', 'op', 'layers'];
const isLottie = (doc) => doc && typeof doc === 'object' && LOTTIE_KEYS.every((k) => k in doc);

/** Fill colours in document order — the palette fingerprint of a framing. */
function fillColours(doc) {
  const out = [];
  const walk = (items) => {
    for (const item of items ?? []) {
      if (item?.ty === 'gr') { walk(item.it); continue; }
      if (item?.ty !== 'fl') continue;
      const c = item.c?.k;
      if (Array.isArray(c)) {
        out.push(c.slice(0, 3).map((n) => Math.round(n * 255)).join(','));
      }
    }
  };
  for (const layer of doc.layers ?? []) walk(layer.shapes);
  return out;
}

function checkFile(rel, doc, { enforceName = true } = {}) {
  const problems = [];
  const add = (msg) => problems.push(msg);

  const name = basename(rel);
  const match = NAME_RE.exec(name);
  if (!match && enforceName) {
    add('filename does not match vela-wallet-launch-{phone|desktop}-{core|full}-{dark|light}.json');
  }

  for (const key of ['w', 'h', 'fr', 'ip', 'op']) {
    if (!isNum(doc[key])) add(`\`${key}\` is not a finite number`);
  }
  if (isNum(doc.ip) && isNum(doc.op) && doc.op <= doc.ip) add(`\`op\` (${doc.op}) must be greater than \`ip\` (${doc.ip})`);
  if (!Array.isArray(doc.assets) || doc.assets.length > 0) add('`assets` must be present and empty — nothing external may need resolving');
  if (doc.fonts) add('`fonts` is present — the wordmark must be outlines, not text');

  (doc.layers ?? []).forEach((layer, i) => {
    scanLayer(layer, (msg) => add(`layer ${i} "${layer.nm ?? '?'}": ${msg}`));
  });

  // Geometry: only meaningful once the document is structurally sane.
  let geometry = null;
  if (!problems.length && Array.isArray(doc.layers) && doc.layers.length) {
    const { vertices, outline } = documentBoxes(doc);
    if (isEmptyBox(vertices)) {
      add('no path geometry found — the file draws nothing');
    } else {
      const vOffset = (vertices.y0 + vertices.y1) / 2 - doc.h / 2;
      if (Math.abs(vOffset) > CENTRE_TOLERANCE) {
        add(`content is ${vOffset > 0 ? 'below' : 'above'} the canvas centre by ${Math.abs(vOffset).toFixed(2)} units ` +
            `(tolerance ±${CENTRE_TOLERANCE}) — top margin ${(vertices.y0).toFixed(2)}, bottom ${(doc.h - vertices.y1).toFixed(2)}`);
      }
      const eps = 1e-6;
      if (outline.x0 < -eps || outline.y0 < -eps || outline.x1 > doc.w + eps || outline.y1 > doc.h + eps) {
        add(`content is clipped by the canvas: swept outline x ${outline.x0.toFixed(2)}…${outline.x1.toFixed(2)}, ` +
            `y ${outline.y0.toFixed(2)}…${outline.y1.toFixed(2)} against ${doc.w}×${doc.h}`);
      }
      geometry = { vertices, outline };
    }
  }

  return {
    problems,
    meta: match
      ? { animation: match[1], formFactor: match[2], framing: match[3], appearance: match[4] }
      : null,
    geometry,
  };
}

// ---------------------------------------------------------------------------
// Cross-file checks
// ---------------------------------------------------------------------------

function checkCrossFile(files) {
  const problems = [];
  // Deliberately NOT filtered on `problems.length`: these assertions read only
  // `fr`/`ip`/`op`/`w`/`layers`, each validated per-file elsewhere. Skipping a
  // file because it failed some other check would let a broken crop hide the
  // ratio drift it caused — which is exactly what the `ratio` fixture caught.
  const usable = files.filter((f) => f.meta && f.doc);
  if (usable.length < 2) return problems;

  // Group by animation first. Two DIFFERENT animations may legitimately differ
  // in duration, palette and crop — only a single animation's framings must
  // agree with each other.
  const byAnimation = new Map();
  for (const f of usable) {
    if (!byAnimation.has(f.meta.animation)) byAnimation.set(f.meta.animation, []);
    byAnimation.get(f.meta.animation).push(f);
  }

  // 1. One animation in several framings → identical timing across them.
  for (const [animation, group] of byAnimation) {
    const timings = new Map();
    for (const f of group) {
      const key = `${f.doc.fr}/${f.doc.ip}/${f.doc.op}`;
      if (!timings.has(key)) timings.set(key, []);
      timings.get(key).push(f.rel);
    }
    if (timings.size > 1) {
      const groups = [...timings.entries()]
        .map(([k, names]) => `    fr/ip/op ${k}: ${names.map((n) => basename(n)).join(', ')}`)
        .join('\n');
      problems.push(
        `"${animation}": timing disagrees between framings — the form factors would desynchronise:\n${groups}`,
      );
    }
  }

  // 2. Same form factor + appearance → the framings are the same artwork.
  const byPair = new Map();
  for (const f of usable) {
    const key = `${f.meta.animation}/${f.meta.formFactor}/${f.meta.appearance}`;
    if (!byPair.has(key)) byPair.set(key, []);
    byPair.get(key).push(f);
  }
  for (const [key, group] of byPair) {
    if (group.length < 2) continue;
    const [a, ...rest] = group;
    const aColours = fillColours(a.doc).join('|');
    const aNames = (a.doc.layers ?? []).map((l) => l.nm).join('|');
    for (const b of rest) {
      if (fillColours(b.doc).join('|') !== aColours) {
        problems.push(`${key}: fill colours differ between ${basename(a.rel)} and ${basename(b.rel)} — one framing was re-exported without the other`);
      }
      if ((b.doc.layers ?? []).map((l) => l.nm).join('|') !== aNames) {
        problems.push(`${key}: layer names/count differ between ${basename(a.rel)} and ${basename(b.rel)}`);
      }
    }
  }

  // 3. The apps' only geometric constant is a DERIVATION from these files.
  for (const [animation, group] of byAnimation) {
    for (const [formFactor, expected] of Object.entries(EXPECTED_BOX_W_RATIO)) {
    const core = group.find((f) => f.meta.formFactor === formFactor && f.meta.framing === 'core');
    const full = group.find((f) => f.meta.formFactor === formFactor && f.meta.framing === 'full');
    if (!core || !full) continue;
    const actual = core.doc.w / full.doc.w;
    if (Math.abs(actual - expected) > 1e-4) {
      problems.push(
        `"${animation}" ${formFactor}: BOX_W_RATIO drifted — ${core.doc.w}/${full.doc.w} = ${actual.toFixed(5)}, ` +
        `apps use ${expected.toFixed(5)}. A re-crop changed how much of the screen the lockup occupies. ` +
        `Update EXPECTED_BOX_W_RATIO here AND the four apps' launch constants together, or restore the crop.`,
      );
    }
    }
  }

  return problems;
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

function jsonFilesUnder(dir) {
  const out = [];
  const walk = (d) => {
    let entries;
    try { entries = readdirSync(d); } catch { return; }
    for (const entry of entries.sort()) {
      const full = join(d, entry);
      if (statSync(full).isDirectory()) walk(full);
      else if (entry.endsWith('.json')) out.push(full);
    }
  };
  walk(dir);
  return out;
}

function collect(dir) {
  const files = [];
  for (const abs of jsonFilesUnder(dir)) {
    const rel = relative(REPO_ROOT, abs);
    let doc;
    try {
      doc = JSON.parse(readFileSync(abs, 'utf8'));
    } catch (error) {
      // Not every JSON under design/ is a Lottie file; an unparseable one is
      // only this script's business if the name says it is an animation.
      if (NAME_RE.test(basename(rel))) {
        files.push({ rel, doc: null, problems: [`is not valid JSON: ${error.message}`], meta: null });
      }
      continue;
    }
    if (!isLottie(doc)) continue;
    const { problems, meta, geometry } = checkFile(rel, doc);
    files.push({ rel, doc, problems, meta, geometry });
  }
  return files;
}

/**
 * SC-004: `docs/design/onboarding/launch` is the ONLY place an animation may live.
 *
 * Each app receives them at build time — an Xcode file-list phase, a Gradle
 * Sync, a Vite alias, `include_bytes!`. A committed copy under an app would
 * still build, still run, and quietly go stale; nothing else in the repo would
 * notice. Matched by basename against the real assets, so an unrelated
 * `animations/` directory cannot trip it.
 */
function strayCopies(files) {
  const names = new Set(files.map((f) => basename(f.rel)));
  if (!names.size) return [];
  let tracked;
  try {
    tracked = execFileSync('git', ['ls-files'], { cwd: REPO_ROOT, encoding: 'utf8' });
  } catch {
    return []; // not a git checkout — the per-app build gates still apply
  }
  return tracked
    .split('\n')
    .filter(
      (p) => p && names.has(basename(p)) && !p.startsWith('docs/design/') && !p.startsWith(FIXTURE_ROOT),
    )
    .sort();
}

function report(files, crossProblems, label) {
  let failed = 0;
  for (const f of files) {
    if (!f.problems.length) continue;
    failed += 1;
    console.error(f.rel);
    for (const p of f.problems) console.error(`  ${p}`);
  }
  if (crossProblems.length) {
    console.error('cross-file');
    for (const p of crossProblems) console.error(`  ${p}`);
  }
  if (failed || crossProblems.length) {
    console.error(`\nSee ${CONTRACT} for the permitted subset and the reason each rejection exists.`);
    return false;
  }
  console.log(`${label}: ${files.length} animation file${files.length === 1 ? '' : 's'} legal`);
  return true;
}

/**
 * The reason each fixture MUST be rejected for. Without this, a fixture that is
 * broken in some unrelated way still shows green — "was rejected" is a much
 * weaker claim than "was rejected by the check it exists to exercise".
 */
const EXPECTED_REASON = {
  'illegal-gradient-fill.json': 'gradient fill',
  'illegal-expression.json': 'expression on property',
  'illegal-clipped.json': 'clipped by the canvas',
  'illegal-off-centre.json': 'canvas centre',
  'illegal-text-layer.json': 'text layer',
  'illegal-track-matte.json': 'track matte',
  'illegal-auto-orient.json': 'auto-orient',
  'illegal-blend-mode.json': 'blend mode',
  'illegal-trim-path.json': 'trim path',
  'illegal-external-asset.json': '`assets` must be present and empty',
};

/** Same idea for the cross-file sets. */
const EXPECTED_CROSS_REASON = {
  timing: 'timing disagrees between framings',
  colours: 'fill colours differ',
  ratio: 'BOX_W_RATIO drifted',
};

/** Names the convention must accept, and names it must reject. */
const NAME_CASES = [
  ['vela-wallet-launch-phone-core-dark.json', true],
  // The animation name is a FIELD. A second animation must be addable without
  // touching this regex — that is FR-004, and hardcoding `launch` broke it.
  ['vela-wallet-onboarding-phone-core-dark.json', true],
  ['vela-wallet-send-success-desktop-full-light.json', true],
  ['vela-wallet-launch-phone-full-light.json', true],
  ['vela-wallet-launch-desktop-core-dark.json', true],
  ['vela-wallet-launch-desktop-full-light.json', true],
  ['vela-wallet-launch-core-dark.json', false],        // form factor missing
  ['vela-wallet-launch-phone-dark.json', false],       // framing missing
  ['vela-wallet-launch-phone-core.json', false],       // appearance missing
  ['vela-wallet-launch-tablet-core-dark.json', false], // unknown form factor
  ['vela-wallet-launch-phone-core-dark.JSON', false],  // case
];

/**
 * Three layers, because the linter can be inert in three different ways.
 *
 * Per-file fixtures are each ONE mutation away from a real asset, so a rejection
 * can only be attributed to that mutation. `legal-control.json` is an unmodified
 * copy and is the most important case in the file: it carries the 22 bezier ease
 * handles that a naive `x`-key search would report as expressions, so a linter
 * that fails it is broken in the exact way this script exists to prevent.
 *
 * Fixture names deliberately do NOT follow the asset naming convention — that
 * convention governs `design/`, and applying it here would make every fixture
 * fail for the wrong reason. The convention is covered by NAME_CASES instead.
 */
function selfTest() {
  let ok = true;
  const fail = (msg, details = []) => {
    ok = false;
    console.error(`self-test FAIL  ${msg}`);
    for (const d of details) console.error(`    ${d}`);
  };

  for (const [name, shouldMatch] of NAME_CASES) {
    const matched = NAME_RE.test(name);
    if (matched !== shouldMatch) fail(`naming rule ${matched ? 'accepted' : 'rejected'} "${name}" — expected the opposite`);
    else console.log(`self-test ok    naming ${shouldMatch ? 'accepts' : 'rejects'} ${name}`);
  }

  const files = collect(FIXTURE_DIR).map((f) => ({ ...f, problems: checkFile(f.rel, f.doc, { enforceName: false }).problems }));
  if (!files.length) fail(`no per-file fixtures under ${relative(REPO_ROOT, FIXTURE_DIR)}`);
  for (const f of files) {
    const name = basename(f.rel);
    const mustPass = name.startsWith('legal-');
    const passed = f.problems.length === 0;
    if (mustPass && !passed) { fail(`${name} is legal but was rejected:`, f.problems); continue; }
    if (!mustPass && passed) { fail(`${name} is illegal but passed — that check is inert`); continue; }
    if (!mustPass) {
      const want = EXPECTED_REASON[name];
      if (!want) { fail(`${name} has no entry in EXPECTED_REASON — add one so it cannot pass for the wrong reason`); continue; }
      if (!f.problems.some((p) => p.includes(want))) {
        fail(`${name} was rejected, but not for "${want}":`, f.problems);
        continue;
      }
    }
    console.log(`self-test ok    ${name} ${mustPass ? 'passes' : `→ ${f.problems[0].split('\n')[0]}`}`);
  }

  let sets;
  try { sets = readdirSync(CROSSFILE_DIR).sort(); } catch { sets = []; }
  if (!sets.length) fail(`no cross-file fixture sets under ${relative(REPO_ROOT, CROSSFILE_DIR)}`);
  for (const set of sets) {
    const dir = join(CROSSFILE_DIR, set);
    if (!statSync(dir).isDirectory()) continue;
    const group = collect(dir);
    const problems = checkCrossFile(group);
    const want = EXPECTED_CROSS_REASON[set];
    if (!problems.length) fail(`cross-file set "${set}" produced no problem — that check is inert`);
    else if (!want) fail(`cross-file set "${set}" has no entry in EXPECTED_CROSS_REASON`);
    else if (!problems.some((p) => p.includes(want))) fail(`cross-file set "${set}" was rejected, but not for "${want}":`, problems);
    else console.log(`self-test ok    cross-file/${set} → ${problems[0].split('\n')[0]}`);
  }

  return ok;
}

/**
 * Print what the geometry checks actually measured. Exists so "legal" can be
 * confirmed to be a measurement rather than a silent no-op, and so a designer
 * can read the numbers back without opening After Effects.
 */
function printReport() {
  const files = collect(DESIGN_DIR).filter((f) => f.geometry);
  const pad = (s, n) => String(s).padEnd(n);
  console.log(pad('file', 46), pad('canvas', 11), pad('lockup', 18), pad('vOffset', 9), 'lockup/canvasW');
  for (const f of files) {
    const { vertices } = f.geometry;
    const w = vertices.x1 - vertices.x0;
    const h = vertices.y1 - vertices.y0;
    const vOffset = (vertices.y0 + vertices.y1) / 2 - f.doc.h / 2;
    console.log(
      pad(basename(f.rel), 46),
      pad(`${f.doc.w}x${f.doc.h}`, 11),
      pad(`${w.toFixed(2)} x ${h.toFixed(2)}`, 18),
      pad(vOffset.toFixed(2), 9),
      (w / f.doc.w).toFixed(4),
    );
  }
  for (const [formFactor, expected] of Object.entries(EXPECTED_BOX_W_RATIO)) {
    const core = files.find((f) => f.meta?.formFactor === formFactor && f.meta.framing === 'core');
    const full = files.find((f) => f.meta?.formFactor === formFactor && f.meta.framing === 'full');
    if (core && full) {
      console.log(`BOX_W_RATIO ${pad(formFactor, 8)} ${core.doc.w}/${full.doc.w} = ${(core.doc.w / full.doc.w).toFixed(5)} (apps use ${expected.toFixed(5)})`);
    }
  }
  return true;
}

function main() {
  const argv = process.argv;
  const ok = argv.includes('--self-test')
    ? selfTest()
    : argv.includes('--report')
      ? printReport()
      : (() => {
          const files = collect(DESIGN_DIR);
          const problems = checkCrossFile(files);
          for (const stray of strayCopies(files)) {
            problems.push(
              `${stray} is a committed copy of an animation — docs/design/onboarding/launch is the only ` +
                'source (FR-001/SC-004). Every app receives these at build time; a copy goes stale silently.',
            );
          }
          return report(files, problems, 'docs/design/');
        })();
  process.exit(ok ? 0 : 1);
}

// Importable for probing/tests; only self-executing when run directly.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();

export { NAME_RE, checkFile, checkCrossFile, collect, documentBoxes, EXPECTED_BOX_W_RATIO };
