/**
 * The key-place chooser, in a real browser (spec 075, spec 102).
 *
 * One list serves three places — the FIRST founding key, "add another key",
 * and the sign-in sheet on Welcome — so what it offers is what every one of
 * them offers.
 *
 * Spec 102: THREE places and no fourth. The Trusted Signer sat here as a
 * fourth "place", and a person who tapped it was asked "this device / scan a
 * code / USB key" all over again — a signing page is where a person reviews
 * and signs, not where a key lives. Beside the three, the apps offer "Use a
 * trusted signing page" (D6); the web does not (P2b-W3): it opens no signing
 * page, so a self-hosted page's ceremonies could never run here, and the
 * official page's sign in Vela on the web anyway (P2-11).
 *
 * A `.svelte.test.ts` because it is about what a person sees, and the strings
 * are the REAL corpus: a chooser that reads well with invented copy proves
 * nothing about the screen that ships.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import en from '../../../../../../../assets/i18n/en.json';
import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
import AddMethodPicker from './AddMethodPicker.svelte';

const strings = (key: string, params?: Record<string, string | number>): string => {
	const value = key
		.split('.')
		.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], en);
	if (typeof value !== 'string') throw new Error(`no corpus value for "${key}"`);
	return Object.entries(params ?? {}).reduce(
		(text, [name, replacement]) => text.replaceAll(`{{${name}}}`, String(replacement)),
		value
	);
};

function drawn(options: { allowed?: KeyMethod[]; chooser?: 'create' | 'sign_in' } = {}) {
	const picked: KeyMethod[] = [];
	const screen = render(AddMethodPicker, {
		props: {
			open: true,
			strings,
			onPick: (method: KeyMethod) => picked.push(method),
			...(options.allowed ? { allowed: options.allowed } : {}),
			...(options.chooser ? { chooser: options.chooser } : {})
		}
	});
	const buttons = [...screen.container.querySelectorAll<HTMLButtonElement>('button.method')];
	return {
		picked,
		buttons,
		names: buttons.map((button) => button.querySelector('.name')?.textContent?.trim()),
		captions: buttons.map((button) => button.querySelector('.caption')?.textContent?.trim())
	};
}

/**
 * Issue 475: every row's caption is ONE line. Three rows that each wrap to
 * two read as six lines of caption and stop lining up; the corpus was
 * shortened so none has to. Measured here in all fifteen languages, in the
 * width a 390 px phone leaves the list (its two 24 px gutters off), for both
 * choosers — so a translation that grows past the line fails here, not on
 * somebody's phone.
 */
const CATALOGS = import.meta.glob('../../../../../../../assets/i18n/*.json', {
	import: 'default',
	eager: true
}) as Record<string, unknown>;

const stringsOf =
	(catalog: unknown) =>
	(key: string): string => {
		const value = key
			.split('.')
			.reduce<unknown>((node, part) => (node as Record<string, unknown>)?.[part], catalog);
		if (typeof value !== 'string') throw new Error(`no corpus value for "${key}"`);
		return value;
	};

describe('every caption fits one line on a 390 px phone (issue 475)', () => {
	const locales = Object.entries(CATALOGS).map(
		([path, catalog]) => [/\/([^/]+)\.json$/.exec(path)![1], catalog] as const
	);

	it('covers all fifteen languages', () => {
		expect(locales).toHaveLength(15);
	});

	for (const chooser of ['create', 'sign_in'] as const) {
		it.each(locales)(`%s — ${chooser}`, async (locale, catalog) => {
			const screen = render(AddMethodPicker, {
				props: { open: true, strings: stringsOf(catalog), onPick: () => {}, chooser }
			});
			screen.container.style.width = '342px';
			await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
			const rows = [...screen.container.querySelectorAll<HTMLElement>('button.method')];
			expect(rows).toHaveLength(3);
			for (const row of rows) {
				const caption = row.querySelector('.caption') as HTMLElement;
				const name = row.querySelector('.name') as HTMLElement;
				const text = `${locale}: ${caption.textContent}`;
				// One line, and all of it: nothing is cut to an ellipsis.
				expect(caption.scrollWidth, text).toBeLessThanOrEqual(caption.clientWidth);
				expect(caption.getBoundingClientRect().height, text).toBeLessThan(
					parseFloat(getComputedStyle(caption).fontSize) * 2
				);
				// The name above it too.
				expect(name.getBoundingClientRect().height, `${locale}: ${name.textContent}`).toBeLessThan(
					parseFloat(getComputedStyle(name).fontSize) * 2
				);
				// Settings' row metrics: at least a control tall.
				expect(row.getBoundingClientRect().height).toBeGreaterThanOrEqual(52);
			}
			await screen.unmount();
		});
	}
});

describe('the key-place chooser', () => {
	it('offers the three places a key can live — and no fourth', () => {
		const view = drawn();
		expect(view.names).toEqual([
			strings('onboarding.create.methodPlatformTitle'),
			strings('onboarding.create.methodHybridTitle'),
			strings('onboarding.create.methodSecurityKeyTitle')
		]);
		expect(view.buttons).toHaveLength(3);
		// The words the fourth row wore are gone from the corpus altogether.
		const signing = (en as { componentsUi: { signing: Record<string, unknown> } }).componentsUi
			.signing;
		expect(signing.trustedSignerTitle).toBeUndefined();
	});

	it('offers no "Use a trusted signing page" — not to create, not to sign in (P2b-W3)', () => {
		// The apps' entry, in the corpus's words (read here, never drawn: the
		// onboarding surfaces' literal scan must not count it as requested).
		const entry = ['onboarding.create.signingPageTitle', 'onboarding.create.signingPageBody'].map(
			(key) => strings(key)
		);
		for (const chooser of ['create', 'sign_in'] as const) {
			const view = drawn({ chooser });
			expect(view.buttons, chooser).toHaveLength(3);
			const text = `${view.names.join(' ')} ${view.captions.join(' ')}`;
			for (const words of entry) expect(text, chooser).not.toContain(words);
		}
	});

	it('hands the core the place the person tapped', () => {
		const view = drawn();
		view.buttons.at(-1)?.click();
		view.buttons[0]?.click();
		expect(view.picked).toEqual(['security_key', 'platform']);
	});
});
