/**
 * What a key row SAYS, in a real browser (issue 207).
 *
 * The reporter's screenshot had a row contradicting itself three ways at once:
 * a hardware-fob icon, the caption "Passkey", and the badge "This device only"
 * — on a YubiKey, which is not this device, and on a phone reached by scanning
 * a code, which is not a fob. The three slots came from three unrelated
 * signals. This pins the rule that replaced them: the mark and the caption both
 * answer WHERE THE KEY LIVES, from what the authenticator reported, and the
 * badge answers only whether it is backed up.
 *
 * A `.svelte.test.ts` because it is about what a person sees: the node project
 * does not render components, so a caption wired to the wrong field passes
 * there. The strings are the REAL generated corpus — a row that reads well with
 * invented copy proves nothing about the screen that ships.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { pluralTemplate } from '$lib/i18n/plural';
import en from '../../../../../../../assets/i18n/en.json';
import ru from '../../../../../../../assets/i18n/ru.json';
import zh from '../../../../../../../assets/i18n/zh.json';
import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
import KeysScreen from './KeysScreen.svelte';
import DoneScreen from './DoneScreen.svelte';

/** A screen's `strings`, over one real catalog — plural keys the way the routes resolve them. */
const stringsIn =
	(catalog: unknown, locale: string) =>
	(key: string, params?: Record<string, string | number>): string => {
		const lookup = (path: string) => {
			const node = path
				.split('.')
				.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], catalog);
			return typeof node === 'string' ? node : undefined;
		};
		const value =
			lookup(key) ??
			(typeof params?.count === 'number'
				? pluralTemplate(lookup, locale, key, params.count)
				: undefined);
		if (value === undefined) throw new Error(`no corpus value for "${key}"`);
		return Object.entries(params ?? {}).reduce(
			(text, [name, fill]) => text.replaceAll(`{{${name}}}`, String(fill)),
			value
		);
	};

const strings = stringsIn(en, 'en');

// The real core draws (or declines to draw) the fallback artwork, which is the
// half of this fix that lives in Rust.
beforeAll(() => loadCore());

const key = (over: Partial<CreateKeyRow>): CreateKeyRow => ({
	name: 'Key 2',
	authenticator_attachment: 'platform',
	transports: 'internal',
	confirmed: true,
	synced: true,
	synced_known: true,
	aaguid: '',
	provider_name: '',
	method: 'platform',
	kind: 'platform',
	...over
});

/** A YubiKey, minted by somebody who tapped "Phone or tablet" in the picker. */
const FOB = key({
	name: 'Yubikey',
	authenticator_attachment: 'cross-platform',
	transports: 'usb,nfc',
	synced: false,
	method: 'hybrid',
	kind: 'security_key'
});

/** A phone reached by scanning a code — the row that was drawn as a fob. */
const PHONE = key({
	name: 'My phone',
	authenticator_attachment: 'cross-platform',
	transports: 'hybrid,internal',
	synced: false,
	method: 'hybrid',
	kind: 'hybrid'
});

const keysScreen = (keys: CreateKeyRow[]) =>
	render(KeysScreen, {
		props: {
			keys,
			canAddKey: true,
			canFinish: true,
			needsSecondKey: false,
			busy: false,
			maxKeys: 7,
			// As the core says for a list with keys on it (issue 475).
			addHeadingKey: 'onboarding.create.addMethodLabel',
			methodsPinned: false,
			strings,
			onAddKey: () => {},
			onConfirmKey: () => {},
			onRemoveKey: () => {},
			onFinish: () => {}
		}
	});

const doneScreen = (keys: CreateKeyRow[], copy = strings) =>
	render(DoneScreen, {
		props: {
			address: '0x71C7A4E9b2F03D8cA51e7F6d92B4c8035E9A3F1c',
			walletName: 'Everyday wallet',
			keys,
			strings: copy,
			onEnter: () => {}
		}
	});

/**
 * Issue 409: the line under "Wallet created" read "Any of your 1 keys can
 * sign in on its own." It is plural now, and the form is the core's choice.
 */
describe('DoneScreen: the line under the title agrees with its count', () => {
	const subtitle = (container: HTMLElement) =>
		container.querySelector<HTMLElement>('.subtitle')?.textContent ?? '';

	it('says "your key" for one key and "any of your N keys" for several', () => {
		expect(subtitle(doneScreen([PHONE]).container)).toBe(
			'Your key can sign in on its own. The contract deploys with your first transaction.'
		);
		expect(subtitle(doneScreen([PHONE, FOB]).container)).toBe(
			'Any of your 2 keys can sign in on its own. The contract deploys with your first transaction.'
		);
	});

	it('takes Russian’s own form for 2–4 keys, and one sentence for every count in Chinese', () => {
		const inRu = stringsIn(ru, 'ru');
		expect(subtitle(doneScreen([PHONE], inRu).container)).toMatch(/^Ваш ключ/);
		expect(subtitle(doneScreen([PHONE, FOB, PHONE], inRu).container)).toContain('из 3 ключей');
		const inZh = stringsIn(zh, 'zh');
		expect(subtitle(doneScreen([PHONE], inZh).container)).toBe(
			subtitle(doneScreen([PHONE, FOB, PHONE], inZh).container)
		);
	});
});

/** The rows only, so the screen's own headings never answer for them. */
const rowText = (container: HTMLElement, selector: string) =>
	[...container.querySelectorAll<HTMLElement>(selector)].map((row) => row.textContent ?? '');

for (const [name, mount, selector] of [
	['KeysScreen', keysScreen, 'li.row'],
	['DoneScreen', doneScreen, 'li.key']
] as const) {
	describe(name, () => {
		it('captions a security key as a security key, whatever was tapped', () => {
			const { container } = mount([FOB]);
			const [row] = rowText(container, selector);
			expect(row).toContain('Security key');
			expect(row).not.toContain('This device');
			// The badge names the KIND of passkey, and it is not "where".
			expect(row).toContain('Device-bound');
		});

		it('draws a phone reached by a code as a phone, never as a hardware key', () => {
			const { container } = mount([PHONE]);
			const [row] = rowText(container, selector);
			expect(row).toContain('Phone or tablet');
			expect(row).not.toContain('This device');
			// No fallback ARTWORK at all for a phone: the fob image was the bug.
			const marks = container.querySelectorAll('img.mark');
			expect(marks.length).toBe(0);
			expect(container.querySelector('.glyph svg')).not.toBeNull();
		});

		it('says "This device" only for a key that really is on this device', () => {
			const { container } = mount([key({ name: 'This laptop' })]);
			const [row] = rowText(container, selector);
			expect(row).toContain('This device');
			expect(row).toContain('Cloud-synced');
			expect(row).not.toContain('Device-bound');
		});

		it('draws no badge when nobody can vouch for the backup', () => {
			const { container } = mount([key({ synced: true, synced_known: false })]);
			const [row] = rowText(container, selector);
			expect(row).not.toContain('Cloud-synced');
			expect(row).not.toContain('Device-bound');
		});

		it('never lets the three slots contradict each other', () => {
			const { container } = mount([FOB, PHONE, key({ name: 'This laptop' })]);
			const rows = rowText(container, selector);
			expect(rows).toHaveLength(3);
			// Each row names exactly one place, and it is its own.
			expect(rows[0]).toContain('Security key');
			expect(rows[1]).toContain('Phone or tablet');
			expect(rows[2]).toContain('This device');
			for (const row of rows.slice(0, 2)) expect(row).not.toContain('This device');
		});
	});
}
