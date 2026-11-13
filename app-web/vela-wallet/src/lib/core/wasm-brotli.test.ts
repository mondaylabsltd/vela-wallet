/**
 * perf-4: the Worker answers the core's wasm from its brotli-11 copy, and the
 * build writes that copy and puts the answering module in front of
 * SvelteKit's worker.
 */
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { brotliDecompressSync } from 'node:zlib';
import { describe, expect, it, vi } from 'vitest';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { acceptsBrotli, WASM_BR_URL, withBrotliCore } from './wasm-brotli';
import { emitBrotliCore, wrapWorker } from '../../../scripts/wasm-brotli.mjs';

const ORIGIN = 'https://wallet.example';
const BR_BYTES = new Uint8Array([1, 2, 3, 4, 5]);

function harness(stored: Response | null = new Response(BR_BYTES)) {
	const assets = vi.fn(async (input: Request | string) => {
		const url = new URL(typeof input === 'string' ? input : input.url);
		if (url.pathname === WASM_BR_URL && stored !== null) return stored;
		return new Response(null, { status: 404 });
	});
	const inner = vi.fn(async () => new Response('from sveltekit'));
	const worker = withBrotliCore({ fetch: inner });
	const env = { ASSETS: { fetch: assets } };
	const get = (path: string, headers: Record<string, string> = {}, method = 'GET') =>
		worker.fetch(new Request(`${ORIGIN}${path}`, { method, headers }), env, {});
	return { assets, inner, get };
}

describe('acceptsBrotli', () => {
	it('reads what a browser sends', () => {
		expect(acceptsBrotli('gzip, deflate, br, zstd')).toBe(true);
		expect(acceptsBrotli('br;q=0.5, gzip')).toBe(true);
		expect(acceptsBrotli('*')).toBe(true);
	});
	it('takes no br where none is offered or it is refused', () => {
		expect(acceptsBrotli(null)).toBe(false);
		expect(acceptsBrotli('')).toBe(false);
		expect(acceptsBrotli('gzip, deflate')).toBe(false);
		expect(acceptsBrotli('br;q=0, gzip')).toBe(false);
		expect(acceptsBrotli('*;q=0')).toBe(false);
		expect(acceptsBrotli('br;q=0, *')).toBe(false);
	});
});

describe('withBrotliCore', () => {
	it('answers the wasm with the stored br bytes, typed for streaming compile', async () => {
		const { get, inner, assets } = harness();
		const response = await get(WASM_URL, { 'accept-encoding': 'gzip, deflate, br, zstd' });
		expect(response.status).toBe(200);
		expect(response.headers.get('content-type')).toBe('application/wasm');
		expect(response.headers.get('content-encoding')).toBe('br');
		expect(response.headers.get('cache-control')).toBe('public, immutable, max-age=31536000');
		expect(response.headers.get('vary')).toBe('Accept-Encoding');
		expect(new Uint8Array(await response.arrayBuffer())).toEqual(BR_BYTES);
		expect(inner).not.toHaveBeenCalled();
		expect(new URL((assets.mock.calls[0][0] as Request).url).pathname).toBe(WASM_BR_URL);
	});

	it('answers HEAD with the same headers and no body', async () => {
		const { get } = harness();
		const response = await get(WASM_URL, { 'accept-encoding': 'br' }, 'HEAD');
		expect(response.headers.get('content-encoding')).toBe('br');
		expect(response.body).toBeNull();
	});

	it('hands everything else to SvelteKit: no br, another path, another method, no copy', async () => {
		for (const [path, headers, method, stored] of [
			[WASM_URL, { 'accept-encoding': 'gzip' }, 'GET', undefined],
			[WASM_URL, {}, 'GET', undefined],
			['/en/wallet', { 'accept-encoding': 'br' }, 'GET', undefined],
			[WASM_URL, { 'accept-encoding': 'br' }, 'POST', undefined],
			[WASM_URL, { 'accept-encoding': 'br' }, 'GET', null]
		] as const) {
			const { get, inner } = harness(stored === null ? null : undefined);
			const response = await get(path, headers, method);
			expect(await response.text(), `${method} ${path}`).toBe('from sveltekit');
			expect(inner).toHaveBeenCalledOnce();
		}
	});
});

describe('the build step', () => {
	it('writes a br copy that decodes to the wasm, and reuses only a cache that still does', async () => {
		const dir = mkdtempSync(join(tmpdir(), 'vela-wasm-br-'));
		const assets = join(dir, 'assets');
		const cache = join(dir, 'cache');
		mkdirSync(assets);
		const name = WASM_URL.slice(1);
		const plain = Buffer.from('\0asm'.repeat(4096));
		writeFileSync(join(assets, name), plain);

		const first = await emitBrotliCore(assets, { cache });
		expect(first).toMatchObject({ name, plain: plain.length });
		expect(brotliDecompressSync(readFileSync(join(assets, `${name}.br`)))).toEqual(plain);

		// New bytes under the same (source-fingerprinted) name: the cache is stale.
		const changed = Buffer.from('\0asm-changed'.repeat(4096));
		writeFileSync(join(assets, name), changed);
		await emitBrotliCore(assets, { cache });
		expect(brotliDecompressSync(readFileSync(join(assets, `${name}.br`)))).toEqual(changed);
	});

	it('puts the answering module where main points and keeps the adapter worker out of the assets', () => {
		const dir = mkdtempSync(join(tmpdir(), 'vela-wasm-worker-'));
		const main = join(dir, '_worker.js');
		writeFileSync(main, '// adapter');
		writeFileSync(join(dir, '.assetsignore'), '_worker.js\n_headers');
		wrapWorker(main);
		wrapWorker(main); // twice over one output: still in front of the adapter's worker
		expect(readFileSync(join(dir, '_sveltekit_worker.js'), 'utf8')).toBe('// adapter');
		// The next build: the adapter writes its worker again, and it is wrapped again.
		writeFileSync(main, '// adapter, rebuilt');
		wrapWorker(main);
		expect(readFileSync(join(dir, '_sveltekit_worker.js'), 'utf8')).toBe('// adapter, rebuilt');
		const wrapper = readFileSync(main, 'utf8');
		expect(wrapper).toContain("import sveltekit from './_sveltekit_worker.js'");
		expect(wrapper).toContain('withBrotliCore(sveltekit)');
		const ignored = readFileSync(join(dir, '.assetsignore'), 'utf8').split('\n');
		expect(ignored.filter((line) => line === '_sveltekit_worker.js')).toHaveLength(1);
		expect(ignored).toContain('_headers');
	});
});
