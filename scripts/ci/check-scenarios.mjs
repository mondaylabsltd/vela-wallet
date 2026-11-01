#!/usr/bin/env node
/**
 * The scenario manifest's static gate (native parity Phase 5, T1).
 *
 *   node scripts/ci/check-scenarios.mjs                  check, print the matrix and the gaps
 *   node scripts/ci/check-scenarios.mjs --summary <file> also append the matrix as markdown
 *   node scripts/ci/check-scenarios.mjs --self-test      prove the matcher bites
 *
 * For docs/scenarios/multisig.yaml, fails when:
 *   - a listed file does not exist;
 *   - a listed test is not declared in it under that exact name (Rust `fn`,
 *     Kotlin `fun`, Swift `func`, a quoted vitest/Playwright title, or a
 *     trusted-signer check label quoted in its sample);
 *   - a test marked `ci: required` carries a skip marker where it is
 *     declared (`#[ignore]`, `@Ignore`, `.disabled`, `.enabled(if:`,
 *     `XCTSkip`, `.skip(`, `skipIf`) — a required test that does not run is
 *     not coverage;
 *   - a platform, `ci` value, or scenario shape the manifest does not define.
 *
 * Gaps (`gap:`) are printed, never failed: they are the backlog.
 */
import { appendFileSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
	CI_VALUES,
	PLATFORMS,
	ROOT,
	entries,
	escapeRe,
	extOf,
	findDeclaration,
	loadManifest,
	vectorReplays
} from './scenarios-lib.mjs';

const args = process.argv.slice(2);
const option = (name) => {
	const at = args.indexOf(name);
	return at >= 0 ? args[at + 1] : undefined;
};

const SKIP = /#\[ignore\b|@Ignore\b|\.disabled\s*\(|\.enabled\s*\(\s*if:|XCTSkip|\b(?:it|test|describe)\.skip\b|\.skipIf\s*\(|\btest\.fixme\b/;

/** A skip marker on the declaration: the few lines above it, and its own line. */
export function skipMarker(source, index) {
	const before = source.slice(0, index).split('\n');
	const own = source.slice(index).split('\n')[0];
	// Up to 4 lines above (attributes, annotations), stopping at the previous
	// declaration's closing brace.
	const above = [];
	for (let i = before.length - 2; i >= 0 && above.length < 4; i--) {
		const line = before[i];
		if (/^\s*}\s*$/.test(line)) break;
		above.push(line);
	}
	const window = [...above, before[before.length - 1] + own].join('\n');
	const m = SKIP.exec(window);
	return m ? m[0] : null;
}


function selfTest() {
	const cases = [
		['rust fn', findDeclaration('#[test]\nfn a_b() {}', 'rs', 'a_b') >= 0],
		['rust fn: prefix is not a match', findDeclaration('fn a_b_c() {}', 'rs', 'a_b') < 0],
		['kotlin backticks', findDeclaration('@Test\nfun `a b c`() {}', 'kt', 'a b c') >= 0],
		['swift func', findDeclaration('@Test func reads() {}', 'swift', 'reads') >= 0],
		['swift XCTest', findDeclaration('func testX() async throws {', 'swift', 'testX') >= 0],
		['vitest title', findDeclaration("it('does a thing', () => {", 'ts', 'does a thing') >= 0],
		['vitest title only as a title', findDeclaration("const s = 'does a thing';", 'ts', 'does a thing') < 0],
		['playwright test.describe', findDeclaration('test.describe.serial(`x y`, () =>', 'ts', 'x y') >= 0],
		['label', findDeclaration("check('a label here', ok)", 'label', 'a label') >= 0],
		[
			'ignored rust test is caught',
			skipMarker('#[test]\n#[ignore = "net"]\nfn a() {}', '#[test]\n#[ignore = "net"]\n'.length) !== null
		],
		[
			'enabled(if:) swift test is caught',
			skipMarker('@Test(.enabled(if: x)) func a() {}', '@Test(.enabled(if: x)) '.length) !== null
		],
		[
			'a previous test’s marker is not this one’s',
			skipMarker('#[ignore]\nfn a() {\n}\n#[test]\nfn b() {}', '#[ignore]\nfn a() {\n}\n#[test]\n'.length) === null
		],
		['it.skip is caught', skipMarker("it.skip('x', () => {})", 0) !== null]
	];
	let bad = 0;
	for (const [name, pass] of cases) {
		if (!pass) bad++;
		console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}`);
	}
	process.exit(bad ? 1 : 0);
}

if (args.includes('--self-test')) selfTest();

const manifest = loadManifest(option('--manifest'));
const problems = [];
const sources = new Map();
const read = (file) => {
	if (!sources.has(file)) {
		const p = join(ROOT, file);
		sources.set(file, existsSync(p) ? readFileSync(p, 'utf8') : null);
	}
	return sources.get(file);
};

const ids = new Set();
for (const s of manifest.scenarios ?? []) {
	if (!s.id || !s.title || !s.covers) problems.push(`a scenario lacks id, title or covers: ${JSON.stringify(s).slice(0, 80)}`);
	if (ids.has(s.id)) problems.push(`scenario id ${s.id} is listed twice`);
	ids.add(s.id);
	for (const p of PLATFORMS) {
		if (!(p in (s.covers ?? {}))) problems.push(`${s.id}: no entry for ${p} (map it, or write gap: and why)`);
	}
}

const all = entries(manifest);
let tests = 0;
for (const e of all) {
	if (!PLATFORMS.includes(e.platform)) {
		problems.push(`${e.scenario}: unknown platform ${e.platform}`);
		continue;
	}
	if (e.gap !== undefined) continue;
	tests++;
	const where = `${e.scenario} / ${e.platform}`;
	if (!e.file || !e.test) {
		problems.push(`${where}: an entry needs file and test (or gap:)`);
		continue;
	}
	if (!CI_VALUES.includes(e.ci)) problems.push(`${where}: ci must be one of ${CI_VALUES.join(', ')} (got ${e.ci})`);
	const source = read(e.file);
	if (source === null) {
		problems.push(`${where}: ${e.file} does not exist`);
		continue;
	}
	const at = findDeclaration(source, extOf(e.file), String(e.test));
	if (at < 0) {
		problems.push(`${where}: ${e.file} declares no test "${e.test}"`);
		continue;
	}
	if (e.ci === 'required') {
		const marker = skipMarker(source, at);
		if (marker) problems.push(`${where}: "${e.test}" is ci: required but carries ${marker}`);
	}
}

let replays = 0;
for (const runner of Object.keys(manifest.vector_replays ?? {})) {
	for (const v of vectorReplays(manifest, runner)) {
		replays++;
		const source = read(v.file);
		if (source === null) problems.push(`vector_replays.${runner}: ${v.file} does not exist`);
		else if (v.suite && !new RegExp(`\\b(?:struct|class)\\s+${escapeRe(v.suite)}\\b`).test(source))
			problems.push(`vector_replays.${runner}: ${v.file} declares no ${v.suite}`);
	}
}

// The matrix: per scenario, per platform, how many tests and the best CI tier.
const RANK = { required: 0, main: 1, nightly: 2, 'opt-in': 3, local: 4 };
const cell = (scenario, platform) => {
	const mine = all.filter((e) => e.scenario === scenario && e.platform === platform);
	if (mine.some((e) => e.gap !== undefined)) return 'GAP';
	if (!mine.length) return '?';
	const best = mine.map((e) => e.ci).sort((a, b) => RANK[a] - RANK[b])[0];
	return `${mine.length} ${best}`;
};
const rows = (manifest.scenarios ?? []).map((s) => [s.id, ...PLATFORMS.map((p) => cell(s.id, p))]);
const gaps = all.filter((e) => e.gap !== undefined);

const width = [Math.max(8, ...rows.map((r) => r[0].length)), ...PLATFORMS.map((p) => Math.max(p.length, 10))];
const line = (cells) => cells.map((c, i) => String(c).padEnd(width[i])).join('  ');
console.log(line(['scenario', ...PLATFORMS]));
for (const r of rows) console.log(line(r));
console.log(`\n${gaps.length} gaps:`);
for (const g of gaps) console.log(`  ${g.scenario} / ${g.platform}: ${g.gap}`);

const summary = option('--summary');
if (summary) {
	const md = [
		'### Multisig scenario manifest (docs/scenarios/multisig.yaml)',
		'',
		`| scenario | ${PLATFORMS.join(' | ')} |`,
		`|---|${PLATFORMS.map(() => '---').join('|')}|`,
		...rows.map((r) => `| ${r.join(' | ')} |`),
		'',
		`<details><summary>${gaps.length} gaps</summary>`,
		'',
		...gaps.map((g) => `- **${g.scenario} / ${g.platform}**: ${g.gap}`),
		'',
		'</details>',
		''
	].join('\n');
	appendFileSync(summary, md + '\n');
}

if (problems.length) {
	console.error(`\ncheck-scenarios: ${problems.length} problems`);
	for (const p of problems) console.error(`  - ${p}`);
	process.exit(1);
}
console.log(`\ncheck-scenarios: ${ids.size} scenarios, ${tests} tests and ${replays} vector replays named, all found; ${gaps.length} gaps recorded`);
