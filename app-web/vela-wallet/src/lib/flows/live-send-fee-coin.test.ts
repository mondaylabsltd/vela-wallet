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
import { feeTokenNews } from './core/send-estimates';
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
const USD = { code: 'USD', rate: 1, committed: true };
const IDLE_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false,
	no_coin_pays: false
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
function realSend() {
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
		tokens: [BSC_USDT, BNB],
		chains: [{ chain_id: 56, network: 'bsc', native_symbol: 'BNB' }]
	});
	while (send.asks('prewarm_fees')) send.answer('prewarm_fees', { type: 'fees_prewarmed' });
	return send;
}

/** Pick `token`, and let its warm quote FAIL: the form has no estimate in hand. */
function withAFailedQuote(token: SendToken) {
	const send = realSend();
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
		const told = feeTokenNews(undefined, BSC_USDT_CONTRACT);
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

describe('the fee→send bridge tells the send machine the card’s coin', () => {
	it('tells a fresh session whatever the card holds, the chain’s own coin included', () => {
		expect(feeTokenNews(undefined, null)).toEqual({ type: 'fee_token_changed', fee_token: null });
		expect(feeTokenNews(undefined, BSC_USDT_CONTRACT)).toEqual({
			type: 'fee_token_changed',
			fee_token: BSC_USDT_CONTRACT
		});
	});

	it('tells a change, and nothing it has heard already', () => {
		expect(feeTokenNews(null, null)).toBeNull();
		expect(feeTokenNews(BSC_USDT_CONTRACT, BSC_USDT_CONTRACT)).toBeNull();
		expect(feeTokenNews(BSC_USDT_CONTRACT, null)).toEqual({
			type: 'fee_token_changed',
			fee_token: null
		});
		expect(feeTokenNews(null, BSC_USDT_CONTRACT)).toEqual({
			type: 'fee_token_changed',
			fee_token: BSC_USDT_CONTRACT
		});
	});
});
