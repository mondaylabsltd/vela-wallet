/**
 * §35 (review 1-1): wallet.getvela.app's headers are one list. The static
 * assets' `_headers` `/*` block is exactly `WALLET_HEADERS`; the Worker's
 * responses get the same from `withWalletHeaders`, which keeps kit's own CSP;
 * the page CSP loads no script from anywhere but the wallet itself.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	CSP_DIRECTIVES,
	FRAME_CSP,
	PERMISSIONS_POLICY,
	WALLET_HEADERS,
	withWalletHeaders
} from './security-headers';

function block(path: string): Record<string, string> {
	const lines = readFileSync(new URL('../../_headers', import.meta.url), 'utf8').split('\n');
	const start = lines.findIndex((l) => l.trim() === path);
	expect(start, path).toBeGreaterThanOrEqual(0);
	const out: Record<string, string> = {};
	for (const line of lines.slice(start + 1)) {
		if (!/^\s+\S/.test(line)) break;
		const at = line.indexOf(':');
		out[line.slice(0, at).trim()] = line.slice(at + 1).trim();
	}
	return out;
}

describe('wallet.getvela.app headers', () => {
	it('the static rules are the one list, beside the wasm cache rule', () => {
		expect(block('/*')).toEqual(WALLET_HEADERS);
		expect(block('/vela_core_bg.*')).toEqual({
			'Cache-Control': 'public, immutable, max-age=31536000'
		});
	});

	it('is never framed, sniffed or referred', () => {
		expect(WALLET_HEADERS['Content-Security-Policy']).toBe(FRAME_CSP);
		expect(FRAME_CSP).toContain("frame-ancestors 'none'");
		expect(WALLET_HEADERS['X-Frame-Options']).toBe('DENY');
		expect(WALLET_HEADERS['Referrer-Policy']).toBe('no-referrer');
		expect(WALLET_HEADERS['X-Content-Type-Options']).toBe('nosniff');
		// The signing page answers through window.opener: no COOP same-origin.
		expect(Object.keys(WALLET_HEADERS)).not.toContain('Cross-Origin-Opener-Policy');
	});

	it('keeps passkeys for itself and turns off what it never uses', () => {
		const policy = Object.fromEntries(
			PERMISSIONS_POLICY.split(', ').map((p) => p.split('=') as [string, string])
		);
		expect(policy['publickey-credentials-get']).toBe('(self)');
		expect(policy['publickey-credentials-create']).toBe('(self)');
		expect(policy['camera']).toBe('(self)');
		for (const off of ['microphone', 'geolocation', 'payment', 'usb', 'hid', 'bluetooth']) {
			expect(policy[off], off).toBe('()');
		}
	});

	it('runs no script but its own: no remote origin, no eval', () => {
		const scripts: readonly string[] = CSP_DIRECTIVES['script-src'];
		expect(scripts).toEqual(['self', 'wasm-unsafe-eval']);
		expect(scripts).not.toContain('unsafe-eval');
		expect(scripts).not.toContain('unsafe-inline');
		expect(CSP_DIRECTIVES['object-src']).toEqual(['none']);
		expect(CSP_DIRECTIVES['frame-src']).toEqual(['none']);
		expect(CSP_DIRECTIVES['frame-ancestors']).toEqual(['none']);
	});

	it("keeps kit's CSP on a rendered page and adds the framing rule where it is missing", () => {
		const kit = withWalletHeaders(
			new Response('x', {
				headers: { 'Content-Security-Policy': "script-src 'self'; frame-ancestors 'none'" }
			})
		);
		expect(kit.headers.get('Content-Security-Policy')).toBe(
			"script-src 'self'; frame-ancestors 'none'"
		);
		const other = withWalletHeaders(
			new Response('x', { headers: { 'Content-Security-Policy': "default-src 'none'" } })
		);
		expect(other.headers.get('Content-Security-Policy')).toBe(`default-src 'none'; ${FRAME_CSP}`);
		// An immutable response (a redirect) is rebuilt with the headers.
		const redirect = withWalletHeaders(Response.redirect('https://wallet.getvela.app/en', 302));
		expect(redirect.status).toBe(302);
		expect(redirect.headers.get('X-Frame-Options')).toBe('DENY');
		expect(redirect.headers.get('Location')).toBe('https://wallet.getvela.app/en');
	});
});
