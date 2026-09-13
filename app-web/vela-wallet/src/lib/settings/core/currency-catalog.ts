/**
 * The display currencies a person can choose (spec 028 Phase 9, T491).
 *
 * The settings sheet listed eight drawn currencies since 023, whatever the
 * rate sources could actually price. The list is now the union of what the
 * two sources cover — the Chainlink fiat feeds (`fiat-rates`) and the
 * configured endpoint (`fiat-fx`, the self-hosted Frankfurter by default,
 * replaceable in 服务端点) — with the drawn eight as the floor while nothing
 * has answered yet, so the sheet is never empty on a cold start.
 *
 * Names come from the browser's own currency display names, in the page's
 * locale: a wallet that ships fifteen locales does not want fifteen copies of
 * "US Dollar" in its corpus when every browser already carries them.
 */
import { FIAT_FEED_CODES } from '$lib/services/fiat-rates';
import { getSupportedFxCodes } from '$lib/services/fiat-fx';

/** The drawn eight (settings/fixtures.ts) — the floor, never the ceiling. */
export const DEFAULT_CURRENCY_CODES: readonly string[] = [
	'USD',
	'EUR',
	'GBP',
	'CNY',
	'JPY',
	'KRW',
	'HKD',
	'VND'
];

/** USD first, then everything either source prices, alphabetically. */
export async function loadCurrencyCodes(): Promise<string[]> {
	const fx = await getSupportedFxCodes().catch((): string[] => []);
	const codes = new Set<string>();
	for (const code of [...FIAT_FEED_CODES, ...fx]) {
		const upper = code.toUpperCase();
		if (/^[A-Z]{3}$/.test(upper)) codes.add(upper);
	}
	if (codes.size === 0) for (const code of DEFAULT_CURRENCY_CODES) codes.add(code);
	codes.delete('USD');
	return ['USD', ...[...codes].sort()];
}

/** The browser's name for a currency in this locale, or nothing worth showing. */
export function currencyDisplayName(code: string, locale: string): string | undefined {
	try {
		const name = new Intl.DisplayNames([locale], { type: 'currency' }).of(code);
		return name === undefined || name === code ? undefined : name;
	} catch {
		return undefined;
	}
}
