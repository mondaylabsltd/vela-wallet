// out/results.json + out/sizes.json -> out/report.md (and stdout).
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { LAB } from './decoders.mjs';

const res = JSON.parse(readFileSync(path.join(LAB, 'out/results.json'), 'utf8'));
const sizes = existsSync(path.join(LAB, 'out/sizes.json')) ? JSON.parse(readFileSync(path.join(LAB, 'out/sizes.json'), 'utf8')) : [];
const S = res.samples;
const FIRST = ['web', 'zbar', 'jsqr'];
const seen = new Set(S.flatMap((s) => Object.keys(s.results)));
// Only decoders every sample has a result for (a partial run is not a column).
const complete = [...seen].filter((n) => S.every((s) => s.results[n]));
const NAMES = [...FIRST.filter((n) => complete.includes(n)), ...complete.filter((n) => !FIRST.includes(n)).sort()];
const LABEL = new Proxy({ web: 'web today (zbar→jsqr)', zbar: 'zbar-wasm' }, { get: (t, k) => t[k] ?? k });

const pct = (a, b) => (b ? `${((100 * a) / b).toFixed(1)}%` : '—');
const q = (xs, p) => {
	if (!xs.length) return NaN;
	const s = [...xs].sort((a, b) => a - b);
	return s[Math.min(s.length - 1, Math.floor(p * s.length))];
};
const ms = (x) => (Number.isNaN(x) ? '—' : x < 10 ? x.toFixed(2) : x.toFixed(0));
const kb = (b) => `${(b / 1024).toFixed(0)} KB`;
const get = (s, name, mode) => s.results[name]?.[mode];
const categories = [...new Set(S.map((s) => s.category))];

const lines = [];
const p = (l = '') => lines.push(l);

p(`# QR decoder lab — results`);
p();
p(`${S.length} samples, ${categories.length} categories · corpus seed ${res.corpus.seed} · ${res.node} · ${res.date.slice(0, 10)}`);
p();
p(`**still** = a picked image (ladder 1200…400 then the original, plain + inverted). **camera** = one 1000-wide frame. Latency is decode time only, in V8 (Node), single thread.`);
p();

p(`## Summary`);
p();
p(`| decoder | still hit | wrong | camera hit | still latency p50 / p95 / max (ms) | camera frame p50 / p95 (ms) |`);
p(`|---|---:|---:|---:|---|---|`);
for (const n of NAMES) {
	const still = S.map((s) => get(s, n, 'still'));
	const cam = S.map((s) => get(s, n, 'camera')).filter(Boolean);
	const hits = still.filter((r) => r.status === 'hit').length;
	const wrong = still.filter((r) => r.status === 'wrong').length + cam.filter((r) => r.status === 'wrong').length;
	const camHits = cam.filter((r) => r.status === 'hit').length;
	const lat = still.map((r) => r.ms);
	const frame = cam.map((r) => r.firstMs);
	p(
		`| ${LABEL[n]} | ${hits}/${S.length} (${pct(hits, S.length)}) | ${wrong} | ${cam.length ? `${camHits}/${cam.length} (${pct(camHits, cam.length)})` : '—'} | ${ms(q(lat, 0.5))} / ${ms(q(lat, 0.95))} / ${ms(Math.max(...lat))} | ${cam.length ? `${ms(q(frame, 0.5))} / ${ms(q(frame, 0.95))}` : '—'} |`
	);
}
if (Object.values(res.crashes).some((c) => c > 0)) p(`\nCrashes (wasm traps): ${JSON.stringify(res.crashes)}`);
if (res.zbarNonQrFirst?.length) p(`\nzbar reported a NON-QR symbol first ${res.zbarNonQrFirst.length}× (qr-decode.ts returns found[0] unfiltered): ${[...new Set(res.zbarNonQrFirst)].slice(0, 8).join(' · ')}`);
p();

if (sizes.length) {
	p(`## Download size`);
	p();
	p(`| what | raw | gzip -9 | brotli q11 |`);
	p(`|---|---:|---:|---:|`);
	for (const r of sizes) p(`| ${r.total ? `**${r.label}**` : r.label} | ${kb(r.raw)} | ${kb(r.gzip)} | ${kb(r.brotli)} |`);
	p();
}

const table = (mode, names) => {
	p(`| category | n | ${names.map((n) => LABEL[n]).join(' | ')} |`);
	p(`|---|---:|${names.map(() => '---:').join('|')}|`);
	for (const c of categories) {
		const xs = S.filter((s) => s.category === c);
		const cells = names.map((n) => {
			const h = xs.filter((s) => get(s, n, mode)?.status === 'hit').length;
			const w = xs.filter((s) => get(s, n, mode)?.status === 'wrong').length;
			return `${h}${w ? ` (${w}✗)` : ''}`;
		});
		p(`| ${c} | ${xs.length} | ${cells.join(' | ')} |`);
	}
	p();
};
p(`## Still image, by category (hits; ✗ = wrong text)`);
p();
table('still', NAMES);
p(`## Camera frame, by category`);
p();
table('camera', NAMES.filter((n) => n !== 'web'));

p(`## Head to head with today's web pipeline (still)`);
p();
p(`| decoder | reads what web misses | misses what web reads |`);
p(`|---|---:|---:|`);
for (const n of NAMES.filter((n) => n !== 'web')) {
	const gain = S.filter((s) => get(s, n, 'still').status === 'hit' && get(s, 'web', 'still').status !== 'hit');
	const loss = S.filter((s) => get(s, n, 'still').status !== 'hit' && get(s, 'web', 'still').status === 'hit');
	const by = (xs) => {
		const m = new Map();
		for (const s of xs) m.set(s.category, (m.get(s.category) ?? 0) + 1);
		return [...m].map(([c, k]) => `${c} ${k}`).join(', ');
	};
	p(`| ${LABEL[n]} | ${gain.length}${gain.length ? ` — ${by(gain)}` : ''} | ${loss.length}${loss.length ? ` — ${by(loss)}` : ''} |`);
}
p();

const wrongs = S.flatMap((s) =>
	NAMES.flatMap((n) =>
		['still', 'camera'].flatMap((m) => (get(s, n, m)?.status === 'wrong' ? [`- ${s.id} · ${LABEL[n]} ${m}: \`${get(s, n, m).got}\``] : []))
	)
);
if (wrongs.length) {
	p(`## Wrong reads`);
	p();
	wrongs.forEach((w) => p(w));
	p();
}

const md = lines.join('\n');
writeFileSync(path.join(LAB, 'out/report.md'), md);
console.log(md);
