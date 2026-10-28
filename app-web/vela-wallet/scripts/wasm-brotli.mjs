/**
 * The hosted build's last step (perf-4): the core's wasm at brotli 11, and the
 * Worker that serves it (`src/lib/core/wasm-brotli.ts`).
 *
 * Run by `vite.config.ts` right after `@sveltejs/adapter-cloudflare` has
 * written its output, so every `vite build` of the site — a developer's,
 * CI's, Cloudflare's own — carries it. The extension build uses the static
 * adapter and never comes here: it ships the plain wasm as a file.
 *
 * 1. `<assets>/vela_core_bg.<hash>.wasm.br` beside the plain file. Quality 11
 *    with the largest window browsers decode (2^24) is tens of seconds, so
 *    the result is kept under `node_modules/.cache/` and reused while it still
 *    decompresses to exactly these bytes (the name fingerprints the SOURCE,
 *    not the bytes — the check is on the bytes).
 * 2. The Worker. The adapter writes its worker where `wrangler.jsonc`'s
 *    `main` points and serves every static file itself, before SvelteKit's
 *    hooks run — so the wasm path cannot be answered inside it. Its worker is
 *    moved aside (`_sveltekit_worker.js`, kept out of the uploaded assets)
 *    and `main` becomes a two-line module that answers the wasm path and
 *    hands everything else to it.
 */
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { promisify } from 'node:util';
import { brotliCompress, brotliDecompressSync, constants } from 'node:zlib';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const REPO = resolve(APP, '../..');
const CACHE = join(APP, 'node_modules', '.cache', 'vela-wasm-brotli');
const SVELTEKIT_WORKER = '_sveltekit_worker.js';
const MARKER = '// Written by scripts/wasm-brotli.mjs: the core wasm at brotli 11, then SvelteKit.';

/** `WASM_URL`'s file name, as `sync-wasm.mjs` reads it. */
function wasmName() {
	const source = readFileSync(join(REPO, 'rust/pkg-web/vela_core_wasm_url.js'), 'utf8');
	const match = /export const WASM_URL = '\/([^']+)'/.exec(source);
	if (!match) throw new Error('wasm-brotli: could not read WASM_URL from vela_core_wasm_url.js');
	return match[1];
}

/** Brotli 11, window 2^24: the most a browser decodes. */
export async function compressMax(bytes) {
	return promisify(brotliCompress)(bytes, {
		params: {
			[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
			[constants.BROTLI_PARAM_LGWIN]: constants.BROTLI_MAX_WINDOW_BITS,
			[constants.BROTLI_PARAM_SIZE_HINT]: bytes.length
		}
	});
}

/** Write `<name>.br` beside the plain wasm in `assets`; returns both sizes. */
export async function emitBrotliCore(assets, { cache = CACHE } = {}) {
	const name = wasmName();
	const plain = readFileSync(join(assets, name));
	const cached = join(cache, `${name}.br`);
	let encoded = null;
	if (existsSync(cached)) {
		const candidate = readFileSync(cached);
		try {
			if (brotliDecompressSync(candidate).equals(plain)) encoded = candidate;
		} catch {
			// A torn cache entry is made again.
		}
	}
	if (encoded === null) {
		encoded = await compressMax(plain);
		mkdirSync(cache, { recursive: true });
		writeFileSync(cached, encoded);
	}
	writeFileSync(join(assets, `${name}.br`), encoded);
	return { name, plain: plain.length, br: encoded.length };
}

/** Put the wasm-answering module where `main` points, in front of the adapter's worker. */
export function wrapWorker(main) {
	const dir = dirname(main);
	// Already this module (wrapped twice over one output): moving it aside
	// would make it import itself.
	if (readFileSync(main, 'utf8').startsWith(MARKER)) return;
	renameSync(main, join(dir, SVELTEKIT_WORKER));
	const handler = relative(dir, join(APP, 'src/lib/core/wasm-brotli.ts')).split('\\').join('/');
	writeFileSync(
		main,
		[
			MARKER,
			`import sveltekit from './${SVELTEKIT_WORKER}';`,
			`import { withBrotliCore } from '${handler}';`,
			'export default withBrotliCore(sveltekit);',
			''
		].join('\n')
	);
	// The adapter's own worker is server code: never an uploaded asset.
	const ignore = join(dir, '.assetsignore');
	const listed = existsSync(ignore) ? readFileSync(ignore, 'utf8') : '';
	if (!listed.split('\n').includes(SVELTEKIT_WORKER)) {
		writeFileSync(ignore, `${listed.replace(/\n?$/, '\n')}${SVELTEKIT_WORKER}\n`);
	}
}

/**
 * `adapter`, followed by both steps. `main` and `assets` are `wrangler.jsonc`'s
 * (the adapter reads the same file), resolved against the app.
 */
export function withBrotliCoreAdapter(adapter) {
	return {
		...adapter,
		async adapt(builder) {
			await adapter.adapt(builder);
			const config = JSON.parse(
				readFileSync(join(APP, 'wrangler.jsonc'), 'utf8').replace(/^\s*\/\/.*$/gm, '')
			);
			const { name, plain, br } = await emitBrotliCore(join(APP, config.assets.directory));
			wrapWorker(join(APP, config.main));
			console.log(
				`wasm-brotli: ${name}.br ${(br / 1024 / 1024).toFixed(2)} MiB (plain ${(plain / 1024 / 1024).toFixed(2)} MiB)`
			);
		}
	};
}
