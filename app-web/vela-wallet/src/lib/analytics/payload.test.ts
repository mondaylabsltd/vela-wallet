import { describe, expect, it } from 'vitest';
import { EVENTS, PROPERTY_RULES, type AnalyticsProperty } from './catalog';
import {
	RYBBIT_SITE_ID,
	buildPayload,
	campaignQuery,
	carriesIdentifier,
	coarseReferrer,
	isTrackedPath,
	maskPath,
	pagePath,
	type PageContext
} from './payload';

const ADDRESS = '0x52908400098527886E0F7030069857D2E4169EE7';
const HASH = `0x${'ab'.repeat(32)}`;
const ENS = 'vitalik.eth';
const WEI = '1500000000000000000';
/** Every spelling of the address that would identify it. */
const ADDRESS_FORMS = [
	ADDRESS,
	ADDRESS.toLowerCase(),
	ADDRESS.slice(2),
	ADDRESS.slice(2).toLowerCase()
];

const CONTEXT: PageContext = {
	routeId: '/[locale]/wallet',
	pathname: '/ja/wallet',
	search: '',
	hostname: 'wallet.getvela.app',
	origin: 'https://wallet.getvela.app',
	language: 'ja-JP',
	screenWidth: 390,
	screenHeight: 844,
	referrer: '',
	surface: 'web'
};

function leaks(value: unknown, needles: readonly string[]): boolean {
	const text = JSON.stringify(value).toLowerCase();
	return needles.some((needle) => text.includes(needle.toLowerCase()));
}

describe('carriesIdentifier', () => {
	it.each([ADDRESS, ADDRESS.slice(2), HASH, HASH.slice(2), ENS, WEI, `/send/${ADDRESS}`])(
		'flags %s',
		(text) => expect(carriesIdentifier(text)).toBe(true)
	);
	it.each(['/ja/wallet', RYBBIT_SITE_ID, 'wallet.getvela.app', '{"chain":11155111}', 'en-US'])(
		'passes %s',
		(text) => expect(carriesIdentifier(text)).toBe(false)
	);
});

describe('page paths', () => {
	it('reports the route pattern with the locale put in', () => {
		expect(pagePath('/[locale]/wallet', '/ja/wallet')).toBe('/ja/wallet');
		expect(pagePath('/[locale]', '/zh-TW')).toBe('/zh-TW');
	});

	it('names a parameter instead of its value', () => {
		expect(pagePath('/[locale]/send/[to]', `/en/send/${ADDRESS}`)).toBe('/en/send/:to');
		expect(pagePath('/[locale]/[...rest]', `/en/${ADDRESS}/x`)).toBe('/en/:rest');
	});

	it('masks what is not a route word when there is no route id', () => {
		expect(maskPath('/ja/wallet.html')).toBe('/ja/wallet');
		expect(maskPath(`/en/wallet/${ADDRESS}`)).toBe('/en/wallet/:id');
		expect(maskPath(`/en/tx/${HASH}`)).toBe('/en/tx/:id');
		expect(maskPath(`/en/${ENS}`)).toBe('/en/:id');
		expect(maskPath('/en/12345')).toBe('/en/:id');
		expect(maskPath('/en/deadbeefcafe')).toBe('/en/:id');
		expect(maskPath('/en/wallet?to=0xabc#frag')).toBe('/en/wallet');
		expect(maskPath('/')).toBe('/');
	});

	it('leaves developer surfaces out', () => {
		expect(isTrackedPath('/en/gallery')).toBe(false);
		expect(isTrackedPath('/en/parallel')).toBe(false);
		expect(isTrackedPath('/dev/gallery')).toBe(false);
		expect(isTrackedPath('/en/wallet')).toBe(true);
		expect(buildPayload({ type: 'pageview' }, { ...CONTEXT, routeId: '/[locale]/gallery' })).toBe(
			null
		);
	});
});

describe('the query string', () => {
	it('carries the campaign tags and nothing else', () => {
		const search =
			`?to=${ADDRESS}&amount=1.5&utm_source=getvela.app&chain=8453&utm_medium=site` +
			'&utm_campaign=get-started&utm_content=internal-test&utm_term=wallet&panel&rid=7';
		expect(campaignQuery(search)).toBe(
			'?utm_source=getvela.app&utm_medium=site&utm_campaign=get-started' +
				'&utm_content=internal-test&utm_term=wallet'
		);
		const payload = buildPayload({ type: 'pageview' }, { ...CONTEXT, search });
		expect(payload?.querystring).toContain('utm_content=internal-test');
		expect(payload?.querystring).not.toContain('to=');
		expect(payload?.querystring).not.toContain('amount');
		expect(payload?.querystring).not.toContain('chain');
		expect(payload?.querystring).not.toContain('rid');
	});

	it('is empty when there is no tag', () => {
		expect(campaignQuery('')).toBe('');
		expect(campaignQuery(`?to=${ADDRESS}`)).toBe('');
		expect(
			buildPayload({ type: 'pageview' }, { ...CONTEXT, search: '?flow=receive' })?.querystring
		).toBe('');
	});

	it('drops a tag that is shaped like an identifier, or is not a short label', () => {
		expect(campaignQuery(`?utm_source=${ADDRESS}&utm_medium=site`)).toBe('?utm_medium=site');
		expect(campaignQuery(`?utm_term=${ENS}`)).toBe('');
		expect(campaignQuery(`?utm_term=${'x'.repeat(101)}`)).toBe('');
		expect(campaignQuery(`?utm_content=%30x${ADDRESS.slice(2)}`)).toBe('');
	});
});

describe('referrers', () => {
	it('keeps only the origin of another site', () => {
		expect(coarseReferrer('https://getvela.app/download?ref=x#y', CONTEXT.origin)).toBe(
			'https://getvela.app/'
		);
	});
	it('drops its own origin, non-web schemes and identifier-shaped hosts', () => {
		expect(coarseReferrer('https://wallet.getvela.app/en', CONTEXT.origin)).toBe('');
		expect(coarseReferrer('chrome-extension://abc/x', CONTEXT.origin)).toBe('');
		expect(coarseReferrer(`https://${ENS}.limo/`, CONTEXT.origin)).toBe('');
		expect(coarseReferrer('not a url', CONTEXT.origin)).toBe('');
	});
});

describe('buildPayload', () => {
	it("has Rybbit's shape and nothing else", () => {
		const payload = buildPayload(
			{ type: 'custom_event', name: 'send_submitted', properties: { chain: 8453 } },
			CONTEXT
		);
		expect(payload).toEqual({
			site_id: RYBBIT_SITE_ID,
			type: 'custom_event',
			hostname: 'wallet.getvela.app',
			pathname: '/ja/wallet',
			querystring: '',
			screenWidth: 390,
			screenHeight: 844,
			language: 'ja-JP',
			referrer: '',
			event_name: 'send_submitted',
			properties: JSON.stringify({ chain: 8453, surface: 'web', locale: 'ja' })
		});
	});

	it('sends a page view with no event fields', () => {
		const payload = buildPayload({ type: 'pageview' }, CONTEXT);
		expect(payload?.type).toBe('pageview');
		expect(payload).not.toHaveProperty('event_name');
		expect(payload).not.toHaveProperty('properties');
	});

	it('refuses an event the catalog does not name', () => {
		expect(buildPayload({ type: 'custom_event', name: 'whatever' }, CONTEXT)).toBe(null);
	});

	it('drops properties the event does not list, and values that break their rule', () => {
		const payload = buildPayload(
			{
				type: 'custom_event',
				name: 'send_failed',
				properties: { chain: 1, reason: 'boom', method: 'device', to: ADDRESS }
			},
			CONTEXT
		);
		expect(JSON.parse(payload!.properties!)).toEqual({ chain: 1, surface: 'web', locale: 'ja' });
	});

	it("lets a language change name the new locale, never the sender's surface", () => {
		const payload = buildPayload(
			{
				type: 'custom_event',
				name: 'setting_language_changed',
				properties: { locale: 'de', surface: 'extension' }
			},
			CONTEXT
		);
		expect(JSON.parse(payload!.properties!)).toEqual({ locale: 'de', surface: 'web' });
	});

	it('marks the extension as the extension', () => {
		const payload = buildPayload(
			{ type: 'custom_event', name: 'receive_opened' },
			{
				...CONTEXT,
				surface: 'extension',
				hostname: 'kbhkgllmfhaneaaplcbnmilgdfbmlmhk',
				origin: 'chrome-extension://kbhkgllmfhaneaaplcbnmilgdfbmlmhk',
				routeId: null,
				pathname: '/en/wallet.html'
			}
		);
		expect(payload?.pathname).toBe('/en/wallet');
		expect(JSON.parse(payload!.properties!)).toEqual({ surface: 'extension', locale: 'en' });
	});
});

/**
 * The guarantee: an address can never leave, whichever field it is put in —
 * the route, the path, the host, the language, the referrer, any property key
 * or value of any event.
 */
describe('an address never leaves', () => {
	const poisons = [ADDRESS, ADDRESS.toLowerCase(), HASH, ENS, WEI];
	const needles = [...ADDRESS_FORMS, HASH.slice(2), ENS, WEI];

	const contexts: PageContext[] = poisons.flatMap((poison) => [
		{ ...CONTEXT, routeId: `/[locale]/${poison}` },
		{ ...CONTEXT, routeId: null, pathname: `/en/send/${poison}` },
		{ ...CONTEXT, routeId: null, pathname: `/en/wallet?to=${poison}#${poison}` },
		{ ...CONTEXT, search: `?to=${poison}&amount=${poison}` },
		{ ...CONTEXT, search: `?utm_source=${poison}&utm_campaign=${encodeURIComponent(poison)}` },
		{ ...CONTEXT, pathname: `/en/wallet/${poison}` },
		{ ...CONTEXT, hostname: `${poison}.example` },
		{ ...CONTEXT, language: poison },
		{ ...CONTEXT, referrer: `https://dapp.example/pay/${poison}?to=${poison}` },
		{ ...CONTEXT, referrer: `https://${poison}.limo/` },
		{ ...CONTEXT, origin: poison }
	]);

	it('from any field of the page context, in any hit', () => {
		for (const context of contexts) {
			for (const name of Object.keys(EVENTS)) {
				const payload = buildPayload({ type: 'custom_event', name }, context);
				expect(leaks(payload, needles), `${name} ${JSON.stringify(context)}`).toBe(false);
			}
			expect(leaks(buildPayload({ type: 'pageview' }, context), needles)).toBe(false);
		}
	});

	it('from any property, of any event', () => {
		const keys = [...(Object.keys(PROPERTY_RULES) as AnalyticsProperty[]), 'to', 'address', 'hash'];
		for (const name of Object.keys(EVENTS)) {
			for (const poison of poisons) {
				for (const key of keys) {
					const payload = buildPayload(
						{ type: 'custom_event', name, properties: { [key]: poison } },
						CONTEXT
					);
					expect(leaks(payload, needles), `${name}.${key}`).toBe(false);
				}
				// …and as a key.
				const payload = buildPayload(
					{ type: 'custom_event', name, properties: { [poison]: 'device' } },
					CONTEXT
				);
				expect(leaks(payload, needles), `${name} key`).toBe(false);
			}
		}
	});

	it('even as a chain id that is really an amount', () => {
		const payload = buildPayload(
			{ type: 'custom_event', name: 'send_submitted', properties: { chain: Number(WEI) } },
			CONTEXT
		);
		expect(JSON.parse(payload!.properties!)).not.toHaveProperty('chain');
	});

	it('never sends a query string but campaign tags', () => {
		for (const context of contexts) {
			const payload = buildPayload({ type: 'pageview' }, context);
			if (payload !== null) expect(payload.querystring).toBe('');
		}
	});
});
