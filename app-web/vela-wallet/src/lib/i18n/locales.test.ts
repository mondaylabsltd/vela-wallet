import { describe, expect, it } from 'vitest';
import {
	FALLBACK_LOCALE,
	SUPPORTED_LOCALES,
	languageCookie,
	localeOfPath,
	negotiate,
	negotiateLanguages,
	pinnedLocale,
	toLocale,
	usesCjkFont
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

describe('usesCjkFont (perf-9)', () => {
	it('links the CJK face on the Chinese, Japanese and Korean pages only', () => {
		const cjk = SUPPORTED_LOCALES.filter((locale) => usesCjkFont(locale));
		expect(cjk.sort()).toEqual(['ja', 'ko', 'zh', 'zh-HK', 'zh-TW']);
		expect(usesCjkFont('en')).toBe(false);
		expect(usesCjkFont('vi')).toBe(false);
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

describe('the pinned-language cookie `/` reads (set-12)', () => {
	it('pins a supported tag in its canonical case, and clears for anything else', () => {
		expect(languageCookie('zh-tw', true)).toBe(
			'vela_lang=zh-TW; Max-Age=31536000; Path=/; SameSite=Lax; Secure'
		);
		expect(languageCookie('de', false)).toBe(
			'vela_lang=de; Max-Age=31536000; Path=/; SameSite=Lax'
		);
		expect(languageCookie('auto', false)).toBe('vela_lang=; Max-Age=0; Path=/; SameSite=Lax');
		expect(languageCookie('xx', false)).toBe('vela_lang=; Max-Age=0; Path=/; SameSite=Lax');
	});

	it('reads the pin out of a Cookie header, and trusts only a supported tag', () => {
		expect(pinnedLocale('a=1; vela_lang=ja; b=2')).toBe('ja');
		expect(pinnedLocale('vela_lang=pt-br')).toBe('pt-BR');
		expect(pinnedLocale('vela_lang=')).toBeUndefined();
		expect(pinnedLocale('vela_lang=<script>')).toBeUndefined();
		expect(pinnedLocale('other=ja')).toBeUndefined();
		expect(pinnedLocale(null)).toBeUndefined();
	});
});
