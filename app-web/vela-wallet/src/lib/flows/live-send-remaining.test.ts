/**
 * "… left" on a send to several people (device walks of 2026-10-10/11, item
 * 13), through the REAL send core.
 *
 * The iPhone printed 「还剩 0.000300194967818323 BNB」 beside 「余额 0.0003」:
 * `split_remaining` is the exact subtraction and was drawn as it came. The
 * web did not print eighteen places — it put the figure through the balance's
 * formatter — but that one rounds HALF UP, and what is LEFT must never read
 * as more than there is: 1.199999999999999999 left read "1.2 left".
 *
 * The core now says the figure a person reads (`split_remaining_display`:
 * cut to the token ladder, never rounded up, dust kept); the web draws that,
 * with the person's decimal mark, and does no arithmetic of its own.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it } from 'vitest';
import { SendCore } from '$lib/core/client';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendOperation } from '$lib/core/generated/SendOperation';
import type { SendRecipientDraft } from '$lib/core/generated/SendRecipientDraft';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { preferences } from '$lib/services/preferences.svelte';
import { tokenAmountText } from '$lib/wallet/live';
import { buildFlowState } from './fixtures';
import { liveSendForm, sendTokenId } from './live-send';

const m = resolveWalletFlowMessages('en');
const zh = resolveWalletFlowMessages('zh');
const ACCOUNT = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const A = '0x' + 'ab'.repeat(20);
const B = '0x' + 'cd'.repeat(20);
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD = { code: 'USD', rate: 1, committed: true, pending: null };

type Out = { view: SendView; effects: { id: number; operation: SendOperation }[] };

const coin = (symbol: string, chain_id: number, network: string, balance: string): SendToken => ({
	network,
	chain_id,
	symbol,
	balance,
	decimals: 18,
	token_address: null,
	price_usd: null,
	logo_urls: [],
	spam: false
});

/** A send of `token`, opened and switched to several recipients. */
function splitOf(token: SendToken) {
	const core = new SendCore();
	const pending: Out['effects'] = [];
	let view!: SendView;
	const take = (out: Out) => {
		view = out.view;
		pending.push(...out.effects);
	};
	const dispatch = (event: SendEvent) =>
		take(JSON.parse(core.dispatch(JSON.stringify(event))) as Out);
	const resolve = (type: SendOperation['type'], result: SendShellResult) => {
		const at = pending.findIndex((effect) => effect.operation.type === type);
		if (at < 0) throw new Error(`the core asked for no ${type}`);
		const [effect] = pending.splice(at, 1);
		take(JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(result))) as Out);
	};
	dispatch({
		type: 'open',
		account: { id: 'cred-1', address: ACCOUNT, name: 'Wallet' },
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
	resolve('fetch_tokens', {
		type: 'tokens_loaded',
		tokens: [token],
		chains: [{ chain_id: token.chain_id, network: token.network, native_symbol: token.symbol }]
	});
	dispatch({ type: 'select_token', token_id: sendTokenId(token) });
	dispatch({ type: 'enter_split_mode' });
	return {
		view: () => view,
		rows: (...amounts: string[]) =>
			dispatch({
				type: 'recipients_changed',
				recipients: amounts.map((amount, i): SendRecipientDraft => ({
					id: `rcpt_${i + 1}`,
					address: i % 2 === 0 ? A : B,
					amount,
					name: null
				}))
			}),
		free: () => core.free()
	};
}

const IDLE_FEE = {} as FeeView;
const formModel = () => {
	const state = buildFlowState('sd2', m, identicon);
	if (state.base.kind !== 'send-form') throw new Error('sd2 is the send form');
	return state.base.model;
};
/** The line the split form draws under its total. */
const leftLine = (view: SendView, messages = m) =>
	liveSendForm(formModel(), {
		send: view,
		fee: IDLE_FEE,
		m: messages,
		currency: USD,
		identity: { address: ACCOUNT, name: 'Wallet' } as never,
		identicon
	}).summary?.remaining;

afterEach(() => {
	preferences.numberFormat = 'auto';
});

describe('what is left to give out, on a send to several people', () => {
	it('the walk’s own balance reads "0.0003 … left", beside "Balance 0.0003"', () => {
		const BNB = coin('BNB', 56, 'bsc', '0.000300194967818323');
		const send = splitOf(BNB);
		send.rows('0', '0');
		// The exact figure is still the core's to count with…
		expect(send.view().split_remaining).toBe('0.000300194967818323');
		// …and the one drawn is the one cut for reading.
		expect(send.view().split_remaining_display).toBe('0.0003');
		expect(leftLine(send.view())).toBe('0.0003 BNB left');
		expect(leftLine(send.view(), zh)).toBe('还剩 0.0003 BNB');
		// The same digits as the balance the form shows beside it.
		expect(tokenAmountText(BNB.balance)).toBe('0.0003');
		send.free();
	});

	it('never says more is left than is: a figure a hair under is cut, not rounded up', () => {
		const send = splitOf(coin('ETH', 1, 'ethereum', '2'));
		send.rows('0.5', '0.300000000000000001');
		expect(send.view().split_remaining).toBe('1.199999999999999999');
		expect(leftLine(send.view())).toBe('1.1999 ETH left');
		// What the balance's own formatter makes of the exact figure — the
		// rounding this line must not do.
		expect(tokenAmountText(send.view().split_remaining ?? '')).toBe('1.2');
		send.free();
	});

	it('keeps dust: a remainder too small for six places is not "0 left"', () => {
		const send = splitOf(coin('ETH', 1, 'ethereum', '1.0000000004'));
		send.rows('1', '0');
		expect(send.view().split_remaining).toBe('0.0000000004');
		expect(leftLine(send.view())).toBe('0.0000000004 ETH left');
		send.free();
	});

	it('is written with the person’s decimal mark, like the balance beside it', () => {
		preferences.numberFormat = 'dot_comma';
		const send = splitOf(coin('BNB', 56, 'bsc', '0.000300194967818323'));
		send.rows('0', '0');
		expect(leftLine(send.view())).toBe('0,0003 BNB left');
		expect(tokenAmountText('0.000300194967818323')).toBe('0,0003');
		send.free();
	});

	it('says nothing when there is nothing to read: over the balance, or a row that cannot be summed', () => {
		const send = splitOf(coin('ETH', 1, 'ethereum', '2'));
		send.rows('1.5', '0.6');
		expect(send.view().split_over_balance).toBe(true);
		expect(leftLine(send.view())).toBeUndefined();
		send.rows('1,5', '0.1');
		expect(leftLine(send.view())).toBeUndefined();
		send.free();
	});
});

describe('the importer, adding to people already on the form', () => {
	it('is handed the figure for reading, not the exact one', () => {
		// The importer's own line is `live-batch.test.ts`'s; what a node test
		// can hold of the page is which of the two figures it passes on.
		const page = readFileSync('src/routes/[locale]/wallet/+page.svelte', 'utf8');
		expect(page).toContain('remaining: sendView.split_remaining_display');
		expect(page).not.toMatch(/remaining: sendView\.split_remaining[,\s]/);
	});
});
