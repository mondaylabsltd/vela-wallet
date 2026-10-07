#!/usr/bin/env node
// Split VelaWalletTests across N CI machines (ci.yml `ios`, the unit legs).
//
//   node app-ios/scripts/ios-test-shards.mjs <shard 1..N> <N>   # xcodebuild args, one per line
//   node app-ios/scripts/ios-test-shards.mjs --list <N>         # the buckets and their weights
//
// Why machines and not `-parallel-testing-enabled`: nearly every suite is
// @MainActor, so the unit suite is bound by ONE main thread — 1256 tests took
// 410 s on a 12-core Mac, and Xcode's in-process parallel testing measured no
// faster (458 s serial, 472 s with three workers). A second process has a main
// thread of its own; a second machine has its own process.
//
// Shards 1..N-1 each run an explicit list of suites (`-only-testing`). Shard N
// runs EVERYTHING ELSE (`-skip-testing` the others' lists), so a suite this
// script does not recognise — a new file, a free @Test function, a spelling the
// parser misses — still runs, in the last shard. Nothing can fall between them.
//
// Suites are the top-level types (and extensions) that hold @Test functions,
// weighted by how many they hold, and dealt greedily (heaviest first) to the
// lightest bucket. Deterministic: same sources, same split.

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const TESTS = join(dirname(fileURLToPath(import.meta.url)), '..', 'VelaWallet', 'VelaWalletTests');
const TARGET = 'VelaWalletTests';
const UI_TARGET = 'VelaWalletUITests';

// A top-level declaration: column 0, optional attributes and modifiers.
const DECL =
  /^(?:@[A-Za-z]+(?:\([^)]*\))?\s+)*(?:(?:public|internal|private|fileprivate|final)\s+)*(?:struct|class|enum|actor|extension)\s+([A-Za-z_][A-Za-z0-9_]*)/;
const TEST = /^\s*@Test\b/;

export function suites(dir = TESTS) {
  const weight = new Map();
  for (const file of readdirSync(dir).filter((f) => f.endsWith('.swift')).sort()) {
    let current = null;
    for (const line of readFileSync(join(dir, file), 'utf8').split('\n')) {
      const decl = line.match(DECL);
      if (decl) current = decl[1];
      if (TEST.test(line) && current) weight.set(current, (weight.get(current) ?? 0) + 1);
    }
  }
  return [...weight.entries()]
    .map(([name, tests]) => ({ name, tests }))
    .sort((a, b) => b.tests - a.tests || a.name.localeCompare(b.name));
}

export function buckets(n, all = suites()) {
  const out = Array.from({ length: n }, () => ({ tests: 0, names: [] }));
  for (const suite of all) {
    const lightest = out.reduce((min, b) => (b.tests < min.tests ? b : min), out[0]);
    lightest.tests += suite.tests;
    lightest.names.push(suite.name);
  }
  for (const b of out) b.names.sort();
  return out;
}

export function args(shard, n, all = suites()) {
  const bs = buckets(n, all);
  if (shard < n) return bs[shard - 1].names.map((name) => `-only-testing:${TARGET}/${name}`);
  return [
    `-skip-testing:${UI_TARGET}`,
    ...bs.slice(0, n - 1).flatMap((b) => b.names.map((name) => `-skip-testing:${TARGET}/${name}`)),
  ];
}

const [first, second] = process.argv.slice(2);
if (first === '--list') {
  const n = Number(second);
  buckets(n).forEach((b, i) =>
    console.log(`shard ${i + 1}/${n}${i === n - 1 ? ' (and everything unlisted)' : ''}: ${b.tests} tests, ${b.names.length} suites`),
  );
} else if (first) {
  const shard = Number(first);
  const n = Number(second);
  if (!(Number.isInteger(shard) && Number.isInteger(n) && shard >= 1 && shard <= n)) {
    console.error('usage: ios-test-shards.mjs <shard 1..N> <N> | --list <N>');
    process.exit(2);
  }
  console.log(args(shard, n).join('\n'));
}
