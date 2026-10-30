#!/usr/bin/env node
//
// The PerfTests figures beside their targets (native parity Phase 4).
//
//   node app-ios/scripts/perf/report-perf.mjs <result.xcresult> [--json out.json]
//
// Reads `xcrun xcresulttool get test-results metrics` and prints one line per
// metric: the mean, the median, and — where the audit sets one — the target
// with `ok` or `OVER`. Always exits 0 when the bundle could be read: the
// targets are reported, never enforced (simulator figures are indicative).
//
// The targets mirror `PerfTests.Target`; keep the two together.

import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const [bundle, ...rest] = process.argv.slice(2);
if (!bundle) {
	console.error('usage: report-perf.mjs <result.xcresult> [--json out.json]');
	process.exit(2);
}
const jsonOut = rest[0] === '--json' ? rest[1] : null;

// metric id fragment → { label, target, scale (to the target's unit), unit }
const TARGETS = [
	{ id: 'OSSignpost-HomeCachedPaint.duration', label: 'cold start → cached home', target: 1.2, unit: 's' },
	{ id: 'ApplicationLaunch-ApplicationFirstFramePresentationResponsive', label: 'launch → first frame, responsive (lockup plays)', unit: 's' },
	{ id: 'ApplicationLaunch-ApplicationLaunch', label: 'launch → first frame', unit: 's' },
	{ id: 'OSSignpost-ConfirmToPasskey.duration', label: 'Confirm → passkey', target: 300, scale: 1000, unit: 'ms' },
	{ id: 'physical_peak', label: 'memory, peak physical', target: 180, scale: 1 / 1024, unit: 'MB' },
	{ id: 'physical_absolute', label: 'memory, physical at end', target: 180, scale: 1 / 1024, unit: 'MB' },
	{ id: 'Scroll_Deceleration.hitch_count', label: 'scroll hitches (200 rows)', target: 0, unit: '' },
	{ id: 'Scroll_Deceleration.hitch_time_ratio', label: 'scroll hitch time ratio', unit: 'ms/s' },
	{ id: 'Scroll_Deceleration.frame_rate', label: 'scroll frame rate', unit: 'fps' },
];

let raw;
try {
	raw = execFileSync('xcrun', ['xcresulttool', 'get', 'test-results', 'metrics', '--path', bundle], {
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
	});
} catch (error) {
	console.error(`report-perf: could not read ${bundle}: ${error.message}`);
	process.exit(1);
}

const median = (xs) => {
	const s = [...xs].sort((a, b) => a - b);
	const m = Math.floor(s.length / 2);
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
const fmt = (x) => (Math.abs(x) >= 100 ? x.toFixed(0) : x.toFixed(3));

const rows = [];
for (const test of JSON.parse(raw)) {
	for (const run of test.testRuns ?? []) {
		for (const metric of run.metrics ?? []) {
			const known = TARGETS.find((t) => metric.identifier.includes(t.id));
			const scale = known?.scale ?? 1;
			const values = (metric.measurements ?? []).map((v) => v * scale);
			if (!values.length) continue;
			const row = {
				test: test.testIdentifier,
				metric: known?.label ?? metric.displayName,
				unit: known?.unit ?? metric.unitOfMeasurement,
				mean: mean(values),
				median: median(values),
				values,
				target: known?.target ?? null,
			};
			row.verdict = row.target === null ? '' : row.median <= row.target ? 'ok' : 'OVER';
			rows.push(row);
		}
	}
}

console.log('PerfTests — simulator figures are indicative only; targets are not enforced');
for (const r of rows) {
	const target = r.target === null ? '' : `  target ≤${r.target} ${r.unit}  ${r.verdict}`;
	console.log(`  ${r.metric.padEnd(48)} median ${fmt(r.median)} ${r.unit} (mean ${fmt(r.mean)}, n=${r.values.length})${target}`);
}
if (!rows.some((r) => r.metric.startsWith('scroll hitches'))) {
	console.log('  scroll hitches: not reported by this destination (a simulator reports none; run on a device)');
}
if (jsonOut) writeFileSync(jsonOut, JSON.stringify(rows, null, 2));
