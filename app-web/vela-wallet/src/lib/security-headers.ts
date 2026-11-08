/**
 * §35 (review 1-1): what wallet.getvela.app tells the browser about itself —
 * one list, read by the build's CSP (`vite.config.ts`, SvelteKit `kit.csp`),
 * the Worker (`hooks.server.ts`, for the responses it renders) and the
 * static assets' `_headers` (pinned to this list by `security-headers.test.ts`).
 *
 * The wallet holds the passkey ceremony for every getvela.app account, so a
 * script it did not ship, a page that frames it, or a feature it never uses
 * is a way in. Decided (docs/security/hardening.md):
 *
 * - **CSP, hash-based** (SvelteKit's `hash` mode: every page is prerendered,
 *   so a nonce cannot work; kit hashes its own inline scripts into a
 *   `<meta http-equiv>` per page, and `app.html`'s one render-blocking script
 *   is hashed by the config). No remote script origin at all: the wallet's
 *   code is its own. `'wasm-unsafe-eval'` because the core, PeerConnect and
 *   the QR decoder are WebAssembly. `connect-src` stays open to any `https:` /
 *   `wss:` host (and local http / ws): a person may point the wallet at their own
 *   RPC, relay, index or a site's network — that is the product, and the
 *   core's own rules (`network_admin::is_allowed_endpoint_url`) decide which.
 *   `style-src 'unsafe-inline'`: Svelte renders `style:` bindings into the
 *   prerendered HTML; styles cannot run code.
 * - **No framing**: `frame-ancestors 'none'` (a header — a `<meta>` CSP
 *   cannot carry it) and `X-Frame-Options: DENY` for browsers before CSP 2.
 * - **Permissions-Policy**: only what the wallet uses, for itself — passkeys
 *   (`publickey-credentials-get` / `-create`), the camera (QR), the clipboard
 *   (copy), sharing and the screen wake lock (the connect page); everything
 *   else off, for every frame.
 * - **`Referrer-Policy: no-referrer`**, **`X-Content-Type-Options: nosniff`**.
 * - **No `Cross-Origin-Opener-Policy: same-origin`**: the signing page is a
 *   window the wallet opens and answers by `postMessage` (`page-handoff`),
 *   which needs `window.opener`.
 */

/** The CSP's directives, as SvelteKit's `kit.csp.directives` takes them. */
export const CSP_DIRECTIVES = {
	'default-src': ['self'],
	// + kit's hashes of its own inline scripts, + app.html's (vite.config.ts).
	'script-src': ['self', 'wasm-unsafe-eval'],
	'style-src': ['self', 'unsafe-inline'],
	'img-src': ['self', 'data:', 'blob:', 'https:'],
	'font-src': ['self', 'data:'],
	// Local http and ws: a person's own node or relay on this machine (the
	// core allows plain http to localhost only), and the dev harness's.
	'connect-src': [
		'self',
		'https:',
		'wss:',
		'http://localhost:*',
		'http://127.0.0.1:*',
		'ws://localhost:*',
		'ws://127.0.0.1:*'
	],
	'media-src': ['self', 'blob:'],
	'worker-src': ['self'],
	'frame-src': ['none'],
	'object-src': ['none'],
	'base-uri': ['self'],
	'form-action': ['self'],
	'manifest-src': ['self'],
	// A header only (kit leaves it out of the <meta>): see FRAME_CSP.
	'frame-ancestors': ['none']
} as const;

/**
 * The CSP every response carries as a HEADER — what a `<meta>` cannot say
 * (`frame-ancestors`) and what holds for any response, HTML or not.
 */
export const FRAME_CSP = "frame-ancestors 'none'; object-src 'none'; base-uri 'self'";

/** `Permissions-Policy`: the wallet's own features for itself, the rest off. */
export const PERMISSIONS_POLICY = [
	'publickey-credentials-get=(self)',
	'publickey-credentials-create=(self)',
	'camera=(self)',
	'clipboard-write=(self)',
	'web-share=(self)',
	'screen-wake-lock=(self)',
	'microphone=()',
	'geolocation=()',
	'payment=()',
	'usb=()',
	'serial=()',
	'hid=()',
	'bluetooth=()',
	'midi=()',
	'display-capture=()',
	'accelerometer=()',
	'gyroscope=()',
	'magnetometer=()',
	'browsing-topics=()'
].join(', ');

/** Every header the wallet sends, on every response. */
export const WALLET_HEADERS: Readonly<Record<string, string>> = {
	'Content-Security-Policy': FRAME_CSP,
	'X-Frame-Options': 'DENY',
	'Permissions-Policy': PERMISSIONS_POLICY,
	'Referrer-Policy': 'no-referrer',
	'X-Content-Type-Options': 'nosniff'
};

/**
 * Put {@link WALLET_HEADERS} on a response. A CSP the response already has
 * (kit's, for a page the Worker renders — it carries `frame-ancestors` too)
 * is kept, with {@link FRAME_CSP} appended when it lacks `frame-ancestors`.
 * A response whose headers are immutable is rebuilt around the same body.
 */
export function withWalletHeaders(response: Response): Response {
	const apply = (headers: Headers) => {
		for (const [name, value] of Object.entries(WALLET_HEADERS)) {
			if (name === 'Content-Security-Policy') {
				const existing = headers.get(name);
				if (!existing) headers.set(name, value);
				else if (!/frame-ancestors/.test(existing)) headers.set(name, `${existing}; ${value}`);
				continue;
			}
			headers.set(name, value);
		}
	};
	try {
		apply(response.headers);
		return response;
	} catch {
		const rebuilt = new Response(response.body, response);
		apply(rebuilt.headers);
		return rebuilt;
	}
}
