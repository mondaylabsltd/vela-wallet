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
	credential_id: '',
	attestation_hex: '',
	user_verified: null,
	...over
});

describe('ethereumBackupRow', () => {
	it('says where the record stands, and takes a tap only where one does something', () => {
		expect(ethereumBackupRow('not_backed_up', m)).toEqual({
			title: 'Back up public keys to Ethereum',
			subtitle: 'Not backed up yet',
			tone: 'caution',
			actionable: true
		});
		expect(ethereumBackupRow('backed_up', m)).toMatchObject({
			subtitle: 'Backed up on Ethereum',
			tone: 'positive',
			actionable: false
		});
		// Silence is never drawn as a verdict — but it IS the one state a
		// person taps, and what they want is another attempt (founder ruling,
		// 2026-09-23). It carries `retry` so the row can say "ask again"
		// rather than "back it up", which is a different offer.
		expect(ethereumBackupRow('could_not_check', m)).toMatchObject({
			subtitle: 'Could not check',
			actionable: true,
			retry: true
		});
		// Only that one. The other three have nothing to do, so they take no
		// tap at all — the house rule, and what stops a row rippling under a
		// finger for nothing.
		expect(ethereumBackupRow('not_backed_up', m).retry).toBeUndefined();
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
			key({
				name: 'Interleave',
				provider_name: 'Apple Passwords',
				aaguid: 'fbfc3007-154e-4ecc-8c0b-6e020557d7bd',
				credential_id: 'aa_bgDzJkhFmY',
				attestation_hex: '0x01fbfc3007154e4ecc8c0b6e020557d7bd5d0000',
				user_verified: true
			}),
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
		// Location-NEUTRAL wording, unlike the create flow's "This device": this
		// list can hold a key that lives on a computer the person is not sitting
		// at (issue 207).
		expect(model.rows.map((row) => row.holderFallback)).toEqual([
			'Built-in passkey',
			'Security key',
			'Phone or tablet'
		]);
		expect(model.rows[0].fingerprint).toBe('abab…abab');
		// The badge answers ONE question — cloud-synced or device-bound — in the same words
		// the create flow uses for the same fact (issue 207).
		expect(model.rows.map((row) => row.pills.map((pill) => pill.text))).toEqual([
			['Verify to use', 'Cloud-synced'],
			['Device-bound'],
			['Cloud-synced']
		]);
		// What a row opens onto: the explorer's facts, the two a person pastes elsewhere copyable.
		expect(model.rows[0].details.map((d) => [d.label, d.copy])).toEqual([
			['Public key', true],
			['Credential', true],
			['AAGUID', false],
			['Transport', false],
			['Attestation', false]
		]);
		expect(model.rows[0].details[0].value).toBe('0x04' + 'ab'.repeat(64));
		// The mark and the caption read one field, and it is the report.
		expect(model.rows.map((row) => row.key.kind)).toEqual(['platform', 'security_key', 'hybrid']);
		expect(model.rows.every((row) => row.key.synced_known)).toBe(true);
		expect(model.backupExplain).toContain('Private keys never leave');
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
		expect(model.rows[0].pills).toEqual([]);
		// …and the row handed to the shared mark says the same thing, instead of
		// letting `synced ?? true` present a fail-open guess as a verified fact
		// (issue 207).
		expect(model.rows[0].key).toMatchObject({ synced_known: false, kind: 'platform' });
		// Nothing to open when only the device answered.
		expect(model.rows[0].details.map((d) => d.label)).toEqual(['Public key', 'Transport']);
		// A registry that answered with nothing was not unreachable: no such note.
		const unregistered = walletKeysModel(
			{ source: 'not_registered', chainId: null, keys: [key({ synced: null })] },
			'unavailable',
			m
		);
		expect(unregistered.note).toBeUndefined();
	});

	it("carries the backup as the block's last row — tappable only where there is something to do", () => {
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
