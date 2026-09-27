/**
 * Settings' keys list marks the key this device signs with (founder,
 * 2026-09-26: it must stand out). The core decides which row — matched by
 * public key from the sign-in route's credential — so what this pins is the
 * web's half: the walk is handed the credential of the account's sign-in
 * route, and each device key carries the credential it is found by. The
 * chain is not asked here: the walk is answered as if no chain did, which is
 * the device's own rows, marked the same way.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('./core-walk', () => ({ runWalk: async () => null }));

import type { Account } from '$lib/core/generated/Account';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { walletKeysModel } from '$lib/settings/live';
import { readWalletKeys } from './wallet-keys';

const m = resolveSettingsMessages('en');

const KEY = (n: number) => '04' + String(n).repeat(128);
function wallet(signedInWith?: Account['signed_in_with']): Account {
	return {
		id: 'aa01',
		name: 'Ann',
		address: '0x2222222222222222222222222222222222222222',
		public_key_hex: KEY(1),
		created_at_iso: '2026-09-26T00:00:00.000Z',
		keys: [
			{ credential_id: 'aa01', public_key_hex: KEY(1), name: 'Mac', transports: 'internal' },
			{ credential_id: 'bb02', public_key_hex: KEY(2), name: 'YubiKey', transports: 'usb,nfc' },
			{ credential_id: 'cc03', public_key_hex: KEY(3), name: 'Phone', transports: 'hybrid' }
		],
		...(signedInWith === undefined ? {} : { signed_in_with: signedInWith })
	};
}

describe('the keys walk', () => {
	it('marks the row of the key the account signed in with, and only that one', async () => {
		const { keys } = await readWalletKeys(
			wallet({ credential_id: 'bb02', method: 'security_key' })
		);
		expect(keys.map((key) => key.signs_here)).toEqual([false, true, false]);
		expect(keys[1].public_key_hex).toBe(KEY(2));
	});

	it('marks none for a record from before the sign-in key', async () => {
		const { keys } = await readWalletKeys(wallet());
		expect(keys).toHaveLength(3);
		expect(keys.some((key) => key.signs_here)).toBe(false);
	});
});

describe('the row model', () => {
	it('puts the one filled pill first, on that row alone', async () => {
		const model = walletKeysModel(
			await readWalletKeys(wallet({ credential_id: 'cc03', method: 'hybrid' })),
			'backed_up',
			m
		);
		const marked = model.rows.filter((row) => row.pills.some((p) => p.tone === 'signs_here'));
		expect(marked.map((row) => row.name)).toEqual(['Phone']);
		expect(marked[0].pills[0]).toEqual({ text: m.keys.signsHere, tone: 'signs_here' });
	});

	it('draws no such pill for a legacy record', async () => {
		const model = walletKeysModel(await readWalletKeys(wallet()), 'backed_up', m);
		const tones = model.rows.flatMap((row) => row.pills.map((p) => p.tone));
		expect(tones).not.toContain('signs_here');
	});
});
