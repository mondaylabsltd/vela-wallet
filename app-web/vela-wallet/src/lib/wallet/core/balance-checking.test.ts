/**
 * PR 3 final note F19: a wallet that held nothing last session opens with a
 * cached total of 0, and this shell called that "Live · listening for
 * payments" — its own rule (`total === 0`, not partial, no tokens) is true
 * before anything has been read — then swapped the line for "Can't reach 24
 * networks" when the round came back.
 *
 * "Checking…" and "live" are the core's now (`BalanceView.checking_key`,
 * `live_key`), and the hero draws them and derives neither.
 *
 * The REAL `balance_dashboard` core, answered by hand the way the executor
 * answers it; the drawn line through the real builder and the real corpus.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { BalanceDashboardCore } from '$lib/core/client';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { BalanceShellResult } from '$lib/core/generated/BalanceShellResult';
import { rawResolve, resolveWalletMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { liveBalance } from '$lib/wallet/live';
import { BALANCE_SAID_KEYS } from '$lib/wallet/messages';

const ADDRESS = '0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c';
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
const ON_ITS_WAY = { code: 'USD', rate: 1, committed: false, pending: 'VND' };
const m = resolveWalletMessages('en');

type Out = { view: BalanceView; effects: { id: number; operation: { type: string } }[] };

/**
 * The machine for an account whose cache answers `cachedUsd`, stopped with its
 * FIRST fetch still out. `settle` ends that round and returns the view after.
 */
function firstRead(cachedUsd: number | null): {
	before: BalanceView;
	settle: (result: Omit<Partial<Settled>, 'type'> | 'threw') => BalanceView;
	dispatch: (event: object) => BalanceView;
} {
	const core = new BalanceDashboardCore();
	let out = JSON.parse(
		core.dispatch(JSON.stringify({ type: 'account_changed', address: ADDRESS }))
	) as Out;
	let fetchId: number | null = null;
	// Answer the caches until the fetch is the one thing left unanswered.
	for (let round = 0; round < 8; round += 1) {
		const fetch = out.effects.find((effect) => effect.operation.type === 'fetch_tokens');
		if (fetch) fetchId = fetch.id;
		const next = out.effects.find((effect) => effect.operation.type !== 'fetch_tokens');
		if (!next) break;
		const answer: BalanceShellResult =
			next.operation.type === 'read_balance_cache'
				? { type: 'cached_total_loaded', address: ADDRESS, usd: cachedUsd }
				: next.operation.type === 'read_balance_cache_many'
					? { type: 'cached_balances_loaded', balances: [] }
					: { type: 'balance_cache_written' };
		out = JSON.parse(core.resolve_effect(BigInt(next.id), JSON.stringify(answer))) as Out;
	}
	if (fetchId === null) throw new Error('the core asked for no fetch');
	const before = out.view;
	const id = fetchId;
	return {
		before,
		settle: (result) => {
			const answer: BalanceShellResult =
				result === 'threw'
					? { type: 'fetch_errored', address: ADDRESS, pull: false, internal: false }
					: { ...EVERY_CHAIN_ANSWERED, ...result };
			return (JSON.parse(core.resolve_effect(BigInt(id), JSON.stringify(answer))) as Out).view;
		},
		dispatch: (event) => (JSON.parse(core.dispatch(JSON.stringify(event))) as Out).view
	};
}

type Settled = Extract<BalanceShellResult, { type: 'fetch_settled' }>;

/** A round that settled with both chains answering and nothing held. */
const EVERY_CHAIN_ANSWERED: Settled = {
	type: 'fetch_settled',
	address: ADDRESS,
	pull: false,
	tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	read_chain_ids: [1, 100],
	internal_chain_ids: [],
	registry_chain_ids: [],
	now_ms: 1_000
};

const XDAI = {
	chain_id: 100,
	symbol: 'XDAI',
	name: 'xDAI',
	balance: '10',
	decimals: 18,
	token_address: null,
	price_usd: 1,
	spam: false
};

describe('the hero’s status line before and after the first read (F19)', () => {
	it('a cached zero is "Checking…" until a round says it is live — never "Live" first', () => {
		const wallet = firstRead(0);
		expect(wallet.before.checking_key).toBe('componentsUi.funding.checking');
		expect(wallet.before.live_key).toBeNull();
		const checking = liveBalance(wallet.before, USD, m);
		// The cached zero is drawn — and it is not called live.
		expect(checking).toMatchObject({ state: 'normal', integer: '$0', checkingText: 'Checking…' });
		expect(checking.liveText).toBeUndefined();
		expect(checking.status).toBeUndefined();

		// Every chain answered, holding nothing: now it is live.
		const live = liveBalance(wallet.settle({}), USD, m);
		expect(live).toMatchObject({
			state: 'zero-live',
			integer: '$0',
			liveText: 'Live · listening for payments'
		});
		expect(live.checkingText).toBeUndefined();
		expect(live.status).toBeUndefined();
	});

	it('"Checking…" gives way to "Can’t reach" when the first round misses a chain — "Live" is never said', () => {
		const wallet = firstRead(0);
		expect(liveBalance(wallet.before, USD, m).checkingText).toBe('Checking…');
		const view = wallet.settle({ failed_chain_ids: [1] });
		expect(view.checking_key).toBeNull();
		expect(view.live_key).toBeNull();
		const missed = liveBalance(view, USD, m);
		expect(missed.state).toBe('normal');
		expect(missed.liveText).toBeUndefined();
		expect(missed.checkingText).toBeUndefined();
		expect(missed.status).toEqual({ kind: 'warning', text: "Can't reach Ethereum right now" });
	});

	it('a read that threw over a cached zero is neither "Checking…" nor "Live"', () => {
		const wallet = firstRead(0);
		const view = wallet.settle('threw');
		expect(view.checking_key).toBeNull();
		expect(view.live_key).toBeNull();
		const model = liveBalance(view, USD, m);
		expect(model.state).not.toBe('zero-live');
		expect(model.liveText).toBeUndefined();
		expect(model.checkingText).toBeUndefined();
	});

	it('nothing cached: "Checking…" under the skeleton, from the first frame', () => {
		const wallet = firstRead(null);
		const model = liveBalance(wallet.before, USD, m);
		expect(model).toMatchObject({ state: 'loading', checkingText: 'Checking…' });
		expect(model.integer).toBeUndefined();
		expect(model.status).toBeUndefined();
		// …and while the display currency is still on its way, the same line.
		expect(liveBalance(wallet.before, ON_ITS_WAY, m)).toMatchObject({
			state: 'loading',
			checkingText: 'Checking…'
		});
	});

	it('a cached figure under the first read says "Checking…", not "still updating"', () => {
		const wallet = firstRead(1383.28);
		const model = liveBalance(wallet.before, USD, m);
		expect(model).toMatchObject({ state: 'normal', integer: '$1,383', checkingText: 'Checking…' });
		expect(model.status).toBeUndefined();
		// The round lands with something held: the line is empty again.
		const held = liveBalance(wallet.settle({ tokens: [XDAI] }), USD, m);
		expect(held).toMatchObject({ state: 'normal', integer: '$10' });
		expect(held.checkingText).toBeUndefined();
		expect(held.liveText).toBeUndefined();
	});

	it('a later refresh is not "checking" again: what the last round found stands', () => {
		const wallet = firstRead(0);
		wallet.settle({});
		const again = wallet.dispatch({ type: 'refresh_requested', force: true, pull: true });
		expect(again.checking_key).toBeNull();
		const model = liveBalance(again, USD, m);
		expect(model.checkingText).toBeUndefined();
		expect(model).toMatchObject({ state: 'zero-live', liveText: 'Live · listening for payments' });
	});

	it('"zero, live" is the core’s key and nothing else: this shell’s old rule alone says nothing', () => {
		const wallet = firstRead(0);
		const settled = wallet.settle({});
		// Everything the old rule read — a zero total, not unknown, not partial,
		// no tokens — with the core's word taken away: not live.
		const withoutTheKey = liveBalance({ ...settled, live_key: null }, USD, m);
		expect(withoutTheKey.state).toBe('normal');
		expect(withoutTheKey.liveText).toBeUndefined();
	});

	it('hidden: the mask, and no line', () => {
		const wallet = firstRead(0);
		const model = liveBalance({ ...wallet.before, hidden: true }, USD, m);
		expect(model.state).toBe('hidden');
		expect(model.checkingText).toBeUndefined();
	});
});

describe('the keys the hero’s quiet line can be', () => {
	it('are the ones the core’s source names', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/app/balance_dashboard.rs', 'utf8');
		const named = ['CHECKING', 'LIVE_ZERO'].map(
			(constant) => new RegExp(`pub const ${constant}: &str = "([^"]+)";`).exec(source)?.[1]
		);
		expect(named).toEqual([...BALANCE_SAID_KEYS]);
	});

	it.each(SUPPORTED_LOCALES)('have words in %s', (locale) => {
		const said = resolveWalletMessages(locale).balance.said;
		for (const key of BALANCE_SAID_KEYS) {
			expect(said[key], `${key} in ${locale}`).toBe(rawResolve(locale, key));
			expect(said[key].trim()).not.toBe('');
			expect(said[key]).not.toBe(key);
		}
	});
});
