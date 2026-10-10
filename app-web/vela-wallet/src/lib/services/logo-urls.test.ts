/**
 * Where a logo comes from (2026-09-05): the chain-data endpoint the person
 * configured, at call time — and the coin's own chain for a native coin, so
 * ETH on Arbitrum wears Ethereum's mark, as it does on the phone.
 *
 * That rule is the core's now (`remote_mark`), and its vectors are replayed
 * through the web's Marks module in `flows/marks.test.ts`. What stays here is
 * the one place this layer adds: which rows carry a logo at all.
 */
import { readFileSync } from 'node:fs';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

vi.mock('$app/environment', () => ({ browser: true }));

import { liveChainRows } from '$lib/wallet/live';

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

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
				unreachable_networks: [],
				unreachable_key: null,
				internal_chain_ids: [],
				internal_key: null,
				checking_key: null,
				live_key: null,
				empty_key: null,
				holdings_loading: false,
				cached_total_usd: null,
				switcher: { open: false, loading: false, balances: [], hidden: false }
			},
			'All networks',
			null
		);
		expect(rows[0].logoUrl).toBeUndefined();
		expect(rows[1].logoUrl).toMatch(/\/chainlogos\/eip155-137\.png$/);
	});
});
