// Nimiq-style identicons, computed locally from an address.
//
// Why this exists at all: everything a requester says ABOUT an address — a
// contact name, a token label — is metadata it can lie about. The address
// itself cannot be, and neither can a picture derived from it. So the sheet
// shows the address and this avatar, and refuses to show a name it was handed.
//
// This is the same algorithm as `identicons-esm@1.0.1`, which vela-core also
// ports (`rust/crates/vela-core/src/identicon.rs`). Two surfaces drawing the
// same account differently would break the recognition signal, so
// samples/identicon-test.mjs asserts byte equality against the core's wasm.
//
// Three details are load-bearing and must not be "tidied":
//   1. the multiply order `(1 - a) * a * K` — reassociating changes the double,
//      and a different double is a different avatar;
//   2. the hash is the DECIMAL STRING of a float, sliced as text;
//   3. digit 5 is read twice (face and top), and `main` is nudged once while
//      `accent` is nudged in a loop. Both are upstream quirks, kept on purpose.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var CHAOS_K = 3.569956786876;
  var CHAOS_ITERATIONS = 100;
  var SECTION_COUNT = 21;
  var SEED_MAX_UTF16_LEN = 128;

  var COLORS = [
    '#FC8702', '#D94432', '#E9B213', '#1A5493', '#0582CA',
    '#5961A8', '#21BCA5', '#FA7268', '#88B04B', '#795548',
  ];
  // Differs from COLORS at indices 3 and 5.
  var BACKGROUND_COLORS = [
    '#FC8702', '#D94432', '#E9B213', '#1F2348', '#0582CA',
    '#5F4B8B', '#21BCA5', '#FA7268', '#88B04B', '#795548',
  ];

  var CIRCLE_PREFIX = '<circle cx="80" cy="80" r="40" fill="';
  var CIRCLE_SUFFIX = '"/>';
  var DEFAULT_SHADOW = '<path fill="#010101" d="M119.21 80a39.46 39.46 0 0 1-67.13 28.13c10.36 2.33 36 3 49.82-14.28 10.39-12.47 8.31-33.23 4.16-43.26A39.35 39.35 0 0 1 119.21 80" opacity=".1"/>';

  var CIRCULAR_HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160"><path fill="';
  var CIRCULAR_AFTER_BACKGROUND = '" d="M0 0h160v160H0z"/><g fill="';
  var CIRCULAR_AFTER_ACCENT = '" color="';
  var SVG_OPEN_GROUP = '">';
  var SVG_TAIL = '</g></svg>';

  var chaosCache = new Map();

  function chaosHash(number) {
    var a = 1 / number;
    for (var i = 0; i < CHAOS_ITERATIONS; i++) {
      a = (1 - a) * a * CHAOS_K; // order is contract, not style
    }
    return a;
  }

  // Upstream feeds `charCodeAt(0) + 3`, not the code unit itself. Dropping the
  // +3 produces a perfectly plausible-looking avatar for every address, and a
  // different one from the wallet's.
  function chaosForCodeUnit(code) {
    if (!chaosCache.has(code)) chaosCache.set(code, chaosHash(code + 3));
    return chaosCache.get(code);
  }

  /**
   * `makeHash` — the chaotic float, rendered by Number#toString(10) and then
   * chopped up AS A STRING.
   */
  function makeHash(seed) {
    var acc = 0.5;
    // Iterating with for..of yields code POINTS; charCodeAt(0) then gives the
    // leading code unit, which is what the upstream reads.
    var chars = Array.from(seed);
    for (var i = 0; i < chars.length; i++) {
      acc = acc * (1 - acc) * chaosForCodeUnit(chars[i].charCodeAt(0));
    }

    var full = String(acc);
    var reversed = full.split('').reverse().join('');
    var pad = reversed.charAt(5) || '0';
    return reversed.replace('.', pad).slice(4, 21).padEnd(13, pad);
  }

  function digitAt(hash, index) {
    var ch = hash.charAt(index);
    return ch >= '0' && ch <= '9' ? Number(ch) : null;
  }

  function pairAt(hash, index) {
    var hi = digitAt(hash, index);
    var lo = digitAt(hash, index + 1);
    return hi === null || lo === null ? null : hi * 10 + lo;
  }

  function sectionSvg(section, index) {
    var table = ns.identiconFeatures[section];
    var n = Math.abs(index % SECTION_COUNT) + 1;
    var svg = table[n];
    if (!svg) throw new Error('no artwork for ' + section + ' at ' + index);
    return svg;
  }

  // `main` is adjusted once; `accent` in a loop. Upstream asymmetry, preserved.
  function resolveColors(main, background, accent) {
    if (main !== null && main === background) main = (main + 1) % 10;
    for (var i = 0; i < 10; i++) {
      if (accent === main || accent === background) accent = (accent + 1) % 10;
      else break;
    }
    return {
      main: main === null ? 'undefined' : COLORS[main % 10],
      background: BACKGROUND_COLORS[background % 10],
      accent: COLORS[accent % 10],
    };
  }

  function params(seed) {
    var hash = makeHash(seed);
    var face = pairAt(hash, 3);
    var top = pairAt(hash, 5);      // digit 5 read twice — upstream quirk
    var sides = pairAt(hash, 7);
    var bottom = pairAt(hash, 9);
    var background = digitAt(hash, 2);
    var accent = digitAt(hash, 11);
    var main = digitAt(hash, 0);
    if (face === null || top === null || sides === null || bottom === null ||
      background === null || accent === null || main === null) {
      throw new Error('seed "' + seed + '" produces an unrenderable hash (' + hash + ')');
    }
    return {
      sections: {
        face: sectionSvg('face', face),
        top: sectionSvg('top', top),
        sides: sectionSvg('sides', sides),
        bottom: sectionSvg('bottom', bottom),
      },
      colors: resolveColors(main % 10, background % 10, accent % 10),
    };
  }

  /**
   * The wallet's variant: circular, and with no SVG `id` anywhere — the stock
   * output hardcodes `clipPath id="a"`, so several on one page would all clip
   * to whichever came first.
   */
  function svgCircular(seed) {
    var p = params(seed);
    var c = p.colors;
    return CIRCULAR_HEAD + c.background + CIRCULAR_AFTER_BACKGROUND + c.accent +
      CIRCULAR_AFTER_ACCENT + c.main + SVG_OPEN_GROUP +
      CIRCLE_PREFIX + c.main + CIRCLE_SUFFIX + DEFAULT_SHADOW +
      p.sections.top + p.sections.sides + p.sections.face + p.sections.bottom +
      SVG_TAIL;
  }

  function normalizeSeed(seed) {
    var lowered = String(seed).toLowerCase();
    return Array.from(lowered).slice(0, SEED_MAX_UTF16_LEN).join('');
  }

  /** An address's avatar. Lower-cased so checksum casing cannot change it. */
  function forAddress(address) {
    return svgCircular(normalizeSeed(address));
  }

  ns.identicon = {
    makeHash: makeHash,
    params: params,
    svgCircular: svgCircular,
    normalizeSeed: normalizeSeed,
    forAddress: forAddress,
    COLORS: COLORS,
    BACKGROUND_COLORS: BACKGROUND_COLORS,
  };
})(window.VelaCS);
