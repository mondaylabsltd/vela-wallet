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

/**
 * 4174 — or `VELA_E2E_PORT`, for a machine where a second session already has
 * 4174 (several worktrees are tested at once here). The server is never
 * reused: a port somebody else holds fails the run at once, instead of the
 * suites quietly running against another worktree's build.
 */
const PORT = Number(process.env.VELA_E2E_PORT ?? 4174);

export default {
	...base,
	webServer: {
		...base.webServer,
		command: `npm run build && npm run build:extension && node e2e/preview.mjs ${PORT}`,
		port: PORT,
		timeout: 600_000,
		reuseExistingServer: false
	},
	use: { ...base.use, baseURL: `http://localhost:${PORT}` }
};
