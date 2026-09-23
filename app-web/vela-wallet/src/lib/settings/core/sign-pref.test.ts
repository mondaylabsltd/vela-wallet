/**
 * Settings → "Sign with" (spec 071, contract §6), over the real `sign_pref`
 * core: which key it reads and writes, and how the row and its sheet read
 * what the core decided.
 *
 * The Clear Signer's own rows are NOT here: the web wallet has no Clear
 * Signer (owner, 2026-09-23), so the core's `clear_signer` is never drawn and
 * never in force, which is the one thing this file still asserts about it.
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
	it('keys under the `vela.` prefix that survives sign-out', () => {
		expect(SIGN_METHOD_KEY).toBe('vela.signMethod');
		expect(CLEAR_SIGNER_URL_KEY).toBe('vela.clearSignerUrl');
	});

	it('an unreadable preference is "never chose": auto, and nothing stored', () => {
		expect(signPrefOperationFailure({ id: 1, operation: { type: 'read_stored' } })).toEqual({
			type: 'stored',
			method: null,
			signer_url: null
		});
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
	});

	it('a method chosen in Settings is committed and stored', async () => {
		signPreference.chooseMethod('security_key');
		expect(signPreference.view.method).toBe('security_key');
		await settled();
		expect(store.get(SIGN_METHOD_KEY)).toBe('security_key');
	});

	it('a name this build does not offer is ignored by the core', async () => {
		signPreference.chooseMethod('carrier_pigeon');
		await settled();
		expect(signPreference.view.method).toBe('security_key');
		expect(store.get(SIGN_METHOD_KEY)).toBe('security_key');
	});
});

describe('the row and its sheet', () => {
	const view = (patch: Partial<SignPrefView>): SignPrefView => ({
		method: 'auto',
		method_committed: false,
		// The core offers five to every shell; this one draws four.
		offered: ['auto', 'platform', 'hybrid', 'security_key', 'clear_signer'],
		signer_url: 'https://sign.getvela.app/',
		signer_url_is_default: true,
		signer_url_error: null,
		signer_uses_wallet_passkeys: true,
		...patch
	});

	const home = (patch: Partial<SignPrefView> = {}) =>
		withLiveSigning(buildMobileState('st1', m, IDENTICON), view(patch), m);

	it('sits next to "Transaction speed", in the advanced block', () => {
		const rows = home().sections.flatMap((section) => section.rows.map((row) => row.id));
		expect(rows).toContain('sign-with');
		expect(rows.indexOf('sign-with')).toBe(rows.indexOf('fee-speed') + 1);
	});

	it('names the method in force, and ticks it in the sheet', () => {
		const model = home({ method: 'hybrid', method_committed: true });
		const row = model.sections.flatMap((s) => s.rows).find((r) => r.id === 'sign-with');
		expect(row?.value).toBe(m.signing.methods.hybrid);
		expect(model.signWithSheet.rows.filter((r) => r.selected).map((r) => r.id)).toEqual([
			'hybrid'
		]);
	});

	it('the Clear Signer is never drawn, whatever the core offers', () => {
		const model = home();
		expect(model.signWithSheet.rows.map((r) => r.id)).toEqual([
			'auto',
			'platform',
			'hybrid',
			'security_key'
		]);
	});

	it('the Clear Signer page and the tunnel have no rows here at all', () => {
		const rows = home().sections.flatMap((section) => section.rows.map((row) => row.id));
		expect(rows).not.toContain('clear-signer-page');
		expect(rows).not.toContain('clear-signer-tunnel');
	});

	it('the desktop reads the same rows', () => {
		const desktop = withLiveSigningDesktop(
			buildDesktopState('dst1', m, IDENTICON),
			view({ method: 'hybrid', method_committed: true }),
			home().signWithSheet,
			m
		);
		expect(desktop.signing.rows[0]?.value).toBe(m.signing.methods.hybrid);
		expect(desktop.signing.rows[0]?.options?.map((o) => o.id)).toEqual([
			'auto',
			'platform',
			'hybrid',
			'security_key'
		]);
	});
});
