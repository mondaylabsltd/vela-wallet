import { describe, expect, it } from 'vitest';
import { APP_STORE, GOOGLE_PLAY, appStoreLink, googlePlayLink } from './config';

const LIVE_APPLE = { appId: '6740000000', providerToken: '128117000' };
const LIVE_PLAY = { packageId: 'app.getvela.wallet', listed: true };

describe('the configured listings', () => {
	it('are not live until launch', () => {
		expect(APP_STORE).toEqual({ appId: null, providerToken: null });
		expect(GOOGLE_PLAY.listed).toBe(false);
	});

	it("name the Android app's package", () => {
		expect(GOOGLE_PLAY.packageId).toBe('app.getvela.wallet');
	});
});

describe('appStoreLink', () => {
	it('is null until both the app id and the provider token are set', () => {
		expect(appStoreLink('home', { appId: null, providerToken: null })).toBe(null);
		expect(appStoreLink('home', { appId: LIVE_APPLE.appId, providerToken: null })).toBe(null);
		expect(appStoreLink('home', { appId: null, providerToken: LIVE_APPLE.providerToken })).toBe(
			null
		);
	});

	it('names the web wallet and the placement in the campaign token', () => {
		expect(appStoreLink('home', LIVE_APPLE)).toBe(
			'https://apps.apple.com/app/id6740000000?pt=128117000&ct=webwallet-home&mt=8'
		);
		for (const placement of ['home', 'create_done', 'extension'] as const) {
			const url = new URL(appStoreLink(placement, LIVE_APPLE)!);
			expect(url.searchParams.get('ct')).toBe(`webwallet-${placement}`);
			expect(url.searchParams.get('ct')!.length).toBeLessThanOrEqual(40);
			expect(url.searchParams.get('pt')).toBe(LIVE_APPLE.providerToken);
			expect(url.searchParams.get('mt')).toBe('8');
		}
	});

	it('refuses an app id that is not a number', () => {
		expect(appStoreLink('home', { appId: 'id123', providerToken: '1' })).toBe(null);
	});
});

describe('googlePlayLink', () => {
	it('is null until listed', () => {
		expect(googlePlayLink('home', { ...LIVE_PLAY, listed: false })).toBe(null);
	});

	it('carries an install referrer naming the web wallet and the placement', () => {
		for (const placement of ['home', 'create_done', 'extension'] as const) {
			const url = new URL(googlePlayLink(placement, LIVE_PLAY)!);
			expect(url.origin + url.pathname).toBe('https://play.google.com/store/apps/details');
			expect(url.searchParams.get('id')).toBe('app.getvela.wallet');
			const referrer = new URLSearchParams(url.searchParams.get('referrer')!);
			expect(Object.fromEntries(referrer)).toEqual({
				utm_source: 'wallet.getvela.app',
				utm_medium: 'webwallet',
				utm_campaign: placement
			});
		}
		expect(googlePlayLink('create_done', LIVE_PLAY)).toBe(
			'https://play.google.com/store/apps/details?id=app.getvela.wallet&referrer=' +
				encodeURIComponent(
					'utm_source=wallet.getvela.app&utm_medium=webwallet&utm_campaign=create_done'
				)
		);
	});
});
