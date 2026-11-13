import { defineConfig, devices } from '@playwright/test';

/**
 * Live tests on a public testnet (Arc Testnet): real chain, real relay,
 * accounts made with Chrome's virtual passkeys. Not part of any gate — they
 * spend testnet funds. Needs `bun run dev --port 5173` (opened as `localhost`,
 * a passkey's relying party cannot be an IP), a genesis service for the
 * chain (ARC_GENESIS), and a funded key (ARC_FUNDER_KEY, never committed).
 */
export default defineConfig({
	testDir: 'e2e/live',
	testMatch: /.*\.live\.ts$/,
	fullyParallel: false,
	workers: 1,
	timeout: 30 * 60_000,
	expect: { timeout: 60_000 },
	use: {
		baseURL: process.env.VELA_LIVE_URL ?? 'http://localhost:5173',
		...devices['Desktop Chrome'],
		viewport: { width: 430, height: 900 },
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure',
		// A step that cannot happen fails, rather than waiting out the test.
		actionTimeout: 60_000
	},
	outputDir: 'e2e/live/.results'
});
