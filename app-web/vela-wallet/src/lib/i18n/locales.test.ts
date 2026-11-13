import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	FALLBACK_LOCALE,
	SUPPORTED_LOCALES,
	localeOfPath,
	negotiate,
	negotiateLanguages,
	pinnedLocaleTarget,
	toLocale
} from './locales';

describe('registry', () => {
	it('ships exactly the 15 corpus locales', () => {
		expect(SUPPORTED_LOCALES).toHaveLength(15);
		expect(new Set(SUPPORTED_LOCALES).size).toBe(15);
		expect(FALLBACK_LOCALE).toBe('en');
	});

	it('toLocale is case-insensitive and returns canonical case', () => {
		expect(toLocale('ZH-tw')).toBe('zh-TW');
		expect(toLocale('es-mx')).toBe('es-MX');
		expect(toLocale('xx')).toBeUndefined();
	});
});

describe('negotiate — the RN detectSystemLanguage table over Accept-Language', () => {
	const table: Array<[string, string]> = [
		// exact tags
		['en', 'en'],
		['ja', 'ja'],
		['zh-HK', 'zh-HK'],
		['es-MX', 'es-MX'],
		// zh script/region discrimination
		['zh-CN', 'zh'],
		['zh-SG', 'zh'],
		['zh-Hans-CN', 'zh'],
		['zh-Hant', 'zh-TW'],
		['zh-Hant-TW', 'zh-TW'],
		['zh-TW', 'zh-TW'],
		['zh-MO', 'zh-HK'],
		['zh-Hant-HK', 'zh-HK'],
		// single-variant languages absorb every region
		['es-AR', 'es-MX'],
		['es', 'es-MX'],
		['pt-PT', 'pt-BR'],
		['pt', 'pt-BR'],
		// other supported bases keep their base regardless of region
		['fr-CA', 'fr'],
		['de-AT', 'de'],
		['ru-BY', 'ru'],
		// legacy Android Indonesian
		['in', 'id'],
		['in-ID', 'id'],
		// unsupported → fallback
		['th', 'en'],
		['ar-EG', 'en']
	];

	for (const [header, expected] of table) {
		it(`${header} → ${expected}`, () => {
			expect(negotiate(header)).toBe(expected);
		});
	}

	it('honors q-ordering, not listing order', () => {
		expect(negotiate('fr;q=0.5, ja;q=0.9')).toBe('ja');
		expect(negotiate('de, ja;q=0.9')).toBe('de');
	});

	it('skips unsupported candidates and takes the next by quality', () => {
		expect(negotiate('th, vi;q=0.8, ja;q=0.7')).toBe('vi');
	});

	it('is case-insensitive', () => {
		expect(negotiate('ZH-hant-tw')).toBe('zh-TW');
		expect(negotiate('PT-br')).toBe('pt-BR');
	});

	it('falls back to en on empty, missing, wildcard, or junk headers', () => {
		expect(negotiate(null)).toBe('en');
		expect(negotiate(undefined)).toBe('en');
		expect(negotiate('')).toBe('en');
		expect(negotiate('*')).toBe('en');
		expect(negotiate(';;;,,q=')).toBe('en');
	});

	it('ignores q=0 (explicitly refused) candidates', () => {
		expect(negotiate('ja;q=0, vi;q=0.5')).toBe('vi');
	});
});

describe('localeOfPath (issue 317)', () => {
	it('reads the locale a page’s path names, on the site and in the packaged extension', () => {
		expect(localeOfPath('/ja/wallet')).toBe('ja');
		expect(localeOfPath('/ja/wallet.html')).toBe('ja');
		expect(localeOfPath('/zh-TW/settings.html')).toBe('zh-TW');
		expect(localeOfPath('/pt-br.html')).toBe('pt-BR');
		expect(localeOfPath('/de')).toBe('de');
		for (const locale of SUPPORTED_LOCALES) {
			expect(localeOfPath(`/${locale}/request.html`)).toBe(locale);
		}
	});

	it('names nothing for a path no locale page has', () => {
		expect(localeOfPath('/')).toBeUndefined();
		expect(localeOfPath('/panel.html')).toBeUndefined();
		expect(localeOfPath('/app/immutable/x.js')).toBeUndefined();
		expect(localeOfPath('')).toBeUndefined();
	});
});

describe('negotiateLanguages — what "Follow system" follows (set-13)', () => {
	it('takes the first of the browser’s languages the table supports', () => {
		expect(negotiateLanguages(['th', 'de-AT', 'en'])).toBe('de');
		expect(negotiateLanguages(['zh-Hant-HK'])).toBe('zh-HK');
		expect(negotiateLanguages(['pt-PT', 'ja'])).toBe('pt-BR');
	});

	it('falls back to en when the browser names nothing it ships', () => {
		expect(negotiateLanguages(['th', 'he'])).toBe(FALLBACK_LOCALE);
		expect(negotiateLanguages([])).toBe(FALLBACK_LOCALE);
		expect(negotiateLanguages(undefined)).toBe(FALLBACK_LOCALE);
	});
});

describe('a pinned language, honoured by the locale page (set-12)', () => {
	it('moves to the pinned locale, keeping the rest of the path', () => {
		expect(pinnedLocaleTarget('/ja', 'de')).toBe('/de');
		expect(pinnedLocaleTarget('/ja/wallet', 'zh-tw')).toBe('/zh-TW/wallet');
		expect(pinnedLocaleTarget('/en/settings/', 'pt-BR')).toBe('/pt-BR/settings/');
	});

	it('stays for no pin, `auto`, an unknown tag, the same locale, or a path with no locale', () => {
		expect(pinnedLocaleTarget('/ja/wallet', null)).toBeNull();
		expect(pinnedLocaleTarget('/ja/wallet', 'auto')).toBeNull();
		expect(pinnedLocaleTarget('/ja/wallet', 'xx')).toBeNull();
		expect(pinnedLocaleTarget('/ja/wallet', 'JA')).toBeNull();
		expect(pinnedLocaleTarget('/clientmcp', 'de')).toBeNull();
		expect(pinnedLocaleTarget('/', 'de')).toBeNull();
	});

	/** The copy in `app.html`, run with a page's location and storage. */
	function inline(
		href: string,
		pinned: string | null
	): { replaced: string | null; hidden: boolean } {
		const html = readFileSync(new URL('../../app.html', import.meta.url), 'utf8');
		const block = html.slice(html.indexOf('/* set-12:begin */'), html.indexOf('/* set-12:end */'));
		const url = new URL(href);
		let replaced: string | null = null;
		const style: Record<string, string> = {};
		const location = {
			protocol: url.protocol,
			pathname: url.pathname,
			search: url.search,
			hash: url.hash,
			replace: (to: string) => (replaced = to)
		};
		const storage = { getItem: (key: string) => (key === 'vela.language' ? pinned : null) };
		new Function('location', 'localStorage', 'document', block)(location, storage, {
			documentElement: { style }
		});
		return { replaced, hidden: style.visibility === 'hidden' };
	}

	it('is what app.html does before the first paint — hidden while it hops', () => {
		const cases: Array<[string, string | null]> = [
			['https://getvela.app/ja', 'de'],
			['https://getvela.app/ja/wallet', 'zh-tw'],
			['https://getvela.app/en/settings/', 'pt-BR'],
			['https://getvela.app/ja/wallet', null],
			['https://getvela.app/ja/wallet', 'auto'],
			['https://getvela.app/ja/wallet', 'xx'],
			['https://getvela.app/ja/wallet', 'ja'],
			['https://getvela.app/clientmcp', 'de']
		];
		for (const [href, pinned] of cases) {
			const url = new URL(href);
			const target = pinnedLocaleTarget(url.pathname, pinned);
			const ran = inline(href, pinned);
			expect(ran.replaced).toBe(target);
			expect(ran.hidden).toBe(target !== null);
		}
		// The query and the fragment ride along (campaign links, deep links).
		expect(inline('https://getvela.app/ja/wallet?utm_source=x#top', 'fr').replaced).toBe(
			'/fr/wallet?utm_source=x#top'
		);
		// The extension's pages have their own language doorways.
		expect(inline('chrome-extension://abc/ja/wallet.html', 'de').replaced).toBeNull();
	});

	it('knows exactly the locales this build ships', () => {
		const html = readFileSync(new URL('../../app.html', import.meta.url), 'utf8');
		const block = html.slice(html.indexOf('/* set-12:begin */'), html.indexOf('/* set-12:end */'));
		const list = block.slice(block.indexOf('['), block.indexOf(']') + 1);
		expect(JSON.parse(list.replace(/'/g, '"').replace(/,\s*]/, ']'))).toEqual([
			...SUPPORTED_LOCALES
		]);
	});
});
