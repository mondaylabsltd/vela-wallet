/**
 * "Get Vela on your phone" as drawn: nothing without a listing, the platform's
 * own store on a phone, both stores with a code each on a desktop, and gone
 * for good once closed.
 */
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import AppPrompt from './AppPrompt.svelte';
import { appStoreLink, googlePlayLink, storeLinks } from './config';
import type { AppPromptMessages } from './messages';
import { DISMISSED_KEY, SUGGESTED_KEY, resetPromptVisitForTests } from './plan';

const MESSAGES: AppPromptMessages = {
	title: 'Get Vela on your phone',
	body: 'Vela is also an app for iPhone and Android.',
	scanHint: "Scan with your phone's camera",
	close: 'Close',
	appStore: 'App Store',
	googlePlay: 'Google Play'
};
const LINKS = {
	appStore: appStoreLink('home', { appId: '6740000000', providerToken: '128117000' }),
	googlePlay: googlePlayLink('home', { packageId: 'app.getvela.wallet', listed: true })
};

const prompt = () => document.body.querySelector('[data-testid="app-prompt"]');
const storeHrefs = () =>
	[...document.body.querySelectorAll('[data-testid="app-prompt"] a')].map((a) =>
		a.getAttribute('href')
	);

beforeEach(() => {
	localStorage.removeItem(DISMISSED_KEY);
	localStorage.removeItem(SUGGESTED_KEY);
	resetPromptVisitForTests();
});
afterEach(() => vi.restoreAllMocks());

describe('AppPrompt', () => {
	it('draws nothing while the listings are not live (today)', async () => {
		expect(storeLinks('home')).toEqual({ appStore: null, googlePlay: null });
		const screen = render(AppPrompt, {
			props: { placement: 'home', messages: MESSAGES, platform: 'desktop' }
		});
		await tick();
		expect(prompt()).toBe(null);
		await screen.unmount();
	});

	it('offers an iPhone the App Store alone, tagged as the web wallet’s', async () => {
		const screen = render(AppPrompt, {
			props: { placement: 'home', messages: MESSAGES, links: LINKS, platform: 'ios' }
		});
		await tick();
		expect(prompt()?.textContent).toContain(MESSAGES.title);
		expect(storeHrefs()).toEqual([LINKS.appStore]);
		expect(new URL(LINKS.appStore!).searchParams.get('ct')).toBe('webwallet-home');
		expect(document.body.querySelector('[data-testid="app-prompt"] svg[role="img"]')).toBe(null);
		await screen.unmount();
	});

	it('offers a desktop both stores, each with a code to scan', async () => {
		const screen = render(AppPrompt, {
			props: { placement: 'home', messages: MESSAGES, links: LINKS, platform: 'desktop' }
		});
		await tick();
		expect(storeHrefs()).toEqual([LINKS.appStore, LINKS.googlePlay]);
		const codes = document.body.querySelectorAll('[data-testid="app-prompt"] svg[role="img"]');
		expect([...codes].map((code) => code.getAttribute('aria-label'))).toEqual([
			'App Store',
			'Google Play'
		]);
		expect(prompt()?.textContent).toContain(MESSAGES.scanHint);
		await screen.unmount();
	});

	it('never comes back once closed', async () => {
		const first = render(AppPrompt, {
			props: { placement: 'home', messages: MESSAGES, links: LINKS, platform: 'android' }
		});
		await tick();
		expect(storeHrefs()).toEqual([LINKS.googlePlay]);
		(document.body.querySelector('[data-testid="app-prompt"] .close') as HTMLButtonElement).click();
		await tick();
		expect(prompt()).toBe(null);
		expect(localStorage.getItem(DISMISSED_KEY)).not.toBe(null);
		await first.unmount();

		resetPromptVisitForTests();
		const again = render(AppPrompt, {
			props: { placement: 'create_done', messages: MESSAGES, links: LINKS, platform: 'android' }
		});
		await tick();
		expect(prompt()).toBe(null);
		await again.unmount();
	});

	it('suggests on the creation screen once per device', async () => {
		const first = render(AppPrompt, {
			props: { placement: 'create_done', messages: MESSAGES, links: LINKS, platform: 'ios' }
		});
		await tick();
		expect(prompt()).not.toBe(null);
		await first.unmount();

		resetPromptVisitForTests();
		const second = render(AppPrompt, {
			props: { placement: 'create_done', messages: MESSAGES, links: LINKS, platform: 'ios' }
		});
		await tick();
		expect(prompt()).toBe(null);
		await second.unmount();
	});

	it('sends nothing from a test origin, whatever it shows', async () => {
		const sent = vi.spyOn(globalThis, 'fetch');
		const screen = render(AppPrompt, {
			props: { placement: 'home', messages: MESSAGES, links: LINKS, platform: 'desktop' }
		});
		await tick();
		(document.body.querySelector('[data-testid="app-prompt"] .close') as HTMLButtonElement).click();
		await tick();
		expect(sent.mock.calls.filter(([url]) => String(url).includes('appsdata'))).toEqual([]);
		await screen.unmount();
	});
});
