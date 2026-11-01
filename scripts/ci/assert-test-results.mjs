#!/usr/bin/env node
/**
 * A suite that runs NOTHING passes. This reads what a required job actually
 * ran and fails when a test that must run did not — skipped, filtered away,
 * renamed, or never compiled into the target.
 *
 *   node scripts/ci/assert-test-results.mjs --platform android --junit <dir of TEST-*.xml>
 *   node scripts/ci/assert-test-results.mjs --platform web     --junit <vitest junit file>
 *   node scripts/ci/assert-test-results.mjs --platform ios     --xcresult <bundle> | --xcresult-json <file>
 *   node scripts/ci/assert-test-results.mjs --self-test
 *
 * What must have run comes from docs/scenarios/multisig.yaml:
 *
 *   - `vector_replays.<platform>`: every suite that replays the core's shared
 *     vectors (rust/crates/vela-core/tests/vectors). Each must have run at
 *     least one test, with none skipped and none failed.
 *   - every scenario entry on this platform's required suite (android-jvm,
 *     web-unit, ios-unit) marked `ci: required`: that test must have run and
 *     not been skipped.
 *
 * Exit 1 with a list of what is missing; 0 with a one-line tally.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';
import {
	ROOT,
	enclosingSuite,
	entries,
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

/** Which scenario-manifest platform a runner's required job covers. */
const RUNNER_PLATFORM = { android: 'android-jvm', web: 'web-unit', ios: 'ios-unit' };

// ---------------------------------------------------------------------------
// Readers: every result as { suite, name, status: passed|failed|skipped }.

const unescapeXml = (s) =>
	s
		.replace(/&lt;/g, '<')
		.replace(/&gt;/g, '>')
		.replace(/&quot;/g, '"')
		.replace(/&apos;/g, "'")
		.replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)))
		.replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)))
		.replace(/&amp;/g, '&');

export function readJunitXml(xml) {
	const results = [];
	const re = /<testcase\b([^>]*?)(\/>|>([\s\S]*?)<\/testcase>)/g;
	for (const m of xml.matchAll(re)) {
		const attrs = {};
		for (const a of m[1].matchAll(/([\w:-]+)="([^"]*)"/g)) attrs[a[1]] = unescapeXml(a[2]);
		const body = m[3] ?? '';
		const status = /<skipped\b/.test(body)
			? 'skipped'
			: /<(failure|error)\b/.test(body)
				? 'failed'
				: 'passed';
		results.push({ suite: attrs.classname ?? '', name: attrs.name ?? '', status });
	}
	return results;
}

function readJunitPath(path) {
	const files = statSync(path).isDirectory()
		? readdirSync(path)
				.filter((f) => f.endsWith('.xml'))
				.map((f) => join(path, f))
		: [path];
	return files.flatMap((f) => readJunitXml(readFileSync(f, 'utf8')));
}

/**
 * `xcresulttool get test-results tests` JSON → results. `suite` is the
 * innermost Test Suite; `suites` every one it sits in, so a test in a nested
 * Swift Testing suite still belongs to the type its file declares.
 */
export function readXcresultJson(json) {
	const results = [];
	const walk = (node, suites) => {
		if (node.nodeType === 'Test Case') {
			const r = String(node.result ?? '');
			const status = r === 'Skipped' ? 'skipped' : r === 'Failed' ? 'failed' : 'passed';
			results.push({ suite: suites.at(-1) ?? '', suites, name: node.name ?? '', status });
			return; // its children are arguments, messages and attachments
		}
		const next = node.nodeType === 'Test Suite' ? [...suites, node.name] : suites;
		for (const child of node.children ?? []) walk(child, next);
	};
	for (const node of json.testNodes ?? []) walk(node, []);
	return results;
}

// ---------------------------------------------------------------------------
// Matching.

/** Does a result belong to the suite a manifest entry names? */
function inSuite(platform, result, suite) {
	if (platform === 'android') {
		// JUnit's classname is the fully-qualified Kotlin class; a manifest may
		// name it simply. Nested classes report as Outer$Inner.
		const c = result.suite;
		return c === suite || c.endsWith(`.${suite}`) || c.startsWith(`${suite}$`) || c.includes(`.${suite}$`);
	}
	return result.suite === suite || (result.suites ?? []).includes(suite);
}

/** Does a result's name match a manifest test identifier? */
function isTest(platform, result, test) {
	const n = result.name;
	if (platform === 'web') return n === test || n.endsWith(` > ${test}`);
	if (platform === 'ios') {
		const bare = test.replace(/\(\)$/, '');
		return n === test || n === `${bare}()` || n.startsWith(`${bare}(`);
	}
	return n === test || n.replace(/\[.*\]$/, '') === test; // parameterized JUnit
}

/**
 * The suite a manifest entry's results are reported under: its `suite`; for
 * vitest its path under the app; for Kotlin/Swift the type the test is
 * declared in (a file may hold several), else the file's name.
 */
function suiteOf(platform, entry) {
	if (entry.suite) return entry.suite;
	if (platform === 'web') return entry.file.replace(/^app-web\/vela-wallet\//, '');
	const fallback = basename(entry.file).replace(/\.(kt|swift)$/, '');
	const path = join(ROOT, entry.file);
	if (!entry.test || !existsSync(path)) return fallback;
	const source = readFileSync(path, 'utf8');
	const at = findDeclaration(source, extOf(entry.file), String(entry.test));
	return (at >= 0 && enclosingSuite(source, at)) || fallback;
}

export function assess(platform, results, manifest) {
	const problems = [];
	let checked = 0;
	for (const v of vectorReplays(manifest, platform)) {
		const suite = suiteOf(platform, v);
		const mine = results.filter((r) => inSuite(platform, r, suite));
		checked++;
		if (mine.length === 0) {
			problems.push(`vector replay ${suite} (${v.file}): no test ran`);
			continue;
		}
		const skipped = mine.filter((r) => r.status === 'skipped');
		const failed = mine.filter((r) => r.status === 'failed');
		if (skipped.length)
			problems.push(
				`vector replay ${suite}: ${skipped.length} skipped — ${skipped
					.slice(0, 3)
					.map((r) => r.name)
					.join('; ')}`
			);
		if (failed.length) problems.push(`vector replay ${suite}: ${failed.length} failed`);
	}
	const scenarioPlatform = RUNNER_PLATFORM[platform];
	for (const e of entries(manifest)) {
		if (e.platform !== scenarioPlatform || e.gap || e.ci !== 'required') continue;
		checked++;
		const suite = suiteOf(platform, e);
		const inThatSuite = results.filter((r) => inSuite(platform, r, suite));
		const hit = inThatSuite.filter((r) => isTest(platform, r, e.test));
		if (hit.length === 0) {
			const seen = inThatSuite.length
				? `${inThatSuite.length} other tests ran in ${suite}`
				: `no test ran in ${suite}`;
			problems.push(`scenario ${e.scenario}: ${e.file} › ${e.test} did not run (${seen})`);
		} else if (hit.every((r) => r.status === 'skipped')) {
			problems.push(`scenario ${e.scenario}: ${e.file} › ${e.test} was skipped`);
		}
	}
	return { problems, checked };
}

// ---------------------------------------------------------------------------

function selfTest() {
	const manifest = {
		vector_replays: { android: [{ file: 'x/R10CoreVectorsTest.kt' }] },
		scenarios: [
			{
				id: 's',
				covers: {
					'android-jvm': [{ file: 'x/FooTest.kt', test: 'it works', ci: 'required' }]
				}
			}
		]
	};
	const xml = (cases) =>
		`<testsuite>${cases
			.map(
				([c, n, body]) =>
					`<testcase name="${n}" classname="${c}" time="0">${body ?? ''}</testcase>`
			)
			.join('')}</testsuite>`;
	const ok = readJunitXml(
		xml([
			['app.getvela.wallet.R10CoreVectorsTest', 'a'],
			['app.getvela.wallet.FooTest', 'it works']
		])
	);
	const skipped = readJunitXml(
		xml([
			['app.getvela.wallet.R10CoreVectorsTest', 'a', '<skipped/>'],
			['app.getvela.wallet.FooTest', 'it works']
		])
	);
	const missing = readJunitXml(xml([['app.getvela.wallet.R10CoreVectorsTest', 'a']]));
	const cases = [
		['all ran', assess('android', ok, manifest).problems.length === 0],
		['a skipped vector test is caught', assess('android', skipped, manifest).problems.length === 1],
		['a scenario test that never ran is caught', assess('android', missing, manifest).problems.length === 1],
		[
			'vitest names match by their last segment',
			isTest('web', { name: 'outer &gt; inner'.replace('&gt;', '>') }, 'inner')
		],
		[
			'xcresult names match with and without ()',
			isTest('ios', { name: 'reads()' }, 'reads') && isTest('ios', { name: 'reads(x:)' }, 'reads()')
		],
		[
			'xcresult suites nest',
			readXcresultJson({
				testNodes: [
					{
						nodeType: 'Unit test bundle',
						children: [
							{
								nodeType: 'Test Suite',
								name: 'S',
								children: [{ nodeType: 'Test Case', name: 't()', result: 'Skipped' }]
							}
						]
					}
				]
			})[0].suite === 'S'
		]
	];
	let bad = 0;
	for (const [name, pass] of cases) {
		if (!pass) bad++;
		console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}`);
	}
	process.exit(bad ? 1 : 0);
}

if (args.includes('--self-test')) selfTest();

const platform = option('--platform');
if (!RUNNER_PLATFORM[platform]) {
	console.error('usage: assert-test-results.mjs --platform android|web|ios (--junit <path> | --xcresult <bundle> | --xcresult-json <file>)');
	process.exit(2);
}

let results;
if (option('--junit')) results = readJunitPath(option('--junit'));
else if (option('--xcresult-json')) results = readXcresultJson(JSON.parse(readFileSync(option('--xcresult-json'), 'utf8')));
else if (option('--xcresult'))
	results = readXcresultJson(
		JSON.parse(
			execFileSync('xcrun', ['xcresulttool', 'get', 'test-results', 'tests', '--path', option('--xcresult')], {
				maxBuffer: 1 << 30
			}).toString()
		)
	);
else {
	console.error('assert-test-results: give --junit, --xcresult or --xcresult-json');
	process.exit(2);
}
if (results.length === 0) {
	console.error(`assert-test-results (${platform}): the report holds no test at all`);
	process.exit(1);
}

const { problems, checked } = assess(platform, results, loadManifest(option('--manifest')));
const tally = `${results.length} results read, ${results.filter((r) => r.status === 'skipped').length} skipped overall`;
if (problems.length) {
	console.error(`assert-test-results (${platform}): ${problems.length} of ${checked} requirements not met (${tally})`);
	for (const p of problems) console.error(`  - ${p}`);
	console.error('The list is docs/scenarios/multisig.yaml (vector_replays, and scenario entries marked ci: required).');
	process.exit(1);
}
console.log(`assert-test-results (${platform}): ${checked} requirements met (${tally})`);
