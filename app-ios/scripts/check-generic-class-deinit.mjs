#!/usr/bin/env node
/**
 * Every generic class in the app target must declare `nonisolated deinit`.
 *
 * Not a style rule — a toolchain landmine. The target builds with
 * `-default-isolation MainActor` (SWIFT_DEFAULT_ACTOR_ISOLATION), which under
 * Swift 6.2 makes every deinit an ISOLATED deinit (SE-0371). Optimising the
 * deallocating destructor of a GENERIC class in that mode crashes
 * swift-frontend 6.2.4 in `EarlyPerfInliner`, inside
 * `isCallerAndCalleeLayoutConstraintsCompatible`. Three lines reproduce it:
 *
 *     echo 'final class Box<T> { var value: T; init(_ v: T) { self.value = v } }' > /tmp/r.swift
 *     xcrun swift-frontend -frontend -c /tmp/r.swift -O -swift-version 5 \
 *       -default-isolation=MainActor -module-name R \
 *       -target arm64-apple-ios17.4-simulator \
 *       -sdk "$(xcrun --sdk iphonesimulator --show-sdk-path)" -o /tmp/r.o
 *
 * Debug builds are `-Onone`, so the inliner never runs and nobody notices —
 * which is exactly how this reached 057 undetected and made Release and
 * Archive impossible. `nonisolated deinit` restores the pre-6.2 destructor.
 *
 * Writing one is safe when the deinit touches no main-actor state, which is
 * every case here (all three bodies are empty). A generic class that genuinely
 * needs an isolated deinit cannot be built in Release by this toolchain at
 * all — that is the finding, not this guard's problem to paper over.
 *
 * Usage: node app-ios/scripts/check-generic-class-deinit.mjs
 * Exit 0 = every generic class is safe. Exit 1 = names the ones that are not.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('../..', import.meta.url).pathname;
const TARGET = join(ROOT, 'app-ios/VelaWallet/VelaWallet');

/**
 * Directories the Release build does not compile, with the reason. `Dev/` is
 * the uniffi dev-fixture bindings: a Debug-only folder (verified against the
 * Release `sources-*` filelists, which do not list one of its files), and
 * generated, so an edit there would be overwritten by the next
 * `build-ios-dev-fixtures.sh`.
 */
const EXEMPT_DIRS = ['/Dev/'];

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) walk(p, out);
		else if (p.endsWith('.swift')) out.push(p);
	}
	return out;
}

/** The body of the declaration that starts at `open`, by brace matching. */
function body(text, open) {
	let depth = 0;
	for (let i = open; i < text.length; i++) {
		if (text[i] === '{') depth++;
		else if (text[i] === '}') {
			depth--;
			if (depth === 0) return text.slice(open, i + 1);
		}
	}
	return text.slice(open);
}

const offenders = [];
for (const file of walk(TARGET)) {
	if (EXEMPT_DIRS.some((d) => file.includes(d))) continue;
	const text = readFileSync(file, 'utf8');
	const re = /(?:^|\n)[ \t]*(?:(?:public|internal|private|fileprivate|final|open)\s+)*class\s+(\w+)\s*<[^>{]*>[^{]*\{/g;
	for (const m of [...text.matchAll(re)]) {
		const open = m.index + m[0].lastIndexOf('{');
		if (/\bnonisolated\s+deinit\b/.test(body(text, open))) continue;
		offenders.push({
			file: relative(ROOT, file),
			line: text.slice(0, m.index).split('\n').length,
			name: m[1]
		});
	}
}

if (offenders.length === 0) {
	console.log('OK: every generic class in the app target declares `nonisolated deinit`.');
	process.exit(0);
}
console.error('Generic classes without `nonisolated deinit` — Release will crash the compiler:\n');
for (const o of offenders) console.error(`  ${o.file}:${o.line}  class ${o.name}`);
console.error('\nAdd `nonisolated deinit {}` to each, or make the class non-generic.');
process.exit(1);
