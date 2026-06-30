/**
 * 汇率 API — 返回 USD → 目标货币的汇率。
 *
 * 使用 frankfurter.app（免费、无 key、ECB 数据源）。
 * 服务端缓存 1 小时。
 */
import type { RequestHandler } from './$types';
import { fetchWithTimeout, UPSTREAM_TIMEOUTS } from '$lib/server/net';

interface RateCache {
	rates: Record<string, number>;
	fetchedAt: number;
}

let cache: RateCache | null = null;
const CACHE_TTL = 3600_000; // 1 hour

const SUPPORTED_CURRENCIES = [
	'USD', 'CNY', 'EUR', 'GBP', 'JPY', 'KRW', 'HKD', 'SGD',
	'AUD', 'CAD', 'INR', 'BRL', 'MXN', 'TRY', 'PHP', 'IDR'
];

async function fetchRates(): Promise<Record<string, number>> {
	if (cache && Date.now() - cache.fetchedAt < CACHE_TTL) {
		return cache.rates;
	}

	const targets = SUPPORTED_CURRENCIES.filter(c => c !== 'USD').join(',');
	const res = await fetchWithTimeout(
		`https://api.frankfurter.app/latest?from=USD&to=${targets}`,
		{},
		UPSTREAM_TIMEOUTS.exchangeRate
	);

	if (!res.ok) throw new Error(`Frankfurter API error (${res.status})`);

	const data = await res.json();
	const rates: Record<string, number> = { USD: 1, ...data.rates };

	cache = { rates, fetchedAt: Date.now() };
	return rates;
}

export const GET: RequestHandler = async ({ url }) => {
	const currency = url.searchParams.get('currency')?.toUpperCase() ?? 'USD';

	if (currency === 'USD') {
		return new Response(JSON.stringify({ currency: 'USD', rate: 1 }), {
			headers: { 'Content-Type': 'application/json', 'Cache-Control': 'public, max-age=3600' }
		});
	}

	try {
		const rates = await fetchRates();
		const rate = rates[currency];

		if (rate == null) {
			return new Response(JSON.stringify({ error: `Unsupported currency: ${currency}` }), {
				status: 400, headers: { 'Content-Type': 'application/json' }
			});
		}

		return new Response(JSON.stringify({ currency, rate }), {
			headers: { 'Content-Type': 'application/json', 'Cache-Control': 'public, max-age=3600' }
		});
	} catch {
		// Upstream slow/down: serve the last-known rate rather than failing, but
		// label it stale (with its age) so the client never treats it as fresh.
		const staleRate = cache?.rates[currency];
		if (staleRate != null) {
			return new Response(
				JSON.stringify({ currency, rate: staleRate, stale: true, fetchedAt: cache!.fetchedAt }),
				{ headers: { 'Content-Type': 'application/json', 'X-Stale': 'true', 'Cache-Control': 'no-store' } }
			);
		}
		return new Response(JSON.stringify({ error: 'Failed to fetch exchange rates' }), {
			status: 502, headers: { 'Content-Type': 'application/json' }
		});
	}
};
