/**
 * What getvela.app tells the browser about itself (spec 080; §35, review 1-1)
 * — one list, set on the Worker's responses by `hooks.server.ts` and on the
 * static assets by the project's `_headers` (pinned to this list by
 * `security-headers.test.ts`).
 *
 * - **Passkeys denied.** Passkeys are bound to getvela.app, so any page this
 *   origin serves could ask a visitor's keys for a signature, and the system
 *   prompt shows only "getvela.app", never the transaction. No page here uses
 *   WebAuthn (the wallet lives at wallet.getvela.app), so the site forbids
 *   itself, and every script it loads, from calling navigator.credentials. A
 *   signing page ever served from this origin must detach this header for its
 *   own path.
 * - **No framing**: `frame-ancestors 'none'` and `X-Frame-Options: DENY` — a
 *   page that holds the brand is never drawn inside someone else's.
 * - **`X-Content-Type-Options: nosniff`.**
 *
 * The other `*.getvela.app` hosts (relay, key index, ethereum-data, genesis,
 * currency) live in other repositories; the same passkey denial is a go-live
 * item there (docs/security/hardening.md).
 */

export const SITE_PERMISSIONS_POLICY =
	'publickey-credentials-get=(), publickey-credentials-create=()';

export const SITE_FRAME_CSP = "frame-ancestors 'none'";

export const SITE_HEADERS: Readonly<Record<string, string>> = {
	'Permissions-Policy': SITE_PERMISSIONS_POLICY,
	'Content-Security-Policy': SITE_FRAME_CSP,
	'X-Frame-Options': 'DENY',
	'X-Content-Type-Options': 'nosniff'
};

/**
 * Put {@link SITE_HEADERS} on a response. A CSP the response carries already
 * (the bug report's own) is kept, with `frame-ancestors 'none'` appended when
 * it says nothing about framing.
 */
export function applySiteHeaders(headers: Headers): void {
	for (const [name, value] of Object.entries(SITE_HEADERS)) {
		if (name === 'Content-Security-Policy') {
			const existing = headers.get(name);
			if (!existing) headers.set(name, value);
			else if (!/frame-ancestors/.test(existing)) headers.set(name, `${existing}; ${value}`);
			continue;
		}
		headers.set(name, value);
	}
}
