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
