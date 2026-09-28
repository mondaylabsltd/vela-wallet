import base from './playwright.config';

/**
 * The e2e suites on a port of their own (4174, never another session's 4173)
 * and — for the extension — a PACKAGE of their own (spec 082 RJ21, G66).
 *
 * The regular config builds the extension into `extension/dist`, which is what
 * a running browser has loaded (the device pass's Chrome for Testing, or a
 * person's own). Rebuilding it underneath broke that browser: its open side
 * panel failed its next navigation (ERR_FILE_NOT_FOUND). Here the build goes to
 * `VELA_EXTENSION_DIST` — read by `vite.config.ts`, `extension/build.mjs` and
 * `e2e/extension-helpers.ts` — and `extension/dist` is never touched. Set in
 * this process before the web server starts, so the build inherits it, and
 * before the workers start, so they load the same package.
 */
export const ISOLATED_EXTENSION_DIST = 'node_modules/.cache/vela-extension-e2e';
process.env.VELA_EXTENSION_DIST ||= ISOLATED_EXTENSION_DIST;

export default {
	...base,
	webServer: {
		...base.webServer,
		command:
			'npm run build && npm run build:extension && npx wrangler dev .svelte-kit/cloudflare/_worker.js --port 4174',
		port: 4174,
		timeout: 600_000,
		reuseExistingServer: false
	},
	use: { ...base.use, baseURL: 'http://localhost:4174' }
};
