/**
 * Settings does not drop when the keys block loads (device walk, item 13.1),
 * measured in a real browser on the two real screens.
 *
 * The block was ONE grey row until the registry answered; then it grew to the
 * wallet's keys and everything under it moved — with three keys, 202 px on a
 * phone (294 in Russian, whose badges wrap) and 121 px on the wide layout.
 *
 * Here the block is drawn the way the page draws it while the registry is
 * being asked — over the device's own record, the core's device-only answer —
 * and then each answer lands. Every row, the copy's row and the first thing
 * under the block are read both times and must not have moved; and while
 * asking, nothing is said that the answer may contradict. Words are the
 * corpus's own (English, and Russian for its long badges), because the
 * geometry is the words'.
 */
import { tick } from 'svelte';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { commands, page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { Account } from '$lib/core/generated/Account';
import { CHECKING_ROW, type EthereumBackupRow } from '$lib/services/registry-backup';
import { deviceOnlyKeys, type WalletKeys, type WalletKeyRow } from '$lib/services/wallet-keys';
import en from '../../../../../assets/i18n/en.json';
import ru from '../../../../../assets/i18n/ru.json';
import { words } from './__fixtures__/words';
import { buildDesktopState, buildMobileState } from './fixtures';
import { walletKeysModel } from './live';
import { BACKUP_EXPLAIN_KEYS, BACKUP_ROW_KEYS, type SettingsMessages } from './messages';
import type { WalletKeysModel } from './model';
import SettingsDesktop from './SettingsDesktop.svelte';
import SettingsHome from './SettingsHome.svelte';

beforeAll(() => loadCore());
/** Written out when asked (`VITE_KEYS_GEOMETRY=<file>`), for the before/after table. */
const GEOMETRY_OUT = import.meta.env.VITE_KEYS_GEOMETRY as string | undefined;
const LOG: unknown[] = [];
afterAll(async () => {
	if (GEOMETRY_OUT) await commands.writeFile(GEOMETRY_OUT, JSON.stringify(LOG, null, 1));
	await page.viewport(414, 896);
});

const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const paths = words();

/** The block's own words, read from a catalog the way the engine resolves them. */
function blockWords(corpus: unknown): SettingsMessages {
	const k = (path: string) =>
		String(
			path
				.split('.')
				.reduce<unknown>((node, key) => (node as Record<string, unknown>)?.[key], corpus)
		);
	return {
		keys: {
			title: k('settingsModals.keys.title'),
			subtitle: k('settingsModals.keys.subtitle'),
			keyN: k('settingsModals.keys.keyN'),
			synced: k('onboarding.create.keySyncedBadge'),
			notSynced: k('settingsModals.keys.notSynced'),
			fromDevice: k('settingsModals.keys.fromDevice'),
			providerPlatform: k('onboarding.create.providerPlatform'),
			providerGeneric: k('onboarding.create.providerGeneric'),
			providerSecurityKey: k('onboarding.create.providerSecurityKey'),
			publicKey: k('settingsModals.keys.publicKey'),
			credential: k('settingsModals.keys.credential'),
			transport: k('settingsModals.keys.transport'),
			attestation: k('settingsModals.keys.attestation'),
			userVerified: k('settingsModals.keys.userVerified'),
			signsHere: k('settingsModals.keys.signsHere'),
			copy: k('componentsUi.signing.copyValue'),
			copied: k('receive.copied')
		},
		backup: {
			words: Object.fromEntries(BACKUP_ROW_KEYS.map((key) => [key, k(key)])),
			explains: Object.fromEntries(BACKUP_EXPLAIN_KEYS.map((key) => [key, k(key)]))
		},
		signing: { keysOn: k('settings.signing.keysOn') }
	} as unknown as SettingsMessages;
}

const KEY = (n: number) => '04' + String(n).repeat(128);
const NAMES = ['MacBook Pro', 'YubiKey 5C', 'Pixel 9'];

/**
 * The device's record: founding order, the second key the one signed in with.
 * No transports, as an older record has none — so the device alone would call
 * every one of them a built-in passkey, and the registry says otherwise.
 */
function wallet(count: 1 | 3): Account {
	const keys = NAMES.slice(0, count).map((name, index) => ({
		credential_id: `c${index + 1}`,
		public_key_hex: KEY(index + 1),
		name,
		transports: ''
	}));
	return {
		id: 'c1',
		name: 'Ann',
		address: '0x2222222222222222222222222222222222222222',
		public_key_hex: KEY(1),
		created_at_iso: '2026-09-26T00:00:00.000Z',
		keys,
		signing_domain: 'getvela.app',
		signing_venue: { type: 'in_vela' },
		sign_in_key: { credential_id: count === 3 ? 'c2' : 'c1', method: 'platform' }
	} as Account;
}

/** What the registry holds about the same keys — every field filled, the fullest badges. */
const REGISTRY_ROWS: WalletKeyRow[] = [
	{
		name: NAMES[0],
		authenticator_attachment: 'platform',
		transports: 'internal,hybrid',
		confirmed: true,
		synced: true,
		aaguid: 'fbfc3007-154e-4ecc-8c0b-6e020557d7bd',
		provider_name: 'iCloud Keychain',
		method: 'platform',
		public_key_hex: KEY(1),
		credential_id: 'Y3JlZGVudGlhbC0x',
		attestation_hex: '0x01' + 'fb'.repeat(16) + '1d0000',
		user_verified: true,
		signs_here: false
	},
	{
		name: NAMES[1],
		authenticator_attachment: 'cross-platform',
		transports: 'usb,nfc',
		confirmed: true,
		synced: false,
		aaguid: '',
		provider_name: '',
		method: 'security_key',
		public_key_hex: KEY(2),
		credential_id: 'Y3JlZGVudGlhbC0y',
		attestation_hex: '0x01' + '00'.repeat(16) + '050000',
		user_verified: true,
		signs_here: true
	},
	{
		name: NAMES[2],
		authenticator_attachment: 'cross-platform',
		transports: 'hybrid,internal',
		confirmed: true,
		synced: true,
		aaguid: 'ea9b8d66-4d01-1d21-3ce4-b6b48cb575d4',
		provider_name: 'Google Password Manager',
		method: 'hybrid',
		public_key_hex: KEY(3),
		credential_id: 'Y3JlZGVudGlhbC0z',
		attestation_hex: '0x01' + 'ea'.repeat(16) + '1d0000',
		user_verified: true,
		signs_here: false
	}
];

function registryAnswer(count: 1 | 3): WalletKeys {
	const keys = REGISTRY_ROWS.slice(0, count).map((row, index) =>
		count === 1 && index === 0 ? { ...row, signs_here: true } : row
	);
	return { source: 'registry', chainId: 100, keys };
}

const EXPLAIN = 'settingsModals.backup.explain';
const TITLE = 'settingsModals.backup.title';
/** The core's rows for where the record's copy stands (`BackupState::*.row()`). */
const NOT_COPIED: EthereumBackupRow = {
	title_key: TITLE,
	subtitle_key: 'settingsModals.backup.notBackedUp',
	tone: 'neutral',
	action: 'copy',
	explain_key: EXPLAIN
};
const COPIED: EthereumBackupRow = {
	title_key: TITLE,
	subtitle_key: 'settingsModals.backup.backedUp',
	tone: 'positive',
	action: 'none',
	explain_key: EXPLAIN
};
const COULD_NOT_CHECK: EthereumBackupRow = {
	title_key: TITLE,
	subtitle_key: 'settingsModals.backup.couldNotCheck',
	tone: 'neutral',
	action: 'retry',
	explain_key: EXPLAIN
};

/** The device's rows as an ANSWER: what a chain that holds no record, or none at all, leaves. */
function deviceRows(count: 1 | 3): WalletKeyRow[] {
	return deviceOnlyKeys(wallet(count))?.keys ?? [];
}

/** Every way the asking can end with the block still whole. */
const ENDINGS: Record<string, (count: 1 | 3) => [WalletKeys, EthereumBackupRow]> = {
	'the registry answers, every badge': (count) => [registryAnswer(count), NOT_COPIED],
	'the registry answers, its attestations unreadable': (count) => [
		{
			source: 'registry',
			chainId: 100,
			keys: registryAnswer(count).keys.map((row) => ({
				...row,
				synced: null,
				user_verified: null,
				attestation_hex: ''
			}))
		},
		COPIED
	],
	'no chain holds a record': (count) => [
		{ source: 'not_registered', chainId: null, keys: deviceRows(count) },
		NOT_COPIED
	],
	'the copy could not be checked': (count) => [registryAnswer(count), COULD_NOT_CHECK]
};

interface Reading {
	/** The block's own height. */
	block: number;
	/** The top of the first thing under the block. */
	under: number;
	/** The top of each key's row, then of the copy's row. */
	rows: number[];
}

const round = (n: number) => Math.round(n * 100) / 100;
const topOf = (element: Element) => round(element.getBoundingClientRect().top);

function read(root: Element, layout: 'phone' | 'wide'): Reading {
	const block = root.querySelector('section.keys');
	const outer = layout === 'phone' ? root.querySelector('.keys-block') : block;
	const under = outer?.nextElementSibling;
	if (!block || !under) throw new Error('the keys block, or what stands under it, is not drawn');
	return {
		block: round(block.getBoundingClientRect().height),
		under: topOf(under),
		rows: [...block.querySelectorAll('li.key, .row.skeleton, button.backup')].map(topOf)
	};
}

async function settle() {
	await tick();
	await document.fonts.ready;
	await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));
}

/** One of the two real screens, with its keys block; `to` swaps the block's model in place. */
async function screenWith(layout: 'phone' | 'wide', keys: WalletKeysModel) {
	if (layout === 'phone') {
		await page.viewport(390, 844);
		const base = buildMobileState('st1', paths, IDENTICON);
		const screen = render(SettingsHome, { props: { model: { ...base, keys } } });
		await settle();
		return {
			root: screen.container,
			async to(next: WalletKeysModel) {
				await screen.rerender({ model: { ...base, keys: next } });
				await settle();
			},
			unmount: () => screen.unmount()
		};
	}
	await page.viewport(1440, 900);
	const base = buildDesktopState('dst1', paths, IDENTICON);
	const withKeys = (model: WalletKeysModel) => ({
		...base,
		account: { ...base.account, keys: model }
	});
	const screen = render(SettingsDesktop, { props: { model: withKeys(keys) } });
	await settle();
	return {
		root: screen.container,
		async to(next: WalletKeysModel) {
			await screen.rerender({ model: withKeys(next) });
			await settle();
		},
		unmount: () => screen.unmount()
	};
}

const LAYOUTS = ['phone', 'wide'] as const;
const COUNTS = [1, 3] as const;
const LANGUAGES = { en: blockWords(en), ru: blockWords(ru) } as const;
const CASES = LAYOUTS.flatMap((layout) =>
	COUNTS.flatMap((count) =>
		Object.entries(LANGUAGES).map(([language, m]) => ({ layout, count, language, m }))
	)
);

/** The block as the page draws it while a chain is being asked. */
const asking = (count: 1 | 3, m: SettingsMessages) =>
	walletKeysModel(deviceOnlyKeys(wallet(count)), CHECKING_ROW, m);

describe('the keys block, from the device’s record to the registry’s answer', () => {
	for (const { layout, count, language, m } of CASES) {
		for (const [ending, answer] of Object.entries(ENDINGS)) {
			it(`${layout}, ${count} key(s), ${language} — ${ending}: nothing moves`, async () => {
				const view = await screenWith(layout, asking(count, m));
				const before = read(view.root, layout);
				await view.to(walletKeysModel(...answer(count), m));
				const after = read(view.root, layout);
				LOG.push({ layout, count, language, ending, before, after });

				expect(before.rows, 'a row for every key, and the copy’s row').toHaveLength(count + 1);
				expect(after.rows, 'each row where it stood').toEqual(before.rows);
				expect(after.block, 'the block as tall as it was').toBe(before.block);
				expect(after.under, 'what is under the block where it was').toBe(before.under);
				view.unmount();
			});
		}

		it(`${layout}, ${count} key(s), ${language} — the registry silent: the rows stay, and only its one sentence is added under them`, async () => {
			const view = await screenWith(layout, asking(count, m));
			const before = read(view.root, layout);
			await view.to(
				walletKeysModel(
					{ source: 'device', chainId: null, keys: deviceRows(count) },
					COULD_NOT_CHECK,
					m
				)
			);
			const after = read(view.root, layout);
			const note = view.root.querySelector('section.keys p.note');
			LOG.push({ layout, count, language, ending: 'the registry silent', before, after });

			expect(note?.textContent).toBe(m.keys.fromDevice);
			const added = round(note?.getBoundingClientRect().height ?? 0);
			// The keys' rows are where they stood; the sentence is under them.
			expect(added).toBeGreaterThan(0);
			expect(after.rows.slice(0, count)).toEqual(before.rows.slice(0, count));
			expect(after.under - before.under).toBeCloseTo(added, 1);
			expect(after.block - before.block).toBeCloseTo(added, 1);
			view.unmount();
		});
	}
});

describe('while the registry is being asked', () => {
	for (const { layout, count, language, m } of CASES) {
		it(`${layout}, ${count} key(s), ${language}: the count and each key’s own name and short id — and no word the registry may contradict`, async () => {
			const view = await screenWith(layout, asking(count, m));
			const block = view.root.querySelector('section.keys') as HTMLElement;
			const rows = [...block.querySelectorAll('li.key')];

			// What the device's record knows is said.
			expect(block.querySelector('h2 .count')?.textContent).toBe(String(count));
			expect(rows.map((row) => row.querySelector('.name')?.textContent?.trim())).toEqual(
				NAMES.slice(0, count)
			);
			expect(rows.map((row) => row.querySelector('.fingerprint')?.textContent)).toEqual(
				['1111…1111', '2222…2222', '3333…3333'].slice(0, count)
			);
			// …and which key this device signs with, which is the device's to say.
			const said = rows.map((row) =>
				[...row.querySelectorAll('.pill:not(.quiet)')].map((pill) => pill.textContent?.trim())
			);
			expect(said).toEqual(count === 1 ? [[m.keys.signsHere]] : [[], [m.keys.signsHere], []]);

			// Where a key lives is the registry's to say: the device would have
			// called all three "Built-in passkey", and the second is a security key.
			for (const row of rows) {
				expect(row.querySelector('.mark-slot')?.childElementCount, 'no mark yet').toBe(0);
				expect(row.querySelector('.meta .ghost'), 'the holder’s room').not.toBeNull();
				const read = (row as HTMLElement).innerText;
				for (const guess of [
					m.keys.providerPlatform,
					m.keys.providerGeneric,
					m.keys.providerSecurityKey,
					m.keys.synced,
					m.keys.notSynced,
					m.keys.userVerified
				]) {
					expect(read, `"${guess}" is not said yet`).not.toContain(guess);
				}
				// A badge to come is room, and nothing an assistive client reads.
				for (const quiet of row.querySelectorAll('.pill.quiet')) {
					expect(quiet.getAttribute('aria-hidden')).toBe('true');
				}
				expect(row.querySelectorAll('.pill.quiet').length).toBeGreaterThan(0);
				// Nothing of the registry's to open yet.
				expect((row.querySelector('button.summary') as HTMLButtonElement).disabled).toBe(true);
			}
			expect(block.querySelector('p.note'), 'not "couldn’t reach" while still asking').toBeNull();
			expect(block.querySelector('ul')?.getAttribute('aria-busy')).toBe('true');

			// The answer says it: the mark, the holder, the badges, and a row that opens.
			await view.to(walletKeysModel(registryAnswer(count), NOT_COPIED, m));
			const answered = [...block.querySelectorAll('li.key')];
			expect(answered[0].querySelector('.meta')?.textContent).toContain('iCloud Keychain');
			if (count === 3) {
				expect(answered[1].querySelector('.meta')?.textContent).toContain(
					m.keys.providerSecurityKey
				);
			}
			for (const row of answered) {
				expect(row.querySelector('.ghost')).toBeNull();
				expect(row.querySelector('.pill.quiet')).toBeNull();
				expect(row.querySelector('.mark-slot')?.childElementCount).toBeGreaterThan(0);
				expect((row.querySelector('button.summary') as HTMLButtonElement).disabled).toBe(false);
			}
			view.unmount();
		});
	}

	it('a key the device has no name for is not called "Key n" until the registry has had its say', async () => {
		const m = LANGUAGES.en;
		const unnamed = wallet(3);
		unnamed.keys[2].name = '';
		const view = await screenWith(
			'phone',
			walletKeysModel(deviceOnlyKeys(unnamed), CHECKING_ROW, m)
		);
		const third = () => view.root.querySelectorAll('section.keys li.key')[2];
		const before = read(view.root, 'phone');
		expect(third().querySelector('.name .ghost')).not.toBeNull();
		expect((third() as HTMLElement).innerText).not.toContain('Key 3');

		// The registry's record names it…
		const named = registryAnswer(3);
		named.keys[2] = { ...named.keys[2], name: 'Work phone' };
		await view.to(walletKeysModel(named, NOT_COPIED, m));
		expect(third().querySelector('.name')?.textContent?.trim()).toBe('Work phone');
		expect(read(view.root, 'phone').rows).toEqual(before.rows);

		// …and when it does not, the key is "Key 3", in the same place.
		named.keys[2] = { ...named.keys[2], name: '' };
		await view.to(walletKeysModel(named, NOT_COPIED, m));
		expect(third().querySelector('.name')?.textContent?.trim()).toBe('Key 3');
		expect(read(view.root, 'phone').rows).toEqual(before.rows);
		view.unmount();
	});

	it('before the device’s record is read there is one quiet row and no count, as there was', async () => {
		const view = await screenWith('phone', walletKeysModel(null, CHECKING_ROW, LANGUAGES.en));
		const block = view.root.querySelector('section.keys') as HTMLElement;
		expect(block.querySelectorAll('.row.skeleton')).toHaveLength(1);
		expect(block.querySelector('li.key')).toBeNull();
		expect(block.querySelector('h2 .count')).toBeNull();
		view.unmount();
	});
});

describe('the copy’s row', () => {
	for (const [language, m] of Object.entries(LANGUAGES)) {
		it(`${language}, phone: one height and one width through "checking" and every answer the row keeps`, async () => {
			const view = await screenWith('phone', asking(3, m));
			const row = () => view.root.querySelector('button.backup') as HTMLElement;
			const box = () => {
				const rect = row().getBoundingClientRect();
				const who = (row().querySelector('.who') as HTMLElement).getBoundingClientRect();
				return { top: round(rect.top), height: round(rect.height), who: round(who.width) };
			};
			const checking = box();
			expect(row().querySelector('.state')?.textContent).toBe(
				m.backup.words['componentsUi.funding.checking']
			);
			for (const answer of [NOT_COPIED, COPIED, COULD_NOT_CHECK]) {
				await view.to(walletKeysModel(registryAnswer(3), answer, m));
				expect(row().querySelector('.state')?.textContent).toBe(
					m.backup.words[answer.subtitle_key as keyof typeof m.backup.words]
				);
				expect(box(), answer.subtitle_key).toEqual(checking);
			}
			view.unmount();
		});
	}
});
