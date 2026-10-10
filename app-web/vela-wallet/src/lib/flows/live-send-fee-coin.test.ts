/**
 * The send form's fee row names one coin — the core's (`SendView.fee_coin`).
 *
 * With no estimate in hand (a quote out, a quote that failed, a speed being
 * measured) each shell used to pick the row's coin its own way. On BNB Chain
 * with USDT chosen and the quote failed, Android drew an empty disc, the
 * desktop BNB, the web USDT and iOS the fee card's coin: one state, three
 * pictures, and the failed state stays on screen. The rule is the core's now;
 * this pins that the web draws what it says, through the REAL send core, and
 * that the fee card's coin reaches it (`fee_token_changed`).
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import type { WalletIdentity } from '$lib/wallet/identity';
import { SendCore } from '../../../../../rust/pkg-web/vela_core.js';
import type { SendEffect } from './core/send-types';
import { FeeTokenWord } from './core/send-estimates';
import { buildFlowState } from './fixtures';
import { liveSendForm, sendTokenId, type SendLiveInputs } from './live-send';
import { tokenMarkFor } from './marks';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const identity: WalletIdentity = {
	name: 'My Wallet',
	address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
	identiconSvg: '<svg/>'
};
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
const IDLE_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false,
	no_coin_pays: false,
	nothing_to_pay_from: false,
	provisional: false,
	failure: null
};

const BSC_USDT_CONTRACT = '0x55d398326f99059ff775485246999027b3197955';
const BNB: SendToken = {
	network: 'bsc',
	chain_id: 56,
	symbol: 'BNB',
	balance: '1',
	decimals: 18,
	token_address: null,
	price_usd: 600,
	logo_urls: [],
	spam: false
};
const BSC_USDT: SendToken = {
	...BNB,
	symbol: 'USDT',
	balance: '50',
	token_address: BSC_USDT_CONTRACT,
	price_usd: 1
};
const ETH: SendToken = {
	...BNB,
	network: 'ethereum',
	chain_id: 1,
	symbol: 'ETH',
	price_usd: 3000
};

function formModel() {
	const state = buildFlowState('sd2', m, identicon);
	if (state.base.kind !== 'send-form') throw new Error('kind');
	return state.base.model;
}

/** The fee card in the state the bridge mirrors: its coin in force, no estimate. */
function inputs(send: SendView, fee: Partial<FeeView> = {}): SendLiveInputs {
	return { send, fee: { ...IDLE_FEE, ...fee }, m, currency: USD, identity, identicon };
}

/** The real send core, answered by hand. */
function realSend(tokens: SendToken[] = [BSC_USDT, BNB]) {
	const core = new SendCore();
	const pending: SendEffect[] = [];
	const take = (json: string): SendView => {
		const result = JSON.parse(json) as { view: SendView; effects: SendEffect[] };
		pending.push(...result.effects);
		return result.view;
	};
	const send = {
		dispatch: (event: SendEvent) => take(core.dispatch(JSON.stringify(event))),
		answer(type: SendEffect['operation']['type'], result: SendShellResult): SendView {
			const index = pending.findIndex((effect) => effect.operation.type === type);
			if (index < 0) throw new Error(`the core asked for no ${type}: ${JSON.stringify(pending)}`);
			const [effect] = pending.splice(index, 1);
			return take(core.resolve_effect(BigInt(effect.id), JSON.stringify(result)));
		},
		asks: (type: string) => pending.some((effect) => effect.operation.type === type),
		view: () => JSON.parse(core.view()) as SendView
	};
	send.dispatch({
		type: 'open',
		account: { id: 'cred-1', address: identity.address, name: identity.name },
		params: {
			preselected_symbol: null,
			preselected_network: null,
			prefilled_recipient: null,
			prefilled_chain_id: null,
			prefilled_token_address: null,
			prefilled_amount_base: null,
			locked: false,
			preselected_multi: null
		},
		display: { code: 'USD', rate: 1, fiat_decimals: 2 }
	});
	send.answer('fetch_tokens', {
		type: 'tokens_loaded',
		tokens,
		chains: [
			{ chain_id: 56, network: 'bsc', native_symbol: 'BNB' },
			{ chain_id: 1, network: 'ethereum', native_symbol: 'ETH' }
		]
	});
	while (send.asks('prewarm_fees')) send.answer('prewarm_fees', { type: 'fees_prewarmed' });
	return send;
}

/** Pick `token`, and let its warm quote FAIL: the form has no estimate in hand. */
function withAFailedQuote(token: SendToken, send = realSend()) {
	send.dispatch({ type: 'select_token', token_id: sendTokenId(token) });
	send.answer('load_account_credential', { type: 'account_credential', public_key_hex: '04ab' });
	send.answer('estimate_fee', {
		type: 'fee_estimated',
		outcome: { type: 'failed', kind: 'quote_unavailable' }
	});
	return send;
}

const bnbMark = () => tokenMarkFor(56, 'BNB', null);
const usdtMark = () => tokenMarkFor(56, 'USDT', BSC_USDT_CONTRACT);

describe("the fee row draws the core's coin, estimate or none", () => {
	it('the two marks this suite tells apart are different pictures', () => {
		expect(usdtMark()).not.toEqual(bnbMark());
		expect(usdtMark().logoUrls?.length).toBeGreaterThan(0);
	});

	it("a failed quote nobody chose a coin for names the chain's own coin", () => {
		const view = withAFailedQuote(BNB).view();
		expect(view.fee).toBeNull();
		expect(view.fee_coin).toEqual({ symbol: 'BNB', contract: null, chain_id: 56 });
		const row = liveSendForm(formModel(), inputs(view)).fee;
		expect(row.value).toBe('—');
		expect(row.mark).toEqual(bnbMark());
	});

	it('a chosen coin names the row before any estimate of it', () => {
		const send = withAFailedQuote(BNB);
		const view = send.dispatch({ type: 'choose_fee_token', token: BSC_USDT_CONTRACT });
		expect(view.fee).toBeNull();
		// Whatever the shell's own options say — the core's word is drawn.
		const row = liveSendForm(formModel(), inputs(view, { options: [] })).fee;
		expect(row.mark).toEqual(usdtMark());
	});

	it("the fee card's coin in force names the row once the bridge tells the core", () => {
		const send = withAFailedQuote(BNB);
		// Nobody chose: the card picked the coin that can pay, and its quote
		// failed. The bridge mirrors FeeView.fee_token; the row must not say BNB.
		const told = new FeeTokenWord().news(BSC_USDT_CONTRACT, 56, 56);
		expect(told).toEqual({ type: 'fee_token_changed', fee_token: BSC_USDT_CONTRACT });
		const view = send.dispatch(told!);
		const row = liveSendForm(
			formModel(),
			inputs(view, { fee_token: BSC_USDT_CONTRACT, failed: 'quote_unavailable' })
		).fee;
		expect(row.mark).toEqual(usdtMark());
		// The person's pick is newer than the card's word.
		const picked = send.dispatch({ type: 'choose_fee_token', token: null });
		expect(liveSendForm(formModel(), inputs(picked)).fee.mark).toEqual(bnbMark());
	});

	it('with no chain known the row claims no coin — never chain 0', () => {
		const view = realSend().view();
		expect(view.fee_coin).toBeNull();
		const mark = liveSendForm(formModel(), inputs(view)).fee.mark;
		expect(mark.ticker).toBe('');
		expect(mark.logoUrls).toBeUndefined();
		expect(mark.badgeHidden).toBe(true);
	});
});

/**
 * The bridge beside `fee_busy_changed` (iOS `SendStore.feeTokenChanged`): the
 * card's coin reaches the send machine whenever the pair (chain, coin) differs
 * from what this journey was last told, only while the fee session prices the
 * form's own chain — the core files the word against the form's chain at the
 * moment it is said, and drops it while the form has none. A fresh journey has
 * been told nothing.
 */
describe('the fee→send bridge tells the send machine the card’s coin', () => {
	/** The real send core with the page's bridge in front of it. */
	function bridged() {
		const word = new FeeTokenWord();
		const journey = { send: realSend([BSC_USDT, BNB, ETH]) };
		const told: SendEvent[] = [];
		/** The card's coin `token`, with its session pricing `pricing`. */
		const tell = (token: string | null, pricing: number | null) => {
			const view = journey.send.view();
			const form = view.selected_token?.chain_id ?? view.multi_chain_id ?? null;
			const news = word.news(token, pricing, form);
			if (news !== null) {
				told.push(news);
				journey.send.dispatch(news);
			}
			return news;
		};
		/** Leave, and open a new journey: nothing told yet. */
		const reopen = () => {
			word.forget();
			journey.send = realSend([BSC_USDT, BNB, ETH]);
		};
		return { journey, told, tell, reopen };
	}

	it('tells each change once, only about the form’s own chain, and again to a new journey', () => {
		const { journey, told, tell, reopen } = bridged();
		// No chain on the form yet: a word now is about no chain at all.
		expect(tell(BSC_USDT_CONTRACT, 56)).toBeNull();
		withAFailedQuote(BNB, journey.send);
		expect(
			journey.send.view().fee_coin?.contract,
			'nothing was told before the form had a chain'
		).toBeNull();

		// The same coin, now that the form is on the chain it is priced on.
		tell(BSC_USDT_CONTRACT, 56);
		expect(journey.send.view().fee_coin).toEqual({
			symbol: 'USDT',
			contract: BSC_USDT_CONTRACT,
			chain_id: 56
		});
		expect(told).toHaveLength(1);

		// The person picks the chain's own coin: newer than the card's word.
		journey.send.dispatch({ type: 'choose_fee_token', token: null });
		expect(journey.send.view().fee_coin?.contract).toBeNull();
		// The card has not spoken again — the same word is not said twice, or
		// it would undo the person's pick.
		expect(tell(BSC_USDT_CONTRACT, 56), 'an unchanged coin is not told again').toBeNull();
		expect(journey.send.view().fee_coin?.contract).toBeNull();
		expect(tell(null, 56)).toEqual({ type: 'fee_token_changed', fee_token: null });
		// A session still pricing another chain says nothing about this one.
		expect(tell(BSC_USDT_CONTRACT, 1), 'another chain’s coin is not told').toBeNull();
		expect(journey.send.view().fee_coin?.contract).toBeNull();
		tell(BSC_USDT_CONTRACT, 56);
		expect(journey.send.view().fee_coin?.contract, 'a change is told').toBe(BSC_USDT_CONTRACT);
		expect(told).toHaveLength(3);

		// A new journey has been told nothing: the same coin is told again.
		reopen();
		withAFailedQuote(BNB, journey.send);
		tell(BSC_USDT_CONTRACT, 56);
		expect(journey.send.view().fee_coin?.contract, 'a fresh journey hears the card’s coin').toBe(
			BSC_USDT_CONTRACT
		);
		expect(told).toHaveLength(4);
	});

	it('tells the same coin again when the form moves to another chain', () => {
		const { journey, told, tell } = bridged();
		withAFailedQuote(BNB, journey.send);
		tell(null, 56);
		expect(told).toEqual([{ type: 'fee_token_changed', fee_token: null }]);

		// The form moves to Ethereum; the session still prices BNB Chain.
		journey.send.dispatch({ type: 'change_token' });
		withAFailedQuote(ETH, journey.send);
		expect(journey.send.view().selected_token?.chain_id).toBe(1);
		expect(tell(null, 56), 'the network just left says nothing about this one').toBeNull();
		// Priced on Ethereum now: "the chain's own coin" is news about THIS chain.
		expect(tell(null, 1)).toEqual({ type: 'fee_token_changed', fee_token: null });
		expect(tell(null, 1)).toBeNull();
		expect(told).toHaveLength(2);
		expect(journey.send.view().fee_coin).toEqual({ symbol: 'ETH', contract: null, chain_id: 1 });
	});
});
