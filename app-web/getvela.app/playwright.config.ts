import { defineConfig } from '@playwright/test';

/**
 * The preview server is `wrangler dev`, so the port must match what wrangler
 * actually listens on — the previous config waited on 4173, which nothing here
 * ever serves, so these tests could not have run. 8788 is this project's own
 * port: 4173 is shared with `app-web/vela-wallet`'s e2e run, and two sessions
 * on one port silently test each other's build.
 *
 * `reuseExistingServer: false` for the same reason — always build and serve the
 * tree under test, never whatever happens to be listening.
 */
const PORT = 8788;

export default defineConfig({
	webServer: {
		command: `bun run build && bunx wrangler dev --port ${PORT}`,
		port: PORT,
		reuseExistingServer: false,
		timeout: 180_000
	},
	use: { baseURL: `http://localhost:${PORT}` },
	testMatch: '**/*.e2e.{ts,js}'
});
