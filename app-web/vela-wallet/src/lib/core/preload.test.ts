/**
 * perf-3: the core's wasm is preloaded by the document on the routes that
 * boot it — and never on Welcome, which loads no wasm until someone commits
 * to a flow (`welcome-ssr.e2e.ts`, `budgets.e2e.ts`).
 */
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { CORE_ROUTES, corePreloadTag } from './preload';

const ROUTES = join(import.meta.dirname, '../../routes');

describe('corePreloadTag', () => {
	it('preloads the very artifact `loadCore` fetches, as the same kind of request', () => {
		const tag = corePreloadTag('/[locale]/wallet');
		expect(tag).toContain(`href="${WASM_URL}"`);
		// `fetch(url)` from wasm-bindgen's init: CORS mode, same-origin
		// credentials — a preload without `crossorigin` would not be reused.
		expect(tag).toContain('rel="preload"');
		expect(tag).toContain('as="fetch"');
		expect(tag).toContain('crossorigin="anonymous"');
	});

	it('writes nothing on Welcome, the placeholder pages or an unknown route', () => {
		for (const id of [
			'/[locale]',
			'/[locale]/import',
			'/[locale]/gallery',
			'/[locale]/connect',
			null
		]) {
			expect(corePreloadTag(id), String(id)).toBe('');
		}
	});

	it('names only routes that exist', () => {
		for (const id of CORE_ROUTES) {
			expect(existsSync(join(ROUTES, id, '+page.svelte')), id).toBe(true);
		}
	});
});
