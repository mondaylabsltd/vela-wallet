import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { BUILT_IN_NETWORKS } from './networks';
import { en } from './i18n/messages/en';
import { SUPPORTED_LOCALES, DEFAULT_LOCALE } from './i18n/locales';

/**
 * The landing page's network list must be the wallet's (spec 080, SC-002).
 * Read straight from vela-core so the two cannot disagree silently again.
 */
const CORE = join(process.cwd(), '../../rust/crates/vela-core/src/app/network_admin.rs');

function coreChainIds(): number[] {
	const text = readFileSync(CORE, 'utf8');
	const start = text.indexOf('BUILTIN_CHAINS');
	const end = text.indexOf('\n];', start);
	return [...text.slice(start, end).matchAll(/chain_id:\s*(\d+)/g)].map((m) => Number(m[1]));
}

describe('built-in networks', () => {
	it('match vela-core BUILTIN_CHAINS, in order', () => {
		expect(BUILT_IN_NETWORKS.map((n) => n.chainId)).toEqual(coreChainIds());
	});

	it('are counted correctly in the English heading', () => {
		const count = String(BUILT_IN_NETWORKS.length);
		expect(en.home.networks.heading).toContain(count);
	});

	for (const locale of SUPPORTED_LOCALES) {
		if (locale === DEFAULT_LOCALE) continue;
		it(`are counted correctly in the ${locale} heading`, () => {
			const file = join(process.cwd(), `src/lib/i18n/messages/${locale}.json`);
			const heading = JSON.parse(readFileSync(file, 'utf8'))?.home?.networks?.heading;
			// An untranslated page falls back to English, which is checked above.
			if (heading === undefined) return expect(heading).toBeUndefined();
			expect(heading).toContain(String(BUILT_IN_NETWORKS.length));
		});
	}
});
