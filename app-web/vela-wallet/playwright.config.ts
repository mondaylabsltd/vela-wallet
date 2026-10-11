import { defineConfig } from '@playwright/test';

const STORAGE_SUITES = [
	'**/*persistence*.e2e.ts',
	'**/pool-resilience.e2e.ts',
	'**/home-truth.e2e.ts',
	'**/reopen-pending.e2e.ts',
	'**/parallel-entry.e2e.ts',
	// 028's preferences: what a person chose has to survive a reload on every
	// engine, and an erase has to leave nothing behind on any of them.
	'**/preferences.e2e.ts',
	// 072's wide settings: a saved provider key survives its own panel, and a
	// cleared row or an erase takes what it said and nothing else.
	'**/settings-desktop.e2e.ts',
	// A failed lazy import: the tab's memory of the build it recovered from is
	// session storage — and the cancelled imports of a page being left, which
	// it must ignore, are reported by Firefox alone.
	'**/chunk-recovery.e2e.ts'
];

export default defineConfig({
	webServer: {
		// The extension suite reads the PACKAGED artifact, so building it is part
		// of standing the suite up — not a step someone has to remember (spec 027).
		// The server is `e2e/preview.mjs` — the deploy bundle on workerd — and not
		// `npm run preview`: `wrangler dev` exits when one request it forwards is
		// lost, and takes the rest of the run with it (the reasons are in that file).
		command: 'npm run build && npm run build:extension && node e2e/preview.mjs 4173',
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
	 *
	 * The third time found what "dies" was: `wrangler dev`'s proxy ending the
	 * session over one lost forward (`e2e/preview.mjs`). The suites no longer run
	 * behind that proxy. The server is still one single-threaded `workerd`, and
	 * the pin stays — nothing here was measured again with more workers.
	 */
	workers: 3,
	/**
	 * How long a WAIT may last — not how long anything should take. No test
	 * asserts a duration with these: an `expect` still passes the instant its
	 * condition holds, so on a quiet machine nothing here changes a run.
	 *
	 * Playwright's defaults (5 s for an expectation, 30 s for a test) assume
	 * the machine is the suite's own. This one builds three other things at
	 * the same time, and the wallet's first paint after a navigation — the
	 * core is 5 MB of wasm — then takes six to thirteen seconds. Measured on
	 * one commit, the whole suite at `--retries=0`:
	 *
	 *   load  8–21:    448 passed
	 *   load 22–63:    10 failed — every one "Timeout: 5000ms" on a default wait
	 *   load 26–185:   30 failed — 26 of them that, 3 a test's own time running out
	 *
	 * A red run that means "the machine was busy" teaches people to rerun a red
	 * run. A wait that needs longer than this says so with its own `timeout`.
	 */
	expect: { timeout: 20_000 },
	timeout: 90_000,
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
		//
		// Firefox is told to use NO proxy. Playwright's Firefox lets a proxy take
		// `localhost` (`network.proxy.allow_hijacking_localhost`), and it reads
		// the machine's proxy settings: on a Mac with an HTTP proxy set and no
		// exception for localhost, every `page.goto` to the suites' own server
		// went to that proxy and came back `NS_ERROR_NET_RESET` — all 38 Firefox
		// tests, the morning somebody changed the Mac's network settings.
		// Chromium and WebKit never proxy localhost. A suite that depends on what
		// another host answers stubs it; none needs the proxy to reach one.
		{
			name: 'firefox',
			use: {
				browserName: 'firefox',
				launchOptions: { firefoxUserPrefs: { 'network.proxy.type': 0 } }
			},
			testMatch: STORAGE_SUITES
		},
		{ name: 'webkit', use: { browserName: 'webkit' }, testMatch: STORAGE_SUITES }
	]
});
