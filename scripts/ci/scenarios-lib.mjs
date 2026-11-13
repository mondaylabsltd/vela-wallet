// The scenario manifest (docs/scenarios/multisig.yaml), read once for the two
// gates that use it: check-scenarios.mjs (static: every test it names exists)
// and assert-test-results.mjs (runtime: the ones a required job runs DID run).

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
export const MANIFEST = join(ROOT, 'docs/scenarios/multisig.yaml');

/** Platform keys, in the column order the summary prints them. */
export const PLATFORMS = [
	'core',
	'web-unit',
	'web-e2e',
	'android-jvm',
	'android-instrumented',
	'ios-unit',
	'ios-ui',
	'desktop',
	'trusted-signer'
];

/** Where a mapped test runs in CI. */
export const CI_VALUES = ['required', 'main', 'nightly', 'opt-in', 'local'];

export function loadManifest(path = MANIFEST) {
	return parse(readFileSync(path, 'utf8'));
}

/**
 * Every (scenario, platform, entry) the manifest maps, gaps included:
 * `{ scenario, platform, gap?: string, file?, test?, ci?, note? }`.
 */
export function entries(manifest) {
	const out = [];
	for (const scenario of manifest.scenarios ?? []) {
		for (const [platform, value] of Object.entries(scenario.covers ?? {})) {
			if (value && typeof value === 'object' && !Array.isArray(value) && 'gap' in value) {
				out.push({ scenario: scenario.id, platform, gap: String(value.gap) });
				continue;
			}
			for (const entry of Array.isArray(value) ? value : [value]) {
				out.push({ scenario: scenario.id, platform, ...entry });
			}
		}
	}
	return out;
}

/** `vector_replays.<runner>`: `[{ file, suite }]`. */
export function vectorReplays(manifest, runner) {
	return manifest.vector_replays?.[runner] ?? [];
}

export const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * Where `test` is declared in `source` (a file of kind `ext`): the index of
 * the declaration, or -1.
 */
export function findDeclaration(source, ext, test) {
	const t = escapeRe(test);
	let re;
	switch (ext) {
		case 'rs':
			re = new RegExp(`\\bfn\\s+${t}\\s*[(<]`);
			break;
		case 'kt':
			re = new RegExp(`\\bfun\\s+(?:${t}|\`${t}\`)\\s*\\(`);
			break;
		case 'swift':
			re = new RegExp(`\\bfunc\\s+${escapeRe(test.replace(/\(\)$/, ''))}\\s*\\(|@Test\\(\\s*"${t}"`);
			break;
		case 'ts':
		case 'js':
			// A test title, in any of the three quotes, as the first argument.
			re = new RegExp(`\\b(?:it|test|describe)(?:\\.\\w+)*\\(\\s*(['"\`])${t}\\1`);
			break;
		default:
			// A trusted-signer check label: quoted somewhere in the sample.
			re = new RegExp(`(['"\`])[^'"\`]*${t}`);
	}
	const m = re.exec(source);
	return m ? m.index : -1;
}


/** The kind of source a manifest file is, for findDeclaration. */
export function extOf(file) {
	if (file.endsWith('.rs')) return 'rs';
	if (file.endsWith('.kt')) return 'kt';
	if (file.endsWith('.swift')) return 'swift';
	if (file.includes('/trusted-signer/samples/')) return 'label';
	if (/\.(ts|js)$/.test(file)) return 'ts';
	return 'label';
}

/**
 * The type a Kotlin or Swift test at `index` is reported under: the last
 * top-level `class` / `struct` / `extension` / `object` declared above it
 * (`@MainActor`, `@Suite(…)` and `final` allowed in front). Swift Testing and
 * XCTest both report a test under its type; JUnit under its class.
 */
export function enclosingSuite(source, index) {
	const re =
		/^(?:@\w+(?:\([^)\n]*\))?\s+)*(?:(?:final|public|internal|private|open|data|abstract)\s+)*(?:class|struct|extension|object|enum|actor)\s+(\w+)/gm;
	let suite = null;
	for (const m of source.slice(0, index).matchAll(re)) suite = m[1];
	return suite;
}
