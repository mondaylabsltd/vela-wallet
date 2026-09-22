/**
 * The key-method chooser, in a real browser (spec 075).
 *
 * One list serves three places — the FIRST founding key, "add another key",
 * and the sign-in sheet on Welcome — so what it offers is what every one of
 * them offers. Since 075 that is four routes, not three: the Clear Signer is a
 * passkey route of our own, and the owner's words were that it is a peer of
 * this device / a phone or tablet / a security key, offered wherever they are.
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
import type { AddBlocked } from '$lib/onboarding/generated/AddBlocked';
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

function drawn(narrow?: { allowed: KeyMethod[]; blocked: AddBlocked }) {
	const picked: KeyMethod[] = [];
	const screen = render(AddMethodPicker, {
		props: {
			open: true,
			strings,
			onPick: (method: KeyMethod) => picked.push(method),
			...(narrow ? { allowed: narrow.allowed, blocked: narrow.blocked } : {})
		}
	});
	const buttons = [...screen.container.querySelectorAll<HTMLButtonElement>('button.method')];
	return {
		picked,
		buttons,
		reason: [...screen.container.querySelectorAll('.reason')].map(
			(line) => line.textContent?.trim() ?? ''
		),
		names: buttons.map((button) => button.querySelector('.name')?.textContent?.trim()),
		captions: buttons.map((button) => button.querySelector('.caption')?.textContent?.trim())
	};
}

describe('the key-method chooser', () => {
	it('offers four routes, the Clear Signer last, each with its own line', () => {
		const view = drawn();
		expect(view.names).toEqual([
			strings('onboarding.create.methodPlatformTitle'),
			strings('onboarding.create.methodHybridTitle'),
			strings('onboarding.create.methodSecurityKeyTitle'),
			strings('componentsUi.signing.clearSignerTitle')
		]);
		expect(view.captions.at(-1)).toBe(strings('componentsUi.signing.clearSignerBody'));
	});

	it('the Clear Signer wears the glyph of a page you read', () => {
		const view = drawn();
		// `eye`, the one route that is not a place a passkey is but a page that
		// shows what it is about to do.
		const last = view.buttons.at(-1);
		expect(last?.querySelector('svg')).not.toBeNull();
		expect(last?.innerHTML).toContain('circle');
	});

	it('a route that would mint for another site is off, and says why', () => {
		// Spec 075, the owner's report of 2026-09-23: a set of `getvela.app`
		// keys plus a signer page on `localhost`. The page cannot add to this
		// set — it would mint a key no unit accepts — and a person who reads
		// this row needs to know the page is a setting they can change.
		const view = drawn({
			allowed: ['platform', 'hybrid', 'security_key'],
			blocked: {
				relying_party: 'getvela.app',
				page: 'http://localhost:8140/sign.html',
				page_relying_party: 'localhost'
			}
		});
		expect(view.buttons.map((button) => button.disabled)).toEqual([false, false, false, true]);
		view.buttons.at(-1)?.click();
		expect(view.picked).toEqual([]);
		// Two paragraphs: what this wallet's keys belong to, then the page to
		// change — never joined, which would put a space after a full stop
		// that already ends the line in Chinese.
		expect(view.reason).toHaveLength(2);
		expect(view.reason[0]).toContain('getvela.app');
		expect(view.reason[1]).toContain('http://localhost:8140/sign.html');
		expect(view.reason[1]).toContain('localhost');
	});

	it('says nothing extra while every route still fits', () => {
		expect(drawn().reason).toEqual([]);
	});

	it('hands the core the route the person tapped', () => {
		const view = drawn();
		view.buttons.at(-1)?.click();
		view.buttons[0]?.click();
		expect(view.picked).toEqual(['clear_signer', 'platform']);
	});
});
