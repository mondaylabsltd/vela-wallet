/**
 * The assets flow overlay (spec 025 Phase 3): the pushed screen shows the
 * core's holdings, never the fixture's; empty only once the core has looked.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import { resolveWalletFlowMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildFlowState } from './fixtures';
import { withLiveFlow } from './live';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD = { code: 'USD', rate: 1, committed: true };

const LOOKED: BalanceView = {
	address: '0xabc',
	display_total_usd: 4500,
	balance_unknown: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: 1,
	tokens: [
		{
			chain_id: 1,
			symbol: 'ETH',
			name: 'Ether',
			balance: '1.5',
			decimals: 18,
			token_address: null,
			price_usd: 3000,
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
};

describe('withLiveFlow (assets)', () => {
	const t1 = buildFlowState('t1', fm, identicon);
	const t4 = buildFlowState('t4', fm, identicon);
	const emptyCopy = t4.base.kind === 'assets' ? t4.base.model.empty : undefined;

	it("swaps the fixture rows for the core's holdings", () => {
		const live = withLiveFlow(t1, { balance: LOOKED, currency: USD, m, emptyCopy });
		expect(live.base.kind).toBe('assets');
		if (live.base.kind !== 'assets') return;
		expect(live.base.model.rows.map((r) => r.ticker)).toEqual(['ETH']);
		expect(live.base.model.empty).toBeUndefined();
		expect(JSON.stringify(live)).not.toContain('0.8533');
	});

	it('shows the empty body only after the core has looked', () => {
		const looked = withLiveFlow(t1, {
			balance: { ...LOOKED, tokens: [], display_total_usd: 0 },
			currency: USD,
			m,
			emptyCopy
		});
		if (looked.base.kind !== 'assets') throw new Error('kind');
		expect(looked.base.model.empty).toEqual(emptyCopy);

		const unknown = withLiveFlow(t1, {
			balance: { ...LOOKED, tokens: [], display_total_usd: null, balance_unknown: true },
			currency: USD,
			m,
			emptyCopy
		});
		if (unknown.base.kind !== 'assets') throw new Error('kind');
		expect(unknown.base.model.empty).toBeUndefined();
		expect(unknown.base.model.rows).toEqual([]);
	});

	it('leaves screens without a live overlay alone (send is 026)', () => {
		const sd1 = buildFlowState('sd1', fm, identicon);
		expect(withLiveFlow(sd1, { balance: LOOKED, currency: USD, m, emptyCopy })).toBe(sd1);
	});
});
