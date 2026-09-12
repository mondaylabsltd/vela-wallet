/**
 * Where a logo comes from (2026-09-05): the chain-data endpoint the person
 * configured, at call time — and the coin's own chain for a native coin, so
 * ETH on Arbitrum wears Ethereum's mark, as it does on the phone.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));

import { DEFAULT_SERVICE_ENDPOINTS } from './endpoints';
import {
	balanceTokenBadgeChainId,
	balanceTokenLogoURLs,
	chainLogoURL,
	nativeLogoURLs
} from './tokens-model';
import { liveChainRows } from '$lib/wallet/live';

function fakeLocalStorage(seed: Record<string, string> = {}) {
	const map = new Map(Object.entries(seed));
	return {
		get length() {
			return map.size;
		},
		key: (i: number) => [...map.keys()][i] ?? null,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k),
		clear: () => map.clear()
	};
}

afterEach(() => vi.unstubAllGlobals());

describe('logo URLs', () => {
	it('point at the chain-data endpoint, by EIP-155 id', () => {
		vi.stubGlobal('localStorage', fakeLocalStorage());
		expect(chainLogoURL(56)).toBe(
			`${DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL}/chainlogos/eip155-56.png`
		);
	});

	it('follow a re-pointed endpoint at call time, not at import', () => {
		vi.stubGlobal(
			'localStorage',
			fakeLocalStorage({
				'vela.serviceEndpoints': JSON.stringify({ ethereumDataURL: 'https://mirror.example' })
			})
		);
		expect(chainLogoURL(1)).toBe('https://mirror.example/chainlogos/eip155-1.png');
	});

	it("give a native coin its COIN's logo, not its chain's", () => {
		vi.stubGlobal('localStorage', fakeLocalStorage());
		expect(nativeLogoURLs(42161, 'ETH')[0]).toMatch(/eip155-1\.png$/);
		expect(balanceTokenLogoURLs({ chain_id: 8453, symbol: 'ETH', token_address: null })[0]).toMatch(
			/eip155-1\.png$/
		);
		// An unknown native symbol falls back to the chain it sits on.
		expect(nativeLogoURLs(4217, 'pathUSD')[0]).toMatch(/eip155-4217\.png$/);
	});

	it('drop the badge only when it would repeat the token — the coin on its own chain', () => {
		expect(
			balanceTokenBadgeChainId({ chain_id: 1, symbol: 'ETH', token_address: null })
		).toBeNull();
		expect(
			balanceTokenBadgeChainId({ chain_id: 56, symbol: 'BNB', token_address: null })
		).toBeNull();
		// The same coin somewhere else, or any contract token, names its chain.
		expect(balanceTokenBadgeChainId({ chain_id: 42161, symbol: 'ETH', token_address: null })).toBe(
			42161
		);
		expect(
			balanceTokenBadgeChainId({
				chain_id: 1,
				symbol: 'USDT',
				token_address: '0x' + 'a'.repeat(40)
			})
		).toBe(1);
	});

	it('ride on the sidebar rows, except 全部, which is not a chain', () => {
		vi.stubGlobal('localStorage', fakeLocalStorage());
		const rows = liveChainRows(
			{
				address: null,
				display_total_usd: null,
				balance_unknown: false,
				balance_partial: false,
				unreachable: false,
				notice: null,
				hidden: false,
				refreshing: false,
				last_refreshed_at_ms: null,
				tokens: [
					{
						chain_id: 137,
						symbol: 'POL',
						name: 'POL',
						balance: '1',
						decimals: 18,
						token_address: null,
						price_usd: null,
						spam: false
					}
				],
				unpriced_tokens: [],
				failed_chain_ids: [],
				rate_limited_chain_ids: [],
				banner_chain_ids: [],
				holdings_loading: false,
				cached_total_usd: null,
				switcher: { open: false, loading: false, balances: [] }
			},
			'All networks',
			null
		);
		expect(rows[0].logoUrl).toBeUndefined();
		expect(rows[1].logoUrl).toMatch(/\/chainlogos\/eip155-137\.png$/);
	});
});
