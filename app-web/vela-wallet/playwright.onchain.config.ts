import { defineConfig, devices } from '@playwright/test';

/**
 * The multisig flows against a local Gnosis fork (`e2e/onchain/`): real
 * contracts, the real genesis service, a local relay. Needs
 * `scripts/onchain/multisig-fork/up.sh` and `bun run dev --port 5173` running;
 * not part of `bun run test:e2e`, which stays hermetic.
 */
export default defineConfig({
	testDir: 'e2e/onchain',
	testMatch: /.*\.onchain\.ts$/,
	fullyParallel: false,
	workers: 1,
	timeout: 240_000,
	// A cold fork pulls state from the public Gnosis RPC on first touch, and the
	// first test of a run can wait past its budget for a balance. One retry,
	// reported as flaky, not a pass that hides a failure.
	retries: 1,
	expect: { timeout: 30_000 },
	use: {
		baseURL: process.env.VELA_ONCHAIN_URL ?? 'http://127.0.0.1:5173',
		...devices['Desktop Chrome'],
		viewport: { width: 430, height: 900 },
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure'
	},
	outputDir: 'e2e/onchain/.results'
});
