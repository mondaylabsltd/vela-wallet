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
import { CHECKING_ROW } from './registry-backup';
import { deviceOnlyKeys, readWalletKeys } from './wallet-keys';

const m = resolveSettingsMessages('en');

const KEY = (n: number) => '04' + String(n).repeat(128);
/** As stored: a 102 record names its sign-in key `sign_in_key`; an older one `signed_in_with`. */
type SignIn = { credential_id: string; method: 'platform' | 'hybrid' | 'security_key' };
function wallet(signIn?: SignIn, legacy = false): Account {
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
		signing_domain: 'getvela.app',
		signing_venue: { type: 'in_vela' },
		...(signIn === undefined ? {} : legacy ? { signed_in_with: signIn } : { sign_in_key: signIn })
	} as Account;
}

describe('the keys walk', () => {
	it('marks the row of the key the account signed in with, and only that one', async () => {
		const { keys } = await readWalletKeys(
			wallet({ credential_id: 'bb02', method: 'security_key' })
		);
		expect(keys.map((key) => key.signs_here)).toEqual([false, true, false]);
		expect(keys[1].public_key_hex).toBe(KEY(2));
	});

	it('reads an older record’s `signed_in_with` through the core', async () => {
		const { keys } = await readWalletKeys(
			wallet({ credential_id: 'cc03', method: 'hybrid' }, true)
		);
		expect(keys.map((key) => key.signs_here)).toEqual([false, false, true]);
	});

	it('marks none for a record from before the sign-in key', async () => {
		const { keys } = await readWalletKeys(wallet());
		expect(keys).toHaveLength(3);
		expect(keys.some((key) => key.signs_here)).toBe(false);
	});
});

/** The core's row for a record already on Ethereum (`BackupState::BackedUp.row()`). */
const COPIED = {
	title_key: 'settingsModals.backup.title',
	subtitle_key: 'settingsModals.backup.backedUp',
	tone: 'positive',
	action: 'none'
} as const;

describe('the row model', () => {
	it('puts the one filled pill first, on that row alone', async () => {
		const model = walletKeysModel(
			await readWalletKeys(wallet({ credential_id: 'cc03', method: 'hybrid' })),
			COPIED,
			m
		);
		const marked = model.rows.filter((row) => row.pills.some((p) => p.tone === 'signs_here'));
		expect(marked.map((row) => row.name)).toEqual(['Phone']);
		expect(marked[0].pills[0]).toEqual({ text: m.keys.signsHere, tone: 'signs_here' });
	});

	it('draws no such pill for a legacy record', async () => {
		const model = walletKeysModel(await readWalletKeys(wallet()), COPIED, m);
		const tones = model.rows.flatMap((row) => row.pills.map((p) => p.tone));
		expect(tones).not.toContain('signs_here');
	});
});

/**
 * Device walk, item 13.1: Settings dropped when the keys block loaded, because
 * the block was one grey row until the registry answered. The device's own
 * record already says how many keys there are and what each is called.
 */
describe('before any chain is asked', () => {
	it('the device’s record gives every row at once, the sign-in key marked — and says it is still asking', () => {
		const keys = deviceOnlyKeys(wallet({ credential_id: 'bb02', method: 'security_key' }));
		expect(keys).toMatchObject({ source: 'device', chainId: null, asking: true });
		expect(keys?.keys.map((key) => key.name)).toEqual(['Mac', 'YubiKey', 'Phone']);
		expect(keys?.keys.map((key) => key.signs_here)).toEqual([false, true, false]);
		// Nothing a chain would have to vouch for.
		expect(keys?.keys.every((key) => key.synced === null && key.user_verified === null)).toBe(true);
	});

	it('the block has its rows and its count from that record, and no word the registry may contradict', () => {
		const account = wallet({ credential_id: 'bb02', method: 'security_key' });
		account.keys[2].name = '';
		const model = walletKeysModel(deviceOnlyKeys(account), CHECKING_ROW, m);
		expect(model).toMatchObject({ loading: false, asking: true, count: '3' });
		// The registry's record may name a key the device has no name for.
		expect(model.rows.map((row) => row.name)).toEqual(['Mac', 'YubiKey', '']);
		expect(model.rows.map((row) => row.fingerprint)).toEqual([
			'1111…1111',
			'2222…2222',
			'3333…3333'
		]);
		// Which key this device signs with is the device's own to say; no other badge is.
		expect(model.rows.map((row) => row.pills.map((pill) => pill.tone))).toEqual([
			[],
			['signs_here'],
			[]
		]);
		// "Couldn't reach the registry" is not said while it is being asked.
		expect(model.note).toBeUndefined();
	});

	it('a row keeps room for the fullest badges it can end with, either wording of "synced"', () => {
		const model = walletKeysModel(
			deviceOnlyKeys(wallet({ credential_id: 'bb02', method: 'security_key' })),
			CHECKING_ROW,
			m
		);
		const room = (index: number) =>
			model.rows[index].pillRoom.map((set) => set.map((pill) => pill.text));
		expect(room(0)).toEqual([
			[m.keys.userVerified, m.keys.synced],
			[m.keys.userVerified, m.keys.notSynced]
		]);
		expect(room(1)).toEqual([
			[m.keys.signsHere, m.keys.userVerified, m.keys.synced],
			[m.keys.signsHere, m.keys.userVerified, m.keys.notSynced]
		]);
		// …and keeps it once answered, whatever the answer could vouch for.
		const answered = walletKeysModel(
			{ source: 'not_registered', chainId: null, keys: deviceOnlyKeys(wallet())?.keys ?? [] },
			COPIED,
			m
		);
		expect(answered.asking).toBe(false);
		expect(answered.rows.map((row) => row.pills)).toEqual([[], [], []]);
		expect(answered.rows.map((row) => row.pillRoom.map((set) => set.length))).toEqual([
			[2, 2],
			[2, 2],
			[2, 2]
		]);
		// A key nobody named is "Key n" once the registry has had its say.
		const unnamed = wallet();
		unnamed.keys[0].name = '';
		const done = walletKeysModel(
			{ source: 'not_registered', chainId: null, keys: deviceOnlyKeys(unnamed)?.keys ?? [] },
			COPIED,
			m
		);
		expect(done.rows[0].name).toBe('Key 1');
	});

	it('the copy’s row keeps room for every second line it can say', () => {
		const model = walletKeysModel(deviceOnlyKeys(wallet()), CHECKING_ROW, m);
		expect(model.backup?.subtitle).toBe(m.backup.words['componentsUi.funding.checking']);
		expect([...model.backupLines].sort()).toEqual(
			[
				m.backup.words['settingsModals.backup.backedUp'],
				m.backup.words['settingsModals.backup.notBackedUp'],
				m.backup.words['settingsModals.backup.couldNotCheck'],
				m.backup.words['settingsModals.backup.cannotCopy'],
				m.backup.words['componentsUi.funding.checking']
			].sort()
		);
		expect(model.backupLines).not.toContain(model.backup?.title);
	});
});
