/**
 * Settings → "Sign with" and the Clear Signer's page (spec 071, contract §6),
 * over the real `sign_pref` core: which keys it reads and writes, that it —
 * not the shell — refuses an address it would not open, and how the rows and
 * sheets read what it decided.
 */
import '$lib/i18n/wasm-init.server';
import { beforeAll, describe, expect, it, vi } from 'vitest';

const store = vi.hoisted(() => new Map<string, string>());

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/storage', () => ({
	getItem: async (key: string) => store.get(key) ?? null,
	setItem: async (key: string, value: string) => void store.set(key, value),
	removeItem: async (key: string) => void store.delete(key)
}));

import type { SignPrefView } from '$lib/core/generated/SignPrefView';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { buildDesktopState, buildMobileState } from '../fixtures';
import { withLiveSigning, withLiveSigningDesktop } from '../live';
import { signPreference } from './sign-pref.svelte';
import {
	CLEAR_SIGNER_URL_KEY,
	SIGN_METHOD_KEY,
	executeSignPrefOperation,
	signPrefOperationFailure
} from './sign-pref-executor';

const m = resolveSettingsMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

/** The core answers synchronously; the store is written on the next turns. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the store', () => {
	it('two keys under the `vela.` prefix that survives sign-out', () => {
		expect(SIGN_METHOD_KEY).toBe('vela.signMethod');
		expect(CLEAR_SIGNER_URL_KEY).toBe('vela.clearSignerUrl');
	});

	it('an unreadable preference is "never chose": auto, and the official page', () => {
		expect(signPrefOperationFailure({ id: 1, operation: { type: 'read_stored' } })).toEqual({
			type: 'stored',
			method: null,
			signer_url: null,
			relay_url: null
		});
	});

	it('the official page is stored as NO key, not as a copy of its address', async () => {
		store.set(CLEAR_SIGNER_URL_KEY, 'https://x.example/');
		await executeSignPrefOperation({ id: 1, operation: { type: 'write_signer_url', url: null } });
		expect(store.has(CLEAR_SIGNER_URL_KEY)).toBe(false);
	});
});

describe('the preference, live', () => {
	beforeAll(async () => {
		store.clear();
		store.set(SIGN_METHOD_KEY, 'hybrid');
		await signPreference.ready();
	});

	it('reads what this device stored', () => {
		expect(signPreference.view.method).toBe('hybrid');
		expect(signPreference.view.method_committed).toBe(true);
		expect(signPreference.view.signer_url_is_default).toBe(true);
	});

	it('a method chosen in Settings is committed and stored', async () => {
		signPreference.chooseMethod('clear_signer');
		expect(signPreference.view.method).toBe('clear_signer');
		await settled();
		expect(store.get(SIGN_METHOD_KEY)).toBe('clear_signer');
	});

	it('a name this build does not offer is ignored by the core', async () => {
		signPreference.chooseMethod('carrier_pigeon');
		await settled();
		expect(signPreference.view.method).toBe('clear_signer');
		expect(store.get(SIGN_METHOD_KEY)).toBe('clear_signer');
	});

	it('an address the browser would not sign on is refused, and nothing is stored', async () => {
		signPreference.submitSignerUrl('http://sign.example.org');
		await settled();
		expect(signPreference.view.signer_url_error).toBe('insecure');
		expect(store.has(CLEAR_SIGNER_URL_KEY)).toBe(false);
		signPreference.submitSignerUrl('not a page');
		expect(signPreference.view.signer_url_error).toBe('invalid');
		expect(signPreference.view.signer_url).toBe('https://sign.getvela.app/');
	});

	it('a page of the person’s own is normalised, stored, and said to be off getvela.app', async () => {
		signPreference.submitSignerUrl('  sign.example.org ');
		await settled();
		expect(signPreference.view.signer_url).toBe('https://sign.example.org/');
		expect(signPreference.view.signer_url_error).toBeNull();
		expect(signPreference.view.signer_uses_wallet_passkeys).toBe(false);
		expect(store.get(CLEAR_SIGNER_URL_KEY)).toBe('https://sign.example.org/');
	});

	it('"Use the official page" removes the key', async () => {
		signPreference.resetSignerUrl();
		await settled();
		expect(signPreference.view.signer_url_is_default).toBe(true);
		expect(store.has(CLEAR_SIGNER_URL_KEY)).toBe(false);
	});
});

describe('the rows and sheets', () => {
	const view = (patch: Partial<SignPrefView>): SignPrefView => ({
		method: 'auto',
		method_committed: false,
		offered: ['auto', 'platform', 'hybrid', 'security_key', 'clear_signer'],
		signer_url: 'https://sign.getvela.app/',
		signer_url_is_default: true,
		signer_url_error: null,
		signer_uses_wallet_passkeys: true,
		relay_url: 'wss://relay.getvela.app',
		relay_url_is_default: true,
		relay_url_error: null,
		...patch
	});
	const rows = (model: ReturnType<typeof buildMobileState>) =>
		model.sections.flatMap((section) => section.rows);

	it('sit next to "Transaction speed", in the advanced block', () => {
		const ids = rows(buildMobileState('st1b', m, IDENTICON)).map((row) => row.id);
		const speed = ids.indexOf('fee-speed');
		expect(ids.slice(speed, speed + 4)).toEqual([
			'fee-speed',
			'sign-with',
			'clear-signer-page',
			// Spec 075: how the Clear Signer is reached on another device.
			'clear-signer-relay'
		]);
	});

	it('name the method in force and the official page, and tick the method in the sheet', () => {
		const model = withLiveSigning(
			buildMobileState('st1', m, IDENTICON),
			view({ method: 'clear_signer', method_committed: true }),
			m
		);
		const row = (id: string) => rows(model).find((r) => r.id === id);
		expect(row('sign-with')?.value).toBe(m.signing.methods.clear_signer);
		expect(row('clear-signer-page')?.value).toBe(m.signing.pageOfficial);
		expect(model.signWithSheet.title).toBe(m.signing.title);
		expect(model.signWithSheet.subtitle).toBe(m.signing.subtitle);
		expect(model.signWithSheet.rows.map((r) => r.id)).toEqual([
			'auto',
			'platform',
			'hybrid',
			'security_key',
			'clear_signer'
		]);
		expect(model.signWithSheet.rows.filter((r) => r.selected).map((r) => r.id)).toEqual([
			'clear_signer'
		]);
		expect(model.signWithSheet.rows.at(-1)?.detail).toBe(m.signing.clearSignerBody);
		// The official page offers no reset and says nothing about passkeys.
		expect(model.signerPage.reset).toBeUndefined();
		expect(model.signerPage.foreign).toBeUndefined();
		expect(model.signerPage.error).toBeUndefined();
	});

	it('a page of the person’s own: its host on the row, the reset, and the rpId line', () => {
		const model = withLiveSigning(
			buildMobileState('st1', m, IDENTICON),
			view({
				signer_url: 'http://127.0.0.1:8137/',
				signer_url_is_default: false,
				signer_uses_wallet_passkeys: false
			}),
			m
		);
		expect(rows(model).find((r) => r.id === 'clear-signer-page')?.value).toBe('127.0.0.1:8137');
		expect(model.signerPage.field.value).toBe('http://127.0.0.1:8137/');
		expect(model.signerPage.reset).toBe(m.signing.pageReset);
		expect(model.signerPage.foreign).toBe(m.signing.pageForeign);
	});

	it('the core’s refusal is worded under the field', () => {
		const said = (error: string) =>
			withLiveSigning(buildMobileState('st1', m, IDENTICON), view({ signer_url_error: error }), m)
				.signerPage;
		expect(said('invalid').error).toBe(m.signing.pageInvalid);
		expect(said('insecure').error).toBe(m.signing.pageInsecure);
		expect(said('insecure').field.tone).toBe('error');
	});

	it('the relay row (spec 075): official by default, the host and a reset when it is not', () => {
		const official = withLiveSigning(buildMobileState('st1', m, IDENTICON), view({}), m);
		expect(rows(official).find((r) => r.id === 'clear-signer-relay')?.value).toBe(
			m.signing.relayOfficial
		);
		expect(official.relayPage.title).toBe(m.signing.relayTitle);
		expect(official.relayPage.subtitle).toBe(m.signing.relaySubtitle);
		expect(official.relayPage.field.value).toBe('wss://relay.getvela.app');
		expect(official.relayPage.reset).toBeUndefined();
		// A relay sees nothing but ciphertext: there is no rpId line to draw.
		expect(official.relayPage.foreign).toBeUndefined();

		const own = withLiveSigning(
			buildMobileState('st1', m, IDENTICON),
			view({ relay_url: 'ws://127.0.0.1:8787', relay_url_is_default: false }),
			m
		);
		expect(rows(own).find((r) => r.id === 'clear-signer-relay')?.value).toBe('127.0.0.1:8787');
		expect(own.relayPage.field.value).toBe('ws://127.0.0.1:8787');
		expect(own.relayPage.reset).toBe(m.signing.relayReset);
	});

	it('the relay’s own refusals are worded under its field', () => {
		const said = (error: string) =>
			withLiveSigning(buildMobileState('st1', m, IDENTICON), view({ relay_url_error: error }), m)
				.relayPage;
		expect(said('invalid').error).toBe(m.signing.relayInvalid);
		expect(said('insecure').error).toBe(m.signing.relayInsecure);
		expect(said('insecure').field.tone).toBe('error');
	});

	it('the desktop reads the same rows, and the same page', () => {
		const phone = withLiveSigning(
			buildMobileState('st1', m, IDENTICON),
			view({ method: 'security_key' }),
			m
		);
		const desktop = withLiveSigningDesktop(
			buildDesktopState('dst1', m, IDENTICON),
			view({ method: 'security_key', signer_url_is_default: false }),
			phone.signWithSheet,
			m
		);
		expect(desktop.nav.map((n) => n.id)).toContain('signing');
		expect(desktop.signing.rows[0].value).toBe(m.signing.methods.security_key);
		expect(desktop.signing.rows[0].options?.filter((r) => r.selected).map((r) => r.id)).toEqual([
			'security_key'
		]);
		expect(desktop.signing.page.reset).toBe(m.signing.pageReset);
		// Spec 075: and the relay section, the same body as the phone's sheet.
		expect(desktop.signing.relay.title).toBe(m.signing.relayTitle);
		expect(desktop.signing.relay.field.value).toBe('wss://relay.getvela.app');
	});
});
