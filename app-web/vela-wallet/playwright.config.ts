import { defineConfig } from '@playwright/test';

const STORAGE_SUITES = [
	'**/*persistence*.e2e.ts',
	'**/pool-resilience.e2e.ts',
	'**/home-truth.e2e.ts',
	'**/reopen-pending.e2e.ts',
	'**/parallel-entry.e2e.ts',
	// 028's preferences: what a person chose has to survive a reload on every
	// engine, and an erase has to leave nothing behind on any of them.
	'**/preferences.e2e.ts'
];

export default defineConfig({
	webServer: {
		// The extension suite reads the PACKAGED artifact, so building it is part
		// of standing the suite up — not a step someone has to remember (spec 027).
		command: 'npm run build && npm run build:extension && npm run preview',
		port: 4173,
		timeout: 180_000,
		reuseExistingServer: true
	},
	use: { baseURL: 'http://localhost:4173' },
	/**
	 * The preview is ONE `workerd` process, and it is single-threaded.
	 *
	 * Spec 026 already found it starving under six parallel workers (two budget
	 * suites re-fetching every chunk; 27 unrelated tests went red). Spec 027 hit
	 * the same wall from the other side: the extension suite launches a SEVENTH
	 * browser and hands it a 32 MB unpacked extension, and the preview died
	 * mid-run — again taking down suites that had nothing to do with it.
	 *
	 * Measured: 6 workers → the preview dies; 3 workers → 123/123, twelve seconds
	 * slower. Pinning it is cheaper than re-diagnosing this a third time.
	 */
	workers: 3,
	testMatch: '**/*.e2e.{ts,js}',
	projects: [
		// Everything runs on chromium…
		{ name: 'chromium', use: { browserName: 'chromium' } },
		// …and the suites that assert STORAGE additionally prove IndexedDB on the
		// other two engines: the 024 persistence pair (SC-001/SC-002), the 025
		// read suites that reload on a persisted ban map / privacy flag (T150),
		// and the 026 money pair (T261) — the pending record a closed tab left in
		// IndexedDB, settled on the next boot (SC-204), and the wallet SWAP the
		// parallel space performs in localStorage and gives back byte-for-byte.
		// Money that survives a crash is the promise that must not be one
		// engine's.
		{ name: 'firefox', use: { browserName: 'firefox' }, testMatch: STORAGE_SUITES },
		{ name: 'webkit', use: { browserName: 'webkit' }, testMatch: STORAGE_SUITES }
	]
});
