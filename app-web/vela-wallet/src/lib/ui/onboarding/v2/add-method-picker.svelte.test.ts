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
 * and signs, not where a key lives. Beside the three, a shell that can OPEN a
 * page may offer the advanced "Use my own signing page"; the web cannot (owner,
 * 2026-09-23), so its live flows never pass it — the gallery does, as the
 * design the phones build to.
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

function drawn(options: { ownPage?: () => void; allowed?: KeyMethod[] } = {}) {
	const picked: KeyMethod[] = [];
	const screen = render(AddMethodPicker, {
		props: {
			open: true,
			strings,
			onPick: (method: KeyMethod) => picked.push(method),
			...(options.allowed ? { allowed: options.allowed } : {}),
			...(options.ownPage ? { ownPage: { onPick: options.ownPage } } : {})
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

	it('the web’s live flows draw no "own signing page" (it opens no page)', () => {
		expect(drawn().names).not.toContain(strings('onboarding.create.ownPageTitle'));
	});

	it('a shell that opens pages draws it below the three, and it is not a place', () => {
		let chosen = 0;
		const view = drawn({ ownPage: () => (chosen += 1) });
		expect(view.buttons).toHaveLength(4);
		expect(view.names.slice(0, 3)).toEqual(drawn().names);
		expect(view.names[3]).toBe(strings('onboarding.create.ownPageTitle'));
		expect(view.captions[3]).toBe(strings('onboarding.create.ownPageBody'));
		view.buttons[3]?.click();
		expect(chosen).toBe(1);
		// Choosing it picks no place.
		expect(view.picked).toEqual([]);
	});

	it('hands the core the place the person tapped', () => {
		const view = drawn();
		view.buttons.at(-1)?.click();
		view.buttons[0]?.click();
		expect(view.picked).toEqual(['security_key', 'platform']);
	});
});
