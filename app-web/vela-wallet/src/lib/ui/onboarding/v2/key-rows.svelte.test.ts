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
import en from '../../../../../../../assets/i18n/en.json';
import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
import KeysScreen from './KeysScreen.svelte';
import DoneScreen from './DoneScreen.svelte';

const strings = (key: string, params?: Record<string, string | number>): string => {
	const value = key
		.split('.')
		.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
	if (typeof value !== 'string') throw new Error(`no corpus value for "${key}"`);
	return Object.entries(params ?? {}).reduce(
		(text, [name, fill]) => text.replaceAll(`{{${name}}}`, String(fill)),
		value
	);
};

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
			strings,
			onAddKey: () => {},
			onConfirmKey: () => {},
			onRemoveKey: () => {},
			onFinish: () => {}
		}
	});

const doneScreen = (keys: CreateKeyRow[]) =>
	render(DoneScreen, {
		props: {
			address: '0x71C7A4E9b2F03D8cA51e7F6d92B4c8035E9A3F1c',
			walletName: 'Everyday wallet',
			keys,
			strings,
			onEnter: () => {}
		}
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
			// The badge answers one question only, and it is not "where".
			expect(row).toContain('Not synced');
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
			expect(row).toContain('Synced');
			expect(row).not.toContain('Not synced');
		});

		it('draws no badge when nobody can vouch for the backup', () => {
			const { container } = mount([key({ synced: true, synced_known: false })]);
			const [row] = rowText(container, selector);
			expect(row).not.toContain('Synced');
			expect(row).not.toContain('Not synced');
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
