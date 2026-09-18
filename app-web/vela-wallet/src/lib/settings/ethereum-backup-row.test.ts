/**
 * The Ethereum backup row (spec 062): one line, three states, a button only
 * while there is something to do — and nothing at all when the feature is dark.
 */
import { describe, expect, it } from 'vitest';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { ethereumBackupRow, walletKeysModel } from './live';
import type { WalletKeyRow, WalletKeys } from '$lib/services/wallet-keys';

const m = resolveSettingsMessages('en');
const key = (over: Partial<WalletKeyRow>): WalletKeyRow => ({
	name: '',
	authenticator_attachment: 'platform',
	transports: 'internal',
	confirmed: true,
	synced: true,
	aaguid: '',
	provider_name: '',
	method: 'platform',
	public_key_hex: '04' + 'ab'.repeat(64),
	...over
});

describe('ethereumBackupRow', () => {
	it('says where the record stands, and is a button only when it is not backed up', () => {
		expect(ethereumBackupRow('not_backed_up', m)).toEqual({
			title: 'Back up keys to Ethereum',
			subtitle: 'Not backed up yet',
			tone: 'caution',
			actionable: true
		});
		expect(ethereumBackupRow('backed_up', m)).toMatchObject({
			subtitle: 'Backed up on Ethereum',
			tone: 'positive',
			actionable: false
		});
		// Silence is never drawn as a verdict — and never as something to tap.
		expect(ethereumBackupRow('could_not_check', m)).toMatchObject({
			subtitle: 'Could not check',
			actionable: false
		});
		expect(ethereumBackupRow('checking', m)).toMatchObject({ actionable: false });
	});

	it('draws nothing while the feature is dark or the wallet has no record', () => {
		expect(ethereumBackupRow('unavailable', m)).toBeUndefined();
		expect(ethereumBackupRow('not_registered', m)).toBeUndefined();
	});

	it('every locale has words for every state', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const words = resolveSettingsMessages(locale).backup;
			for (const value of Object.values(words)) expect(value, locale).not.toBe('');
		}
	});
});

describe('walletKeysModel', () => {
	const registry: WalletKeys = {
		source: 'registry',
		chainId: 100,
		keys: [
			key({ name: 'Interleave', provider_name: 'Apple Passwords' }),
			key({ synced: false, method: 'security_key', transports: 'usb,nfc' }),
			key({ method: 'hybrid' })
		]
	};

	it('names every key, says who may be holding it, and badges only what it can vouch for', () => {
		const model = walletKeysModel(registry, 'backed_up', m);
		expect(model.count).toBe('3');
		expect(model.loading).toBe(false);
		expect(model.note).toBeUndefined();
		expect(model.rows.map((row) => row.name)).toEqual(['Interleave', 'Key 2', 'Key 3']);
		expect(model.rows.map((row) => row.holderFallback)).toEqual([
			'Platform passkey',
			'Security key',
			'Passkey'
		]);
		expect(model.rows[0].fingerprint).toBe('abab…abab');
		expect(model.rows.map((row) => row.badge)).toEqual([
			{ text: 'Synced', tone: 'synced' },
			{ text: 'Not synced', tone: 'local' },
			{ text: 'Synced', tone: 'synced' }
		]);
	});

	it('still asking: a title and no guessed count', () => {
		const model = walletKeysModel(null, 'checking', m);
		expect(model).toMatchObject({ loading: true, count: '', rows: [] });
		expect(model.backup).toMatchObject({ actionable: false });
	});

	it("the registry silent: the device's memory, labelled, with no sync badges", () => {
		const model = walletKeysModel(
			{ source: 'device', chainId: null, keys: [key({ name: 'Mine', synced: null })] },
			'could_not_check',
			m
		);
		expect(model.note).toBe(m.keys.fromDevice);
		expect(model.rows[0].badge).toBeUndefined();
	});

	it("carries the backup as the block's last row — a button only when there is something to do", () => {
		expect(walletKeysModel(registry, 'not_backed_up', m).backup).toMatchObject({
			actionable: true
		});
		expect(walletKeysModel(registry, 'backed_up', m).backup).toMatchObject({ actionable: false });
		// No registry on Ethereum: the keys are still shown, the backup is not offered.
		expect(walletKeysModel(registry, 'unavailable', m).backup).toBeUndefined();
	});

	it('every locale has words for the block', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const words = resolveSettingsMessages(locale).keys;
			for (const value of Object.values(words)) expect(value, locale).not.toBe('');
			expect(words.keyN, locale).toContain('{{n}}');
		}
	});
});
