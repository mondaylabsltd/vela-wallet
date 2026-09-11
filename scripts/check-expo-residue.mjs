#!/usr/bin/env node
/**
 * Expo residue check (spec 039, contract: specs/039-retire-expo-tree/contracts/expo-residue-check.md).
 *
 * The React Native / Expo tree was retired in spec 039. This is the committed
 * form of the spec's "clean tree" test 3: it fails CI when Expo comes back in
 * a form that would mislead someone — a deleted path reappearing, an Expo or
 * React Native dependency in any package.json, an Expo term in an executable
 * CI step or npm script, or a COMMAND that no longer exists quoted in a living
 * document. Prose mentions ("ported from the Expo executors", a history
 * banner) are reported with a count and never fail, because the history is
 * real and the words are the honest way to record it.
 *
 * Rules (all over `git ls-files`, never the working tree's untracked files):
 *   1. deleted paths — src/ e2e/ modules/ plugins/ targets/ .eas/
 *      packages/safari-extension/ and the root files of the Expo toolchain
 *   2. dependencies   — any package.json dependency matching the Expo/RN family
 *   3. executable config — Expo terms on a non-comment line of a workflow, or
 *      in a package.json "scripts" value
 *   4. dead commands — `npx expo …`, `expo start|export|run|prebuild|lint|install`,
 *      `eas build|submit|credentials|update`, `npm run web|build:web|test:e2e|
 *      test:e2e:headed|test:live|typecheck`, `metro … start|bundle` — in ANY
 *      tracked text file outside specs/, unless the document opens (first 20
 *      lines) with a dated history banner: `> **History (YYYY-MM-DD).**` or
 *      `> **勘误（YYYY-MM-DD…`. A banner'd document may quote its old
 *      commands in full; a living document may not.
 *
 * Usage:  node scripts/check-expo-residue.mjs
 *         node scripts/check-expo-residue.mjs --self-test
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const SELF = 'scripts/check-expo-residue.mjs';
const FIXTURES = 'scripts/__fixtures__/expo-residue';

// ---------------------------------------------------------------- rules --

const DELETED_DIRS = ['src/', 'e2e/', 'modules/', 'plugins/', 'targets/', '.eas/', 'packages/safari-extension/'];
const DELETED_FILES = new Set([
  'app.json', 'app.config.js', 'eas.json', 'metro.config.js', 'index.js', 'index.web.js',
  'expo-env.d.ts', 'fingerprint.config.js', '.fingerprintignore', 'jest.config.js', 'jest.setup.js',
  'playwright.config.ts', 'tsconfig.json', 'eslint.config.js', 'keystore.properties.example',
  'scripts/reset-project.js', 'scripts/fix-cf-pages-assets.js', 'scripts/jest-skipped-reporter.js',
  '.verify-rescan.mjs', '.verify-rescan-ja.mjs',
]);

const DEP_RE = /^(expo|@expo\/|expo-|react-native|@react-native|@react-navigation|eslint-config-expo|@bacons\/apple-targets|lucide-react-native)/;
const TERM_RE = /\bexpo\b|react-native|react native|\bmetro\b|\beas\b|\bhermes\b/i;
const DEAD_CMD_RE = /(npx\s+expo\b|\bexpo\s+(start|export|run|prebuild|lint|install)\b|\beas\s+(build|submit|credentials|update)\b|npm\s+run\s+(web|build:web|test:e2e|test:e2e:headed|test:live|typecheck)\b|\bmetro\b.*\b(start|bundle)\b)/i;
const BANNER_RE = /^>\s*\*\*(History\s*\(\d{4}-\d{2}-\d{2}\)|勘误\s*[（(]\d{4}-\d{2}-\d{2})/;
const BANNER_WINDOW = 20;

// Paths rule 4 and the report leave alone: the specs are design records and
// quote the old commands on purpose; this script and its fixtures contain
// every pattern by construction; lockfiles are machine output.
const EXEMPT_RE = /^(specs\/|scripts\/check-expo-residue\.mjs$|scripts\/__fixtures__\/expo-residue\/|.*package-lock\.json$|.*pnpm-lock\.yaml$|.*bun\.lock$|.*Cargo\.lock$)/;

function rule1(paths) {
  return paths.filter((p) => DELETED_FILES.has(p) || DELETED_DIRS.some((d) => p.startsWith(d)));
}

function rule2(paths, read) {
  const out = [];
  for (const p of paths) {
    if (!/(^|\/)package\.json$/.test(p)) continue;
    let pkg;
    try { pkg = JSON.parse(read(p)); } catch { continue; }
    for (const field of ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies']) {
      for (const name of Object.keys(pkg[field] ?? {})) {
        if (DEP_RE.test(name)) out.push(`${p}: ${field}.${name}`);
      }
    }
  }
  return out;
}

function rule3(paths, read) {
  const out = [];
  for (const p of paths) {
    if (/^\.github\/workflows\/[^/]+\.ya?ml$/.test(p)) {
      read(p).split('\n').forEach((line, i) => {
        if (/^\s*#/.test(line)) return;
        if (TERM_RE.test(line)) out.push(`${p}:${i + 1}: ${line.trim()}`);
      });
    } else if (/(^|\/)package\.json$/.test(p) && !EXEMPT_RE.test(p)) {
      let pkg;
      try { pkg = JSON.parse(read(p)); } catch { continue; }
      for (const [name, cmd] of Object.entries(pkg.scripts ?? {})) {
        if (TERM_RE.test(String(cmd))) out.push(`${p}: scripts.${name} = ${cmd}`);
      }
    }
  }
  return out;
}

function isBannered(text) {
  return text.split('\n').slice(0, BANNER_WINDOW).some((l) => BANNER_RE.test(l));
}

function rule4(paths, read) {
  const out = [];
  for (const p of paths) {
    if (EXEMPT_RE.test(p)) continue;
    const text = read(p);
    if (text === null) continue; // binary
    if (isBannered(text)) continue;
    text.split('\n').forEach((line, i) => {
      if (DEAD_CMD_RE.test(line)) out.push(`${p}:${i + 1}: ${line.trim()}`);
    });
  }
  return out;
}

function report(paths, read) {
  const rows = [];
  let lines = 0;
  for (const p of paths) {
    if (EXEMPT_RE.test(p)) continue;
    const text = read(p);
    if (text === null) continue;
    const n = text.split('\n').filter((l) => TERM_RE.test(l)).length;
    if (n) { rows.push([n, p]); lines += n; }
  }
  rows.sort((a, b) => b[0] - a[0] || a[1].localeCompare(b[1]));
  return { rows, lines };
}

// ------------------------------------------------------------- plumbing --

function trackedPaths() {
  return execFileSync('git', ['ls-files', '-z'], { cwd: REPO_ROOT, encoding: 'utf8' })
    .split('\0')
    .filter(Boolean);
}

/** Text of a tracked file, or null when it looks binary (a NUL in the first 8 KiB). */
function readTracked(p) {
  const buf = readFileSync(join(REPO_ROOT, p));
  if (buf.subarray(0, 8192).includes(0)) return null;
  return buf.toString('utf8');
}

function pad(s, n) { return s + ' '.repeat(Math.max(0, n - s.length)); }

function run() {
  const paths = trackedPaths();
  const results = [
    ['rule 1 (deleted paths)', rule1(paths)],
    ['rule 2 (dependencies)', rule2(paths, readTracked)],
    ['rule 3 (executable config)', rule3(paths, readTracked)],
    ['rule 4 (dead commands)', rule4(paths, readTracked)],
  ];
  let failed = false;
  for (const [name, hits] of results) {
    console.log(`expo-residue: ${pad(name, 28)} ${hits.length ? `FAIL (${hits.length})` : 'ok'}`);
    for (const h of hits) console.log(`   ${h}`);
    if (hits.length) failed = true;
  }
  const { rows, lines } = report(paths, readTracked);
  console.log(`expo-residue: ${pad('prose mentions (report only)', 28)} ${rows.length} files, ${lines} lines`);
  for (const [n, p] of rows) console.log(`   ${String(n).padStart(4)} ${p}`);
  process.exit(failed ? 1 : 0);
}

// ------------------------------------------------------------ self-test --

function selfTest() {
  const dir = join(REPO_ROOT, FIXTURES);
  if (!existsSync(dir)) { console.error(`self-test: missing ${FIXTURES}/`); process.exit(1); }
  const fixture = (name) => readFileSync(join(dir, name), 'utf8');
  const reads = (map) => (p) => map[p] ?? fixture(p);
  const failures = [];
  const expect = (label, ok) => { if (!ok) failures.push(label); };

  // rule 1: a path under a deleted directory, and a deleted root file
  expect('rule 1 catches src/x.ts', rule1(['src/x.ts']).length === 1);
  expect('rule 1 catches app.json', rule1(['app.json']).length === 1);
  expect('rule 1 ignores app-web/src/x.ts', rule1(['app-web/vela-wallet/src/x.ts']).length === 0);
  // rule 2: an expo dependency in a package.json
  expect('rule 2 catches dependencies.expo', rule2(['x/package.json'], reads({ 'x/package.json': fixture('rule2-package.json') })).length === 1);
  expect('rule 2 ignores i18next', rule2(['ok-package.json'], reads({ 'ok-package.json': '{"dependencies":{"i18next":"1"}}' })).length === 0);
  // rule 3: an expo term in a workflow run: line, but not in a # comment
  expect('rule 3 catches a workflow step', rule3(['.github/workflows/rule3-ci.yml'], reads({ '.github/workflows/rule3-ci.yml': fixture('rule3-ci.yml') })).length === 1);
  expect('rule 3 ignores a workflow comment', rule3(['.github/workflows/c.yml'], reads({ '.github/workflows/c.yml': '# the Expo app is gone\njobs: {}\n' })).length === 0);
  expect('rule 3 catches a package.json script', rule3(['x/package.json'], reads({ 'x/package.json': '{"scripts":{"web":"expo start --web"}}' })).length === 1);
  // rule 4: a dead command in a living doc trips; the same command under a banner does not
  expect('rule 4 catches npm run build:web in a living doc', rule4(['rule4-doc.md'], reads({})).length === 1);
  expect('rule 4 exempts a banner\'d doc', rule4(['banner-doc.md'], reads({})).length === 0);
  expect('rule 4 exempts a 勘误 doc', rule4(['e.md'], reads({ 'e.md': '> **勘误（2026-09-11，spec 039）**：旧命令。\n\n```sh\nnpx expo start\n```\n' })).length === 0);
  expect('rule 4 ignores specs/', rule4(['specs/x.md'], reads({ 'specs/x.md': 'npx expo start\n' })).length === 0);
  expect('rule 4 skips binaries', rule4(['b.bin'], () => null).length === 0);
  // report: counts prose, ignores lockfiles
  expect('report counts a prose mention', report(['a.md'], reads({ 'a.md': 'ported from the Expo tree\n' })).lines === 1);
  expect('report ignores lockfiles', report(['x/package-lock.json'], reads({ 'x/package-lock.json': '"expo": {}\n' })).lines === 0);

  if (failures.length) {
    console.error('expo-residue self-test: FAIL');
    for (const f of failures) console.error(`   ${f}`);
    process.exit(1);
  }
  console.log('expo-residue self-test: ok (15 expectations)');
}

if (process.argv.includes('--self-test')) selfTest();
else run();
