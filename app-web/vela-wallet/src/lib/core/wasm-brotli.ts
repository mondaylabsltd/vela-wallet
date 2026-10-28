/**
 * The core's wasm at brotli 11, answered by the deployed Worker (perf-4).
 *
 * Workers static assets serve a file as it is and compress it on the way out
 * at whatever level the edge picks; they cannot serve a pre-compressed copy.
 * The 5.9 MB core went out as ~1.9 MB of gzip, and on a Slow 4G phone that
 * download IS the wait before the wallet draws (perf-3). Brotli at quality 11
 * makes it ~1.33 MB, but only offline — q11 takes tens of seconds, which is
 * why no edge does it per request.
 *
 * So the build writes that copy beside the wasm (`<name>.wasm.br`,
 * `scripts/wasm-brotli.mjs`), `wrangler.jsonc` sends the one path to the
 * Worker first (`run_worker_first`), and this answers it: the stored bytes,
 * `Content-Encoding: br`, `encodeBody: 'manual'` so the runtime passes them on
 * rather than encoding them again. `Content-Type: application/wasm` is what
 * lets the browser compile while it downloads (`instantiateStreaming`).
 *
 * A request that does not take br, a method other than GET or HEAD, or a
 * build without the copy falls through to SvelteKit's worker, which serves
 * the plain file from the assets as before. (The edge negotiates with the
 * client last: it decodes for one that cannot take br — Miniflare, which
 * `e2e/preview.mjs` runs, re-encodes to the first coding the client lists.)
 * The browser extension never comes here: it ships the plain wasm as a file.
 */
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

/** Where the build puts the brotli copy, beside the plain wasm. */
export const WASM_BR_URL = `${WASM_URL}.br`;

/** The fingerprinted name never changes its bytes: the same rule as `_headers`. */
const IMMUTABLE = 'public, immutable, max-age=31536000';

interface AssetsEnv {
	ASSETS: { fetch: (input: Request | string) => Promise<Response> };
}

interface Worker<E> {
	fetch(request: Request, env: E, ctx: unknown): Promise<Response>;
}

/**
 * Does this `Accept-Encoding` take br? A `q=0` on br — or on `*` with br not
 * named — refuses it (RFC 9110 §12.5.3).
 */
export function acceptsBrotli(header: string | null): boolean {
	if (!header) return false;
	let star: boolean | null = null;
	for (const part of header.split(',')) {
		const [coding, ...params] = part.trim().toLowerCase().split(';');
		const q = params.map((p) => p.trim()).find((p) => p.startsWith('q='));
		const allowed = q === undefined || Number(q.slice(2)) > 0;
		if (coding.trim() === 'br') return allowed;
		if (coding.trim() === '*') star = allowed;
	}
	return star === true;
}

/** `worker`, with the core's wasm answered from its brotli copy where it can be. */
export function withBrotliCore<E extends AssetsEnv>(worker: Worker<E>): Worker<E> {
	return {
		async fetch(request, env, ctx) {
			const url = new URL(request.url);
			if (
				url.pathname === WASM_URL &&
				(request.method === 'GET' || request.method === 'HEAD') &&
				acceptsBrotli(request.headers.get('accept-encoding'))
			) {
				const stored = await env.ASSETS.fetch(new Request(new URL(WASM_BR_URL, url)));
				if (stored.ok && stored.body !== null) {
					const headers = new Headers({
						'Content-Type': 'application/wasm',
						'Content-Encoding': 'br',
						'Cache-Control': IMMUTABLE,
						Vary: 'Accept-Encoding'
					});
					const length = stored.headers.get('content-length');
					if (length !== null && !stored.headers.has('content-encoding')) {
						headers.set('Content-Length', length);
					}
					const etag = stored.headers.get('etag');
					if (etag !== null) headers.set('ETag', etag);
					if (request.method === 'HEAD') await stored.body.cancel();
					return new Response(request.method === 'HEAD' ? null : stored.body, {
						status: 200,
						headers,
						// Cloudflare's own: send the bytes as they are, already encoded.
						encodeBody: 'manual'
					} as ResponseInit);
				}
				await stored.body?.cancel();
			}
			return worker.fetch(request, env, ctx);
		}
	};
}
