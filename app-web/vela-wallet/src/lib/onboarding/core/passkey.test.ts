/**
 * Which relying party a ceremony runs under (spec 027 D31 / FR-307 / SC-306).
 *
 * The failure this prevents is the quiet one. Under `chrome-extension://` the
 * document's hostname is the extension id, and a ceremony that takes it mints a
 * VALID passkey — nothing throws, nothing warns — for a relying party
 * `https://getvela.app` cannot see. The address is derived from the keys, so
 * the extension silently becomes a second, empty wallet. It was found by
 * installing the extension in real Chrome and reading the dialog: it named the
 * extension id. No test in the suite had an opinion.
 *
 * This runs in the NODE project on purpose. The browser project's page is a
 * real https origin and can never be a `chrome-extension:` one, so it could
 * only ever assert the branch that was already correct; supplying the location
 * is what makes the extension branch testable at all.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { classifySign, relyingPartyId, SIGN_TIMEOUT_MS } from './passkey';

/**
 * Put the module at a location. Both globals are stubbed with the SAME object
 * because that is what a browser has: `relyingPartyId` reads `window.location`,
 * and the packaged-app predicate it defers to reads the bare `location`.
 */
function atLocation(protocol: string, hostname: string): void {
	const location = { protocol, hostname };
	vi.stubGlobal('window', { location });
	vi.stubGlobal('location', location);
}

/** The proxy extension's explicit instruction, set on `window`. */
function withProxyRpId(rpId: string): void {
	(window as unknown as { __VELA_WEBAUTHN_PROXY_RPID__?: string }).__VELA_WEBAUTHN_PROXY_RPID__ =
		rpId;
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('the relying party', () => {
	it('is getvela.app inside the extension, never the extension id', () => {
		atLocation('chrome-extension:', 'bjbdmnmpgcfkocfcfdocopkioacojkhl');
		expect(relyingPartyId()).toBe('getvela.app');
	});

	it('is the hosted site itself, and its subdomains resolve to it', () => {
		atLocation('https:', 'getvela.app');
		expect(relyingPartyId()).toBe('getvela.app');
		atLocation('https:', 'wallet.getvela.app');
		expect(relyingPartyId()).toBe('getvela.app');
	});

	it('is an ordinary host on any other origin, which owns its own passkeys', () => {
		// A preview deployment must NOT quietly claim the production relying
		// party; its wallets are its own, and the proxy extension is the way to
		// reach real ones from here.
		atLocation('https:', 'preview-027.vela-wallet.pages.dev');
		expect(relyingPartyId()).toBe('preview-027.vela-wallet.pages.dev');
		atLocation('http:', 'localhost');
		expect(relyingPartyId()).toBe('localhost');
	});

	it('yields to the proxy extension wherever it is asked from', () => {
		// The proxy sets this so the ceremony and the registry lookups agree on
		// one rpId; an origin rule that overruled it would break both at once.
		atLocation('https:', 'some-other-host.example');
		withProxyRpId('getvela.app');
		expect(relyingPartyId()).toBe('getvela.app');

		atLocation('chrome-extension:', 'bjbdmnmpgcfkocfcfdocopkioacojkhl');
		withProxyRpId('staging.getvela.app');
		expect(relyingPartyId()).toBe('staging.getvela.app');
	});

	it('is getvela.app when there is no document at all (prerender)', () => {
		expect(relyingPartyId()).toBe('getvela.app');
	});
});

/**
 * send-11: WebAuthn reports a prompt that ran out of time exactly as one that
 * was closed. A refusal that comes at the signing deadline is the deadline's.
 */
describe('a signing prompt that ran out of time', () => {
	const refusal = Object.assign(new Error('x'), { name: 'NotAllowedError' });

	it('is a cancel that says it timed out, at the deadline', () => {
		const out = classifySign(refusal, 0, SIGN_TIMEOUT_MS);
		expect(out.kind).toBe('cancelled');
		expect(out.timedOut).toBe(true);
	});

	it('is a plain cancel before it — closed, or no passkey on this device', () => {
		const out = classifySign(refusal, 0, 3_000);
		expect(out.kind).toBe('cancelled');
		expect(out.timedOut).toBe(false);
	});

	it('leaves every other failure as it was', () => {
		const other = classifySign(
			Object.assign(new Error('x'), { name: 'NotSupportedError' }),
			0,
			SIGN_TIMEOUT_MS
		);
		expect(other.kind).toBe('not_supported');
		expect(other.timedOut).toBe(false);
	});
});

/**
 * Round 10 (real-device S3): a failure the browser describes in its own
 * language carries its facts to the core, which names the sentence; the
 * message stays the detail for the report.
 */
describe("a browser's passkey error", () => {
	it('carries its DOMException name and message as facts', () => {
		const out = classifySign(
			Object.assign(new Error('The relying party ID is not a registrable domain suffix'), {
				name: 'SecurityError'
			}),
			0,
			3_000
		);
		expect(out.kind).toBe('other');
		expect(out.facts).toEqual({
			platform: 'web',
			domain: 'SecurityError',
			code: null,
			message: 'The relying party ID is not a registrable domain suffix'
		});
		const dup = classifySign(Object.assign(new Error('dup'), { name: 'InvalidStateError' }), 0, 0);
		expect(dup.facts?.domain).toBe('InvalidStateError');
	});

	it('a refusal carries none: it is said as a cancel, not an error', () => {
		const out = classifySign(Object.assign(new Error('x'), { name: 'NotAllowedError' }), 0, 0);
		expect(out.facts).toBeUndefined();
	});
});
