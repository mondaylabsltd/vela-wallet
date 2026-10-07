import { describe, expect, it } from 'vitest';
import {
	APP_STORE,
	GOOGLE_PLAY,
	appStoreLink,
	googlePlayLink,
	storeLinks,
	webWalletLink
} from './app-links';

const LIVE_APPLE = { appId: '6740000000', providerToken: '128117000' };
const LIVE_PLAY = { packageId: 'app.getvela.wallet', listed: true };

describe('webWalletLink', () => {
	it('tags the visit with this site and the place that linked it', () => {
		const url = new URL(webWalletLink('get-started-web'));
		expect(url.origin).toBe('https://wallet.getvela.app');
		expect(Object.fromEntries(url.searchParams)).toEqual({
			utm_source: 'getvela.app',
			utm_medium: 'site',
			utm_campaign: 'get-started-web'
		});
	});
});

describe('store links', () => {
	it('are not live until launch — the page keeps saying "coming soon"', () => {
		expect(APP_STORE).toEqual({ appId: null, providerToken: null });
		expect(GOOGLE_PLAY).toEqual({ packageId: 'app.getvela.wallet', listed: false });
		expect(storeLinks('get-started')).toEqual([]);
	});

	it('App Store: null until both the app id and the provider token are set', () => {
		expect(appStoreLink('get-started', { appId: LIVE_APPLE.appId, providerToken: null })).toBe(
			null
		);
		expect(
			appStoreLink('get-started', { appId: null, providerToken: LIVE_APPLE.providerToken })
		).toBe(null);
	});

	it('App Store: names the site and the place in the campaign token', () => {
		expect(appStoreLink('get-started', LIVE_APPLE)).toBe(
			'https://apps.apple.com/app/id6740000000?pt=128117000&ct=site-get-started&mt=8'
		);
		const ct = new URL(
			appStoreLink('a-very-long-location-name-on-the-site', LIVE_APPLE)!
		).searchParams.get('ct')!;
		expect(ct.length).toBeLessThanOrEqual(40);
		expect(ct.startsWith('site-')).toBe(true);
	});

	it('Google Play: an install referrer naming the site and the place', () => {
		const url = new URL(googlePlayLink('get-started', LIVE_PLAY)!);
		expect(url.searchParams.get('id')).toBe('app.getvela.wallet');
		expect(Object.fromEntries(new URLSearchParams(url.searchParams.get('referrer')!))).toEqual({
			utm_source: 'getvela.app',
			utm_medium: 'site',
			utm_campaign: 'get-started'
		});
		expect(googlePlayLink('get-started', { ...LIVE_PLAY, listed: false })).toBe(null);
	});
});
