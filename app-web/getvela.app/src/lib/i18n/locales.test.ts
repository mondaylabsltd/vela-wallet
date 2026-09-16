import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
	DEFAULT_LOCALE,
	LOCALES,
	PREFIXED_LOCALES,
	SUPPORTED_LOCALES,
	isLocale,
	negotiate,
	pathFor,
	resolveAlias,
	splitLocalePath,
	switchTo,
	toLocale
} from './locales';

/**
 * FR-007: the site's locale set must equal the wallet's, forever.
 *
 * This reads the wallet's source files off disk and parses the literals out of
 * them rather than importing them — `app-web/vela-wallet` is a separate Vite
 * project with its own aliases, and importing across the boundary would drag in
 * that app's module graph. Parsing text is crude and that is the point: it
 * fails loudly the moment somebody adds a sixteenth language on one side only.
 */
function walletSource(relative: string): string {
	return readFileSync(fileURLToPath(new URL(`../../../../vela-wallet/${relative}`, import.meta.url)), 'utf8');
}

function parseWalletLocales(): string[] {
	const src = walletSource('src/lib/i18n/locales.ts');
	const block = src.match(/export const SUPPORTED_LOCALES = \[([\s\S]*?)\] as const;/);
	expect(block, 'SUPPORTED_LOCALES not found in the wallet app').not.toBeNull();
	return [...block![1].matchAll(/'([^']+)'/g)].map((m) => m[1]);
}

function parseWalletEndonyms(): Map<string, string> {
	const src = walletSource('src/lib/settings/fixtures.ts');
	const block = src.match(/export const LOCALE_ENDONYMS[\s\S]*?=\s*\[([\s\S]*?)\];/);
	expect(block, 'LOCALE_ENDONYMS not found in the wallet app').not.toBeNull();
	return new Map(
		[...block![1].matchAll(/\{\s*id:\s*'([^']+)',\s*label:\s*'([^']+)'\s*\}/g)].map((m) => [
			m[1],
			m[2]
		])
	);
}

describe('locale parity with the wallet (FR-007)', () => {
	it('has exactly the wallet’s fifteen tags', () => {
		const wallet = parseWalletLocales();
		expect(wallet.length).toBe(15);
		expect([...SUPPORTED_LOCALES].sort()).toEqual([...wallet].sort());
	});

	it('names each language the way the wallet’s picker does', () => {
		const wallet = parseWalletEndonyms();
		for (const tag of SUPPORTED_LOCALES) {
			expect(wallet.get(tag), `no endonym for ${tag} in the wallet`).toBe(LOCALES[tag].endonym);
		}
	});

	it('describes every supported tag exactly once', () => {
		expect(Object.keys(LOCALES).sort()).toEqual([...SUPPORTED_LOCALES].sort());
		expect(new Set(SUPPORTED_LOCALES).size).toBe(SUPPORTED_LOCALES.length);
	});

	it('gives English no prefix and everyone else one', () => {
		expect(DEFAULT_LOCALE).toBe('en');
		expect(PREFIXED_LOCALES).toHaveLength(14);
		expect(PREFIXED_LOCALES).not.toContain('en');
	});

	it('has no right-to-left locale yet — a sixteenth would need layout work', () => {
		expect(Object.values(LOCALES).every((l) => l.dir === 'ltr')).toBe(true);
	});
});

describe('tag recognition', () => {
	it('accepts a canonical tag in any casing', () => {
		expect(isLocale('pt-BR')).toBe(true);
		expect(isLocale('PT-br')).toBe(true);
		expect(toLocale('PT-br')).toBe('pt-BR');
	});

	it('rejects what is not a tag', () => {
		expect(isLocale('xx')).toBe(false);
		expect(toLocale('')).toBeUndefined();
		expect(toLocale(undefined)).toBeUndefined();
	});
});

describe('alias resolution (contracts/routing.md §Must 308)', () => {
	it('maps a bare language to the catalog that serves it', () => {
		expect(resolveAlias('pt')).toBe('pt-BR');
		expect(resolveAlias('es')).toBe('es-MX');
		expect(resolveAlias('zh-CN')).toBe('zh');
		expect(resolveAlias('zh-Hans')).toBe('zh');
		expect(resolveAlias('zh-SG')).toBe('zh');
		expect(resolveAlias('zh-Hant')).toBe('zh-TW');
		expect(resolveAlias('zh-MO')).toBe('zh-TW');
	});

	it('falls back to the base language for an unlisted region', () => {
		expect(resolveAlias('fr-CA')).toBe('fr');
		expect(resolveAlias('de-AT')).toBe('de');
		expect(resolveAlias('ja-JP')).toBe('ja');
	});

	it('fixes casing', () => {
		expect(resolveAlias('PT-BR')).toBe('pt-BR');
		expect(resolveAlias('ZH-tw')).toBe('zh-TW');
	});

	it('leaves a canonical tag alone', () => {
		expect(resolveAlias('pt-BR')).toBeUndefined();
		expect(resolveAlias('ja')).toBeUndefined();
	});

	it('does not invent a locale for an unknown tag', () => {
		expect(resolveAlias('xx')).toBeUndefined();
		expect(resolveAlias('klingon')).toBeUndefined();
	});

	it('resolves `en` to English so `/en/` can be redirected, not served', () => {
		// `/en/` must never render (FR-008); the router 404s it and the alias
		// table is what a future redirect would consult.
		expect(toLocale('EN')).toBe('en');
	});
});

describe('negotiation — offer banner only, never a redirect (R3, FR-013)', () => {
	it('reads an Accept-Language header by quality', () => {
		expect(negotiate('ja;q=0.9,en;q=0.8')).toBe('ja');
		expect(negotiate('en-US,en;q=0.9')).toBe('en');
	});

	it('reads navigator.languages', () => {
		expect(negotiate(['pt-PT', 'en'])).toBe('pt-BR');
		expect(negotiate(['fr-CA'])).toBe('fr');
	});

	it('falls back to English for nothing, wildcards and strangers', () => {
		expect(negotiate(null)).toBe('en');
		expect(negotiate('')).toBe('en');
		expect(negotiate('*')).toBe('en');
		expect(negotiate(['xx', 'yy'])).toBe('en');
	});

	it('skips a q=0 rejection', () => {
		expect(negotiate('ja;q=0,de;q=0.5')).toBe('de');
	});
});

describe('paths (FR-008, FR-012)', () => {
	it('leaves English unprefixed', () => {
		expect(pathFor('en', '/')).toBe('/');
		expect(pathFor('en', '/docs/faq')).toBe('/docs/faq');
	});

	it('prefixes everyone else', () => {
		expect(pathFor('ja', '/')).toBe('/ja');
		expect(pathFor('pt-BR', '/docs/faq')).toBe('/pt-BR/docs/faq');
	});

	it('splits a localized path back apart', () => {
		expect(splitLocalePath('/ja/docs/faq')).toEqual({ locale: 'ja', path: '/docs/faq' });
		expect(splitLocalePath('/ja')).toEqual({ locale: 'ja', path: '/' });
		expect(splitLocalePath('/docs/faq')).toEqual({ locale: 'en', path: '/docs/faq' });
	});

	it('does not treat /en/ as a locale prefix — English has one URL', () => {
		expect(splitLocalePath('/en/docs')).toEqual({ locale: 'en', path: '/en/docs' });
	});

	it('does not treat a mis-cased prefix as a locale — that is a redirect', () => {
		expect(splitLocalePath('/PT-BR/docs')).toEqual({ locale: 'en', path: '/PT-BR/docs' });
	});

	it('keeps the reader on the same page when switching language (SC-003)', () => {
		expect(switchTo('zh', '/docs/networks-and-fees')).toBe('/zh/docs/networks-and-fees');
		expect(switchTo('en', '/zh/docs/networks-and-fees')).toBe('/docs/networks-and-fees');
		expect(switchTo('ja', '/zh/about')).toBe('/ja/about');
		expect(switchTo('ja', '/')).toBe('/ja');
	});
});
