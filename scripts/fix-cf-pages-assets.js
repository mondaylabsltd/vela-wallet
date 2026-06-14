#!/usr/bin/env node
/**
 * Cloudflare Pages drops any directory named `node_modules` during
 * `wrangler pages deploy`. Expo emits vendored assets (e.g. the Plus Jakarta
 * fonts from @expo-google-fonts) under `dist/assets/node_modules/...`, so those
 * files never reach the server — requests fall back to index.html and the
 * browser receives HTML where it expects a .ttf ("OTS parsing error: invalid
 * sfntVersion"). On web that hangs useFonts() and the app spins forever.
 *
 * Fix: after `expo export`, relocate the vendored asset tree to a deploy-safe
 * path (`assets/vendor/...`) and rewrite every reference to it in the build.
 *
 * Wired into `build:web` so every deploy is correct. Idempotent.
 */
const fs = require('fs');
const path = require('path');

const DIST = path.resolve(__dirname, '..', 'dist');
const FROM_DIR = path.join(DIST, 'assets', 'node_modules');
const TO_DIR = path.join(DIST, 'assets', 'vendor');
const FROM_REF = 'assets/node_modules/';
const TO_REF = 'assets/vendor/';
const REWRITE_EXT = new Set(['.js', '.html', '.css', '.json']);

function fail(msg) {
  console.error(`[fix-cf-pages-assets] ${msg}`);
  process.exit(1);
}

if (!fs.existsSync(DIST)) fail(`dist not found at ${DIST} — run "expo export" first.`);

// 1) Relocate assets/node_modules -> assets/vendor (merge if vendor exists).
if (fs.existsSync(FROM_DIR)) {
  if (!fs.existsSync(TO_DIR)) {
    fs.renameSync(FROM_DIR, TO_DIR);
  } else {
    fs.cpSync(FROM_DIR, TO_DIR, { recursive: true });
    fs.rmSync(FROM_DIR, { recursive: true, force: true });
  }
  console.log(`[fix-cf-pages-assets] moved assets/node_modules -> assets/vendor`);
} else {
  console.log(`[fix-cf-pages-assets] no assets/node_modules dir (already fixed?)`);
}

// 2) Rewrite all references in text assets.
let filesChanged = 0;
let refsChanged = 0;
function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full);
    } else if (REWRITE_EXT.has(path.extname(entry.name))) {
      const before = fs.readFileSync(full, 'utf8');
      if (!before.includes(FROM_REF)) continue;
      const occurrences = before.split(FROM_REF).length - 1;
      fs.writeFileSync(full, before.split(FROM_REF).join(TO_REF));
      filesChanged++;
      refsChanged += occurrences;
    }
  }
}
walk(DIST);
console.log(`[fix-cf-pages-assets] rewrote ${refsChanged} reference(s) in ${filesChanged} file(s)`);

// 3) Verify nothing still points at the dropped path.
let stale = 0;
(function verify(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) verify(full);
    else if (REWRITE_EXT.has(path.extname(entry.name)) && fs.readFileSync(full, 'utf8').includes(FROM_REF)) {
      console.error(`[fix-cf-pages-assets] STALE reference still in ${path.relative(DIST, full)}`);
      stale++;
    }
  }
})(DIST);
if (stale) fail(`${stale} file(s) still reference ${FROM_REF}`);

console.log('[fix-cf-pages-assets] OK — vendored assets are deploy-safe.');
