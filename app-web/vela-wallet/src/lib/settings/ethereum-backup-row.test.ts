/**
 * The row for the wallet record's copy on Ethereum (spec 062): one line, a
 * button only while a tap does something — and nothing at all when the
 * feature is dark.
 *
 * The words, the tone and the tap are the CORE's (`registry_backup::BackupRow`,
 * the same on all four apps). The shell maps no state: it looks up the two
 * corpus keys the row names. The rows below are the core's own table.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { resolveSettingsMessages } from '$lib/i18n/engine.server';
import { CHECKING_ROW, type EthereumBackupRow } from '$lib/services/registry-backup';
import { ethereumBackupRow, walletKeysModel } from './live';
import { BACKUP_EXPLAIN_KEYS, BACKUP_ROW_KEYS } from './messages';
import { deviceKeys } from '$lib/services/wallet-keys';
import type { WalletKeyRow, WalletKeys } from '$lib/services/wallet-keys';

const m = resolveSettingsMessages('en');
/** The paragraph under the block, in English. */
const EXPLAIN = m.backup.explains['settingsModals.backup.explain'];
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
	signs_here: false,
	...over
});

/** `BackupState::row()` for each state that draws one — the core's table. */
const TITLE_KEY = 'settingsModals.backup.title';
/** `registry_backup::EXPLAIN_KEY` — on every row but the one that can never be copied. */
const EXPLAIN_KEY = 'settingsModals.backup.explain';
const CORE_ROW = {
	backed_up: {
		title_key: TITLE_KEY,
		subtitle_key: 'settingsModals.backup.backedUp',
		tone: 'positive',
		action: 'none',
		explain_key: EXPLAIN_KEY
	},
	not_backed_up: {
		title_key: TITLE_KEY,
		subtitle_key: 'settingsModals.backup.notBackedUp',
		tone: 'neutral',
		action: 'copy',
		explain_key: EXPLAIN_KEY
	},
	could_not_check: {
		title_key: TITLE_KEY,
		subtitle_key: 'settingsModals.backup.couldNotCheck',
		tone: 'neutral',
		action: 'retry',
		explain_key: EXPLAIN_KEY
	},
	// The core names no explanation here: nothing can be copied.
	not_copyable: {
		title_key: TITLE_KEY,
		subtitle_key: 'settingsModals.backup.cannotCopy',
		tone: 'neutral',
		action: 'none'
	}
} as const satisfies Record<string, EthereumBackupRow>;

describe('ethereumBackupRow', () => {
	it('says what it is — a copy of the wallet’s record — and where it stands', () => {
		expect(ethereumBackupRow(CORE_ROW.not_backed_up, m)).toEqual({
			title: "Copy this wallet's record to Ethereum",
			// Optional, and it costs a fee: a state, never a warning.
			subtitle: 'Not copied yet (optional)',
			tone: 'neutral',
			action: 'copy',
			explain: EXPLAIN
		});
		expect(ethereumBackupRow(CORE_ROW.backed_up, m)).toEqual({
			title: "Copy this wallet's record to Ethereum",
			subtitle: 'Copied to Ethereum',
			tone: 'positive',
			action: 'none',
			explain: EXPLAIN
		});
		const zh = resolveSettingsMessages('zh');
		expect(ethereumBackupRow(CORE_ROW.not_backed_up, zh)).toMatchObject({
			title: '把钱包记录复制到以太坊',
			subtitle: '尚未复制（可选）'
		});
	});

	it('takes a tap only where one does something, and says which', () => {
		// Silence is never drawn as a verdict — but it IS the one state a
		// person taps, and what they want is another attempt (founder ruling,
		// 2026-09-23): the tap asks again, it does not start a copy.
		expect(ethereumBackupRow(CORE_ROW.could_not_check, m)).toEqual({
			title: "Copy this wallet's record to Ethereum",
			subtitle: "Couldn't check. Tap to try again.",
			tone: 'neutral',
			action: 'retry',
			explain: EXPLAIN
		});
		// An older wallet's record can never be copied. Asking again gets the
		// same answer, so it is a calm end with nothing to tap — it used to
		// read "could not check" and retry for ever.
		expect(ethereumBackupRow(CORE_ROW.not_copyable, m)).toEqual({
			title: "Copy this wallet's record to Ethereum",
			subtitle: "This older wallet can't be copied",
			tone: 'neutral',
			action: 'none'
		});
		// Still asking: the core's "Checking…", nothing to tap.
		expect(ethereumBackupRow(CHECKING_ROW, m)).toEqual({
			title: "Copy this wallet's record to Ethereum",
			subtitle: 'Checking…',
			tone: 'neutral',
			action: 'none',
			explain: EXPLAIN
		});
	});

	// PR 3 note 6. The paragraph says what the copy makes public and what it
	// costs — "This puts a copy on Ethereum too… you confirm it with a
	// passkey". Under "This older wallet can't be copied" it told a person how
	// to do the one thing the line above had just said cannot be done.
	it('a wallet that can never be copied is not told how to copy: no paragraph at all', () => {
		const row = ethereumBackupRow(CORE_ROW.not_copyable, m);
		expect(row).toBeDefined();
		expect(row).not.toHaveProperty('explain');
		// Every other drawn state keeps it — and the walk still running does too.
		for (const other of [
			CORE_ROW.backed_up,
			CORE_ROW.not_backed_up,
			CORE_ROW.could_not_check,
			CHECKING_ROW
		]) {
			expect(ethereumBackupRow(other, m)?.explain).toBe(EXPLAIN);
		}
		// `null` is absent too (the wire's other spelling of "none").
		expect(ethereumBackupRow({ ...CORE_ROW.backed_up, explain_key: null }, m)).not.toHaveProperty(
			'explain'
		);
		// A paragraph this build has no words for draws none — never a dotted path.
		expect(
			ethereumBackupRow({ ...CORE_ROW.backed_up, explain_key: 'settingsModals.backup.other' }, m)
		).not.toHaveProperty('explain');
	});

	it('no state is a caution: the row never wears the warning tone', () => {
		for (const row of [...Object.values(CORE_ROW), CHECKING_ROW]) {
			expect(['neutral', 'positive']).toContain(ethereumBackupRow(row, m)?.tone);
		}
	});

	it('draws nothing while the feature is dark or the wallet has no record', () => {
		// `unavailable` / `not_registered`: the core sends no row.
		expect(ethereumBackupRow(null, m)).toBeUndefined();
	});

	it('a key the manifest does not carry draws no row — never a dotted path', () => {
		const unknown = { ...CORE_ROW.backed_up, subtitle_key: 'settingsModals.backup.somethingNew' };
		expect(ethereumBackupRow(unknown, m)).toBeUndefined();
	});

	// The manifest is held to the core's SOURCE: every corpus key
	// `registry_backup.rs` names for the row (its title, its "checking" line
	// and each state's second line) has words here. A key the core adds later
	// fails this test instead of drawing no row on a person's screen.
	it('the manifest has every key the core’s row can name', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/registry_backup.rs', 'utf8');
		const named = new Set(
			[...source.matchAll(/"((?:settingsModals\.backup|componentsUi\.funding)\.[A-Za-z]+)"/g)].map(
				(match) => match[1]
			)
		);
		// The row's lines, and the paragraph under the block — each in its own
		// list, both held to the core.
		expect(named.size).toBeGreaterThanOrEqual(7);
		expect([...named].sort()).toEqual([...BACKUP_ROW_KEYS, ...BACKUP_EXPLAIN_KEYS].sort());
		expect(source).toContain(`pub const EXPLAIN_KEY: &str = "${EXPLAIN_KEY}";`);
	});

	it('every locale has words for every key, and an explanation', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const backup = resolveSettingsMessages(locale).backup;
			for (const key of BACKUP_ROW_KEYS) expect(backup.words[key], `${locale} ${key}`).toBeTruthy();
			for (const key of BACKUP_EXPLAIN_KEYS)
				expect(backup.explains[key], `${locale} ${key}`).toBeTruthy();
		}
	});

	it('the explanation says what becomes public and that it costs a fee — not "only public keys"', () => {
		const explain = EXPLAIN;
		for (const said of ['name', 'public key', 'credential ID', 'authenticator model', 'fee']) {
			expect(explain).toContain(said);
		}
		expect(explain).not.toMatch(/only public keys/i);
		// No fixed dollar figure: it goes stale, and the sheet shows the fee.
		expect(explain).not.toMatch(/\$\d/);
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
		const model = walletKeysModel(registry, CORE_ROW.backed_up, m);
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
		expect(model.backup?.explain).toBe(EXPLAIN);
	});

	it('still asking: a title and no guessed count', () => {
		const model = walletKeysModel(null, CHECKING_ROW, m);
		expect(model).toMatchObject({ loading: true, count: '', rows: [] });
		expect(model.backup).toMatchObject({ action: 'none' });
	});

	it("the registry silent: the device's memory, labelled, with no sync badges", () => {
		const model = walletKeysModel(
			{ source: 'device', chainId: null, keys: [key({ name: 'Mine', synced: null })] },
			CORE_ROW.could_not_check,
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
			null,
			m
		);
		expect(unregistered.note).toBeUndefined();
	});

	it("carries the copy's row as the block's last — tappable only where a tap does something", () => {
		expect(walletKeysModel(registry, CORE_ROW.not_backed_up, m).backup).toMatchObject({
			action: 'copy'
		});
		expect(walletKeysModel(registry, CORE_ROW.backed_up, m).backup).toMatchObject({
			action: 'none'
		});
		const never = walletKeysModel(registry, CORE_ROW.not_copyable, m).backup;
		expect(never).toMatchObject({ action: 'none' });
		// …and it ends on its row: no paragraph about making a copy.
		expect(never).not.toHaveProperty('explain');
		// No registry on Ethereum: the keys are still shown, the copy is not offered.
		expect(walletKeysModel(registry, null, m).backup).toBeUndefined();
	});

	it('every locale has words for the block', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const words = resolveSettingsMessages(locale).keys;
			for (const value of Object.values(words)) expect(value, locale).not.toBe('');
			expect(words.keyN, locale).toContain('{{n}}');
		}
	});

	/**
	 * Spec 102 (P2-10): a key is captioned by where it LIVES — this device, a
	 * phone, a security key — never by a page. Spec 075 drew a key minted on a
	 * signing page as "Trusted Signer"; but the same key signs in Vela and on
	 * the page alike, and a page is where a person reviews, not where a key is.
	 * An account on its OWN signing domain says that domain once, above its
	 * keys, because those keys answer only on its page.
	 */
	describe('captions by place (spec 102)', () => {
		it('hands the walk each key and nothing about pages', () => {
			expect(
				deviceKeys({
					id: 'one',
					name: 'Wallet',
					public_key_hex: '04ab',
					keys: [
						{
							credential_id: 'one',
							public_key_hex: '04ab',
							name: 'Minted on the page',
							transports: 'internal',
							signer_origin: 'https://sign.getvela.app'
						},
						{
							credential_id: 'two',
							public_key_hex: '04cd',
							name: 'Built in',
							transports: 'internal'
						}
					]
				})
			).toEqual([
				{
					credential_id: 'one',
					public_key_hex: '04ab',
					name: 'Minted on the page',
					transports: 'internal'
				},
				{ credential_id: 'two', public_key_hex: '04cd', name: 'Built in', transports: 'internal' }
			]);
			// A legacy record is one key, its credential the record's id.
			expect(deviceKeys({ id: 'old', name: 'Old', public_key_hex: '04ef', keys: [] })).toEqual([
				{ credential_id: 'old', public_key_hex: '04ef', name: 'Old', transports: '' }
			]);
		});

		it('a key the page minted is captioned by its place, and no row names a page', () => {
			// The authenticator answered `platform`; the catalog can name its vault.
			const minted = key({
				name: 'Minted on the page',
				method: 'platform',
				provider_name: 'Apple Passwords',
				aaguid: 'fbfc3007-154e-4ecc-8c0b-6e020557d7bd',
				credential_id: 'aa_bgDzJkhFmY'
			});
			const model = walletKeysModel(
				{ source: 'registry', chainId: 100, keys: [minted] },
				CORE_ROW.backed_up,
				m
			);
			const row = model.rows[0];
			expect(row.holderFallback).toBe(m.keys.providerPlatform);
			expect(row.key.kind).toBe('platform');
			expect(row.details.map((detail) => detail.label)).toEqual([
				'Public key',
				'Credential',
				'AAGUID',
				'Transport'
			]);
			for (const r of walletKeysModel(registry, CORE_ROW.backed_up, m).rows) {
				expect(['platform', 'hybrid', 'security_key']).toContain(r.key.kind);
				expect(r.holderFallback).not.toMatch(/trusted signer/i);
			}
		});

		it('says the domain once for an account on its own — and only then', () => {
			const own = walletKeysModel(registry, CORE_ROW.backed_up, m, 'sign.example.com');
			expect(own.domain).toBe('Keys on sign.example.com');
			expect(walletKeysModel(registry, CORE_ROW.backed_up, m).domain).toBeUndefined();
		});

		it('every locale has the domain line', () => {
			for (const locale of SUPPORTED_LOCALES)
				expect(resolveSettingsMessages(locale).signing.keysOn, locale).toContain('{{domain}}');
		});
	});
});
