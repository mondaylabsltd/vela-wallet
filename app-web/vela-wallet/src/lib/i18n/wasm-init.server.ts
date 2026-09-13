/**
 * One-shot wasm initialization for every build-time vela-core consumer
 * (`engine.server.ts`, `wallet/identicon.server.ts`). Node module caching
 * makes this run exactly once per process; splitting it out keeps a second
 * consumer from calling `initSync` on an already-initialized module.
 *
 * The bytes come from the committed `assets/wasm/vela_core_bg.<fingerprint>.wasm`
 * asset named by `vela_core_wasm_url.js` — the D7 route that replaced the
 * base64 payload `rust/scripts/build-web.mjs` used to emit. Reading the file
 * (rather than importing it, which would inline 3.4 MB of base64 back into the
 * server bundle `e2e/welcome-ssr.e2e.ts` guards) keeps the module out of
 * anything that ships: only prerendering ever imports this file.
 */
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

/**
 * `import.meta.url` points inside `.svelte-kit/output/` once vite has bundled
 * the server, so the repo-relative depth this file sits at is not usable at
 * run time. Walk up from the working directory instead — every entry point
 * that reaches here (`vite build`, `vite dev`, vitest) runs inside the repo.
 */
function wasmPath(): string {
	const asset = WASM_URL.replace(/^\//, '');
	for (let dir = process.cwd(); ; dir = dirname(dir)) {
		const candidate = join(dir, 'assets', 'wasm', asset);
		if (existsSync(candidate)) return candidate;
		if (dirname(dir) === dir) break;
	}
	throw new Error(
		`assets/wasm/${asset} not found above ${process.cwd()} — run \`npm run build:wasm\` at the repo root`
	);
}

initSync({ module: readFileSync(wasmPath()) });
