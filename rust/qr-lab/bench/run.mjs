// Runs every decoder over the corpus and writes out/results.json.
//
//   node bench/run.mjs [--only <category>] [--limit <n per category>]
//
// Three ways a decoder is asked, per sample:
//
//   still   a picked image: the web ladder (widths 1200..400 below the
//           image's), then the image itself; each try plain, then inverted
//           (jsqr inverts itself). First non-null answer wins — right or wrong,
//           which is what a person would be shown.
//   camera  one frame: the image drawn 1000 wide, plain then inverted.
//   web     (zbar+jsqr only) `decodeImage` from src/lib/services/qr-decode.ts,
//           replayed step for step: zbar over the ladder, then jsqr with the
//           three transforms over [width, ...ladder].
//
// Only decode calls are timed (copy into the decoder included); building
// inverted/RGBA inputs is not, since a browser canvas hands those over.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import path from 'node:path';
import {
	LAB,
	zbarDecoder,
	jsqrDecoder,
	rustDecoders,
	toRgba,
	INVERT,
	BINARIZE,
	BIN_INVERT
} from './decoders.mjs';

const args = process.argv.slice(2);
const opt = (name) => {
	const i = args.indexOf(name);
	return i >= 0 ? args[i + 1] : undefined;
};
const only = opt('--only');
const limit = opt('--limit') ? Number(opt('--limit')) : Infinity;
// --decoders a,b: only these (web and zbar/jsqr are named zbar, jsqr, web).
const pick = opt('--decoders')?.split(',');

const corpusDir = path.join(LAB, 'out/corpus');
const manifest = JSON.parse(readFileSync(path.join(corpusDir, 'manifest.json'), 'utf8'));
const load = (file) => new Uint8Array(gunzipSync(readFileSync(path.join(corpusDir, file))));

const rust = rustDecoders(path.join(LAB, 'out/wasm'), pick);
const decoders = [zbarDecoder, jsqrDecoder, ...rust].filter((d) => !pick || pick.includes(d.name));
const withWeb = !pick || pick.includes('web');

/** A variant with lazily built inputs (never timed). */
function variant(w, h, gray) {
	let inv, rgba;
	return {
		w,
		h,
		gray,
		get inverted() {
			if (!inv) {
				inv = new Uint8Array(gray.length);
				for (let i = 0; i < gray.length; i++) inv[i] = 255 - gray[i];
			}
			return inv;
		},
		get rgba() {
			return (rgba ??= toRgba(gray));
		}
	};
}

async function chain(dec, variants, expected) {
	let calls = 0;
	let ms = 0;
	let firstMs = null;
	for (const v of variants) {
		const tries = dec.needsRgba ? [[v.rgba, '']] : dec.invertsItself ? [[v.gray, '']] : [[v.gray, ''], [v.inverted, 'i']];
		for (const [input, tag] of tries) {
			const t0 = performance.now();
			const text = await dec.decode(input, v.w, v.h);
			const dt = performance.now() - t0;
			calls++;
			ms += dt;
			firstMs ??= dt;
			if (text) {
				// '' is "no code": qr-decode.ts's `if (found)` skips it too.
				const hit = text === expected;
				return { status: hit ? 'hit' : 'wrong', ...(hit ? {} : { got: text.slice(0, 120) }), at: `${v.w}${tag}`, calls, ms, firstMs };
			}
		}
	}
	return { status: 'miss', calls, ms, firstMs };
}

/** decodeImage(), replayed. */
async function webCurrent(ladder, original, expected) {
	let calls = 0;
	let ms = 0;
	const done = (text, at) => ({ status: text === expected ? 'hit' : 'wrong', ...(text === expected ? {} : { got: text.slice(0, 120) }), at, calls, ms });
	for (const v of ladder) {
		for (const [input, tag] of [[v.gray, ''], [v.inverted, 'i']]) {
			const t0 = performance.now();
			const text = await zbarDecoder.decode(input, v.w, v.h);
			ms += performance.now() - t0;
			calls++;
			if (text) return done(text, `zbar@${v.w}${tag}`);
		}
	}
	const transforms = [
		['bin-invert', BIN_INVERT(160)],
		['invert', INVERT],
		['binarize', BINARIZE(160)]
	];
	for (const v of [original, ...ladder]) {
		for (const [name, transform] of transforms) {
			const data = v.rgba.slice();
			transform(data);
			const t0 = performance.now();
			const text = jsqrDecoder.raw(data, v.w, v.h);
			ms += performance.now() - t0;
			calls++;
			if (text) return done(text, `jsqr@${v.w}:${name}`);
		}
	}
	return { status: 'miss', calls, ms };
}

const out = [];
const perCategory = new Map();
const started = performance.now();
let lastCategory = null;
for (const s of manifest.samples) {
	if (only && s.category !== only) continue;
	const seen = perCategory.get(s.category) ?? 0;
	if (seen >= limit) continue;
	perCategory.set(s.category, seen + 1);
	if (s.category !== lastCategory) {
		process.stderr.write(`\n${s.category} `);
		lastCategory = s.category;
	}

	const original = variant(s.width, s.height, load(s.file));
	const ladder = s.ladder.map((v) => variant(v.w, v.h, load(v.file)));
	const cam =
		ladder.find((v) => v.w === s.camera.w && v.h === s.camera.h) ??
		(s.camera.file === s.file ? original : variant(s.camera.w, s.camera.h, load(s.camera.file)));

	const results = {};
	for (const dec of decoders) {
		results[dec.name] = {
			still: await chain(dec, [...ladder, original], s.expected),
			camera: await chain(dec, [cam], s.expected)
		};
	}
	if (withWeb) results.web = { still: await webCurrent(ladder, original, s.expected) };
	// The new web scanner (qr-decode.ts on vela-qr): a picked image at most 1200
	// wide, one call — the crate inverts and rescales itself; a frame 1000 wide.
	const vela = decoders.find((d) => d.name === 'vela');
	if (vela) {
		const once = { name: 'vela-web', invertsItself: true, decode: vela.decode.bind(vela) };
		const still = ladder.find((v) => v.w === 1200) ?? original;
		results['vela-web'] = { still: await chain(once, [still], s.expected), camera: await chain(once, [cam], s.expected) };
	}
	out.push({ id: s.id, category: s.category, kind: s.payload_kind, ec: s.ec, width: s.width, height: s.height, results });
	const mark = Object.values(results)[0]?.still.status;
	process.stderr.write(mark === 'hit' ? '.' : mark === 'wrong' ? '!' : 'x');
}
process.stderr.write(`\n${out.length} samples in ${((performance.now() - started) / 1000).toFixed(1)}s\n`);

// Merge into earlier results, so one new contestant can run alone.
const file = path.join(LAB, 'out/results.json');
const prev = existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : { samples: [], crashes: {} };
const byId = new Map(prev.samples.map((s) => [s.id, s]));
for (const s of out) {
	const old = byId.get(s.id);
	byId.set(s.id, old ? { ...old, results: { ...old.results, ...s.results } } : s);
}
const order = manifest.samples.map((s) => s.id).filter((id) => byId.has(id));
writeFileSync(
	file,
	JSON.stringify(
		{
			node: process.version,
			date: new Date().toISOString(),
			corpus: { seed: manifest.seed, per_category: manifest.per_category, resize_filter: manifest.resize_filter },
			crashes: { ...prev.crashes, ...Object.fromEntries(rust.map((d) => [d.name, d.crashes])) },
			zbarNonQrFirst: decoders.includes(zbarDecoder) ? zbarDecoder.nonQrFirst : (prev.zbarNonQrFirst ?? []),
			samples: order.map((id) => byId.get(id))
		},
		null,
		1
	)
);
