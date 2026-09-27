/**
 * Settings and the `sign_pref` core after the founder's ruling of 2026-09-26:
 * an account signs with the key it signed in with, so there is no default
 * "Sign with" to store, and none to draw. What the machine keeps — which
 * Trusted Signer page this device opens — has no row on the web either (the
 * web wallet has no Trusted Signer, owner 2026-09-23); the create flow reads it.
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

import { rawResolve, resolveSettingsMessages } from '$lib/i18n/engine.server';
import { buildDesktopState, buildMobileState, DESKTOP_STATES, MOBILE_STATES } from '../fixtures';
import { signPreference } from './sign-pref.svelte';
import { TRUSTED_SIGNER_URL_KEY, signPrefOperationFailure } from './sign-pref-executor';

const m = resolveSettingsMessages('en');
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

/** The core answers synchronously; the store is written on the next turns. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the store', () => {
	it('keys the page under the `vela.` prefix that survives sign-out', () => {
		expect(TRUSTED_SIGNER_URL_KEY).toBe('vela.trustedSignerUrl');
	});

	it('an unreadable preference is "never chose": the official page', () => {
		expect(signPrefOperationFailure({ id: 1, operation: { type: 'read_stored' } })).toEqual({
			type: 'stored',
			signer_url: null
		});
	});
});

describe('the preference, live', () => {
	beforeAll(async () => {
		store.clear();
		// A default "Sign with" stored by an older build: nobody reads it now.
		store.set('vela.signMethod', 'hybrid');
		store.set(TRUSTED_SIGNER_URL_KEY, 'https://sign.example/');
		await signPreference.ready();
	});

	it('reads the page this device stored — and nothing about how to sign', () => {
		expect(signPreference.view.signer_url).toBe('https://sign.example/');
		expect(signPreference.view.signer_url_is_default).toBe(false);
		expect(Object.keys(signPreference.view).some((key) => key.startsWith('method'))).toBe(false);
	});

	it('back to the official page forgets the stored one', async () => {
		signPreference.resetSignerUrl();
		await settled();
		expect(signPreference.view.signer_url_is_default).toBe(true);
		expect(store.has(TRUSTED_SIGNER_URL_KEY)).toBe(false);
	});
});

describe('Settings', () => {
	const signWith = rawResolve('en', 'settings.signing.title');

	it('draws no "Sign with" on any phone screen', () => {
		expect(signWith).not.toBe('settings.signing.title');
		for (const state of MOBILE_STATES) {
			const rows = buildMobileState(state, m, IDENTICON).sections.flatMap((s) => s.rows);
			expect(
				rows.map((row) => row.id),
				state
			).not.toContain('sign-with');
			expect(
				rows.map((row) => row.title),
				state
			).not.toContain(signWith);
		}
	});

	it('has no signing page on the desktop — it would be empty', () => {
		for (const state of DESKTOP_STATES) {
			const nav = buildDesktopState(state, m, IDENTICON).nav;
			expect(
				nav.map((item) => item.id),
				state
			).not.toContain('signing');
			expect(
				nav.map((item) => item.label),
				state
			).not.toContain(signWith);
		}
	});

	it('the Trusted Signer page and the tunnel have no rows here at all', () => {
		// By SHAPE, not by the two ids those rows once had. They were called
		// `clear-signer-page` and `clear-signer-tunnel`, and asserting on those
		// spellings after the rename is a test that passes because it can no
		// longer fail — the row could come back under any name.
		const rows = buildMobileState('st1', m, IDENTICON).sections.flatMap((section) => section.rows);
		// …and a model that drew nothing at all would satisfy any "not there".
		expect(rows.length).toBeGreaterThan(0);
		const aboutTheSigner = rows.filter((row) =>
			/signer|tunnel/i.test(`${row.id} ${row.title ?? ''}`)
		);
		expect(aboutTheSigner.map((row) => row.id)).toEqual([]);
	});
});
