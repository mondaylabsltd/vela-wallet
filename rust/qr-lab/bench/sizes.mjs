// Download cost of each contestant, measured on what would ship:
//   - zbar + jsqr: the chunks the web app's last build actually emitted
//     (found by the same markers e2e/budgets.e2e.ts uses);
//   - the Rust shims: wasm-opt'd modules from out/wasm.
// brotli = q11 with the largest window, as app-web scripts/wasm-brotli.mjs ships.
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { brotliCompressSync, gzipSync, constants } from 'node:zlib';
import path from 'node:path';
import { LAB, APP } from './decoders.mjs';

const br = (b) =>
	brotliCompressSync(b, {
		params: {
			[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
			[constants.BROTLI_PARAM_LGWIN]: constants.BROTLI_MAX_WINDOW_BITS,
			[constants.BROTLI_PARAM_SIZE_HINT]: b.length
		}
	}).length;
const measure = (label, files) => {
	const parts = files.map((f) => readFileSync(f));
	const sum = (fn) => parts.reduce((a, b) => a + fn(b), 0);
	return { label, files: files.map((f) => path.relative(LAB, f)), raw: sum((b) => b.length), gzip: sum((b) => gzipSync(b, { level: 9 }).length), brotli: sum(br) };
};

const rows = [];
const client = path.join(APP, '.svelte-kit/output/client/app/immutable');
if (existsSync(client)) {
	const chunks = readdirSync(path.join(client, 'chunks')).map((f) => path.join(client, 'chunks', f));
	const carrying = (re) => chunks.filter((f) => re.test(readFileSync(f, 'utf8')));
	const zbarWasm = readdirSync(path.join(client, 'assets')).filter((f) => /^zbar\..*\.wasm$/.test(f)).map((f) => path.join(client, 'assets', f));
	const zbarGlue = carrying(/zbar\.[A-Za-z0-9_-]+\.wasm/);
	const jsqr = carrying(/onlyInvert/);
	rows.push(measure('zbar (wasm + glue, shipped)', [...zbarWasm, ...zbarGlue]));
	rows.push(measure('jsqr (chunk, shipped)', jsqr));
	rows.push({ ...measure('web today: zbar + jsqr', [...zbarWasm, ...zbarGlue, ...jsqr]), total: true });
} else {
	console.warn('no app build at', client, '— run `bun run build` in app-web/vela-wallet for the baseline sizes');
}
for (const f of readdirSync(path.join(LAB, 'out/wasm')).filter((f) => f.endsWith('.wasm')).sort()) {
	rows.push(measure(f.replace(/\.wasm$/, ''), [path.join(LAB, 'out/wasm', f)]));
}
writeFileSync(path.join(LAB, 'out/sizes.json'), JSON.stringify(rows, null, 1));
console.table(rows.map(({ label, raw, gzip, brotli }) => ({ label, raw, gzip, brotli })));
