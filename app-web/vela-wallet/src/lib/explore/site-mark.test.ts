/**
 * No padlock for http (device walk of 2026-10-11, item 11).
 *
 * The phones' address bars drew a padlock on `http://127.0.0.1:8137`: the lock
 * was taken from "safe to use the wallet from", which is true of http on
 * loopback. The mark is the core's `OriginLock` now, and the web's four
 * surfaces that drew a padlock — two of them for every page, whatever it was —
 * draw it through one component that asks.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { resolveExploreMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { buildDesktopState, buildMobileState } from './fixtures';
import { originLock, siteMark } from './site-mark';

const m = resolveExploreMessages('en');
const zh = resolveExploreMessages('zh');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('the mark an address wears is the core’s word for its origin', () => {
	it('a padlock is for https, and for nothing else', () => {
		expect(originLock('https://app.uniswap.org')).toBe('secure');
		for (const origin of [
			'http://127.0.0.1:8137',
			'http://localhost:5173',
			'http://192.168.1.20:3000',
			'http://example.com',
			'http://app.uniswap.org'
		]) {
			expect(originLock(origin), origin).not.toBe('secure');
		}
	});

	it('http on this device or the person’s own network is local — not a padlock, not the warning', () => {
		for (const origin of [
			'http://127.0.0.1:8137',
			'http://localhost:5173',
			'http://192.168.1.20:3000',
			'http://10.0.0.5'
		]) {
			expect(originLock(origin), origin).toBe('local');
		}
	});

	it('public http is insecure, as it was', () => {
		expect(originLock('http://example.com')).toBe('insecure');
		expect(originLock('http://app.uniswap.org')).toBe('insecure');
	});

	it('says each in the corpus’s words, and names the mark that stands alone', () => {
		expect(siteMark('https://app.uniswap.org', m)).toEqual({
			lock: 'secure',
			words: 'Secure site',
			label: 'Secure site'
		});
		expect(siteMark('http://127.0.0.1:8137', m)).toEqual({
			lock: 'local',
			words: 'Local page',
			label: 'Local page — not encrypted'
		});
		expect(siteMark('http://example.com', m)).toEqual({
			lock: 'insecure',
			words: 'Insecure site — not encrypted',
			label: 'Insecure site — not encrypted'
		});
		expect(siteMark('http://127.0.0.1:8137', zh)).toMatchObject({
			words: '本地页面',
			label: '本地页面 — 未加密'
		});
	});

	it('a local page is never called secure, in any language', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const lm = resolveExploreMessages(locale);
			const mark = siteMark('http://127.0.0.1:8137', lm);
			expect(mark.words, locale).not.toBe(lm.secureSite);
			expect(mark.label, locale).not.toBe(lm.secureSite);
			expect(mark.words.trim(), locale).not.toBe('');
		}
	});
});

describe('the drawn boards wear the mark their own page earns', () => {
	it('the phone’s browser, its site menu and its connection panel: https, a padlock’s words', () => {
		const model = buildMobileState('e4', m, identicon);
		expect(model.browser.url.startsWith('https://')).toBe(true);
		expect(model.browser.mark).toEqual(siteMark('https://app.uniswap.org', m));
		expect(model.menus.siteMenu.mark?.lock).toBe('secure');
		expect(model.menus.siteMenu.statusLine).toBe('Secure site');
		expect(model.menus.connection.connection.mark.lock).toBe('secure');
		expect(model.menus.connection.connection.statusLine).toBe('Secure site · Connected');
	});

	it('a history row’s menu claims nothing about a connection: a host, and no mark', () => {
		const recent = buildMobileState('e8', m, identicon).menus.recentMenu;
		expect(recent.mark).toBeUndefined();
		expect(recent.statusLine).not.toBe(m.secureSite);
	});

	it('the wide layout’s toolbar and panel, the same', () => {
		const model = buildDesktopState('de3', m, identicon);
		expect(model.browser.mark.lock).toBe('secure');
		expect(model.connection.mark.lock).toBe('secure');
	});
});

/** Every `.svelte` and `.ts` under `dir`, tests aside. */
function sources(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return name === 'generated' ? [] : sources(path);
		return /\.(svelte|ts)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
	});
}

describe('nothing else draws a padlock or says "Secure site"', () => {
	const all = sources('src').map((path) => [path, readFileSync(path, 'utf8')] as const);

	it('the padlock icon is drawn by `SiteMark.svelte` alone', () => {
		const users = all
			.filter(([, source]) => /UTILITY_ICONS\.lock\b|UTILITY_ICONS\['lock'\]/.test(source))
			.map(([path]) => path);
		expect(users).toEqual(['src/lib/explore/ui/SiteMark.svelte']);
	});

	it('"Secure site" is said by `site-mark.ts` alone — for the `secure` lock', () => {
		const users = all
			.filter(([, source]) => /\bm\.secureSite\b|copy\.secureSite\b/.test(source))
			.map(([path]) => path);
		expect(users).toEqual(['src/lib/explore/site-mark.ts']);
	});

	// The extension's connect surface names the site by its host and draws no
	// mark at all — so it cannot draw the wrong one. Held here, because the
	// day it gains a mark it must come from the same place.
	it('the request window and its consent facts draw no mark of their own', () => {
		for (const path of [
			'src/lib/dapp/DappRequestHost.svelte',
			'src/lib/dapp/ConsentFacts.svelte'
		]) {
			const source = readFileSync(path, 'utf8');
			expect(source, path).not.toMatch(/UTILITY_ICONS\.(lock|info)\b|triangle-alert|secureSite/);
		}
	});
});
