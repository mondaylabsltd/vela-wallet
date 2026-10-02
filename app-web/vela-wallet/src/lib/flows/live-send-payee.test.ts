/**
 * Spec 097 F — whom the money goes to, and everything it moved.
 *
 * S2: the Send confirm's To row read "Wallet" — a name from the public wallet
 * registry, where anyone can register any name — with the address only behind
 * a tap on the identicon (`sw-022-golden-gnosis-confirm.png`). The form's line
 * printed the resolver's own label: "Wallet · passkey".
 *
 * S3: a two-coin sweep's success screen read "Send ETH | Sent 0.000418 ETH"
 * although 0.034929 USDC moved in the same operation
 * (`sw-054-golden-base-status-12.png`).
 *
 * The core now decides both (`SendView.payees`, `SendReceiptView.coins`); these
 * pin that the web draws what it decided — the first test through the REAL
 * core, the rest over hand-built views like the other `live-send` suites.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendPayee } from '$lib/core/generated/SendPayee';
import type { SendReceiptView } from '$lib/core/generated/SendReceiptView';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { fill } from '$lib/wallet/messages';
import { shortenAddress, type WalletIdentity } from '$lib/wallet/identity';
import { SendCore } from '../../../../../rust/pkg-web/vela_core.js';
import type { SendEffect } from './core/send-types';
import { buildFlowState } from './fixtures';
import {
	liveSendConfirm,
	liveSendForm,
	liveSendReceipt,
	sendTokenId,
	type SendLiveInputs
} from './live-send';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const identity: WalletIdentity = {
	name: 'Parallel Multi',
	address: '0x88cCA0000000000000000000000000000c266894',
	identiconSvg: '<svg/>'
};
const USD = { code: 'USD', rate: 1, committed: true };
const IDLE_FEE = { options: [], fee: null, busy: false } as unknown as FeeView;

/** The developer wallet of the pass; the public registry calls it "Wallet". */
const DEV = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';

const XDAI: SendToken = {
	network: 'gnosis',
	chain_id: 100,
	symbol: 'xDAI',
	balance: '0.09567',
	decimals: 18,
	token_address: null,
	price_usd: 1,
	logo_urls: [],
	spam: false
};
const ETH: SendToken = { ...XDAI, network: 'base', chain_id: 8453, symbol: 'ETH', price_usd: 2650 };
const USDC: SendToken = {
	...ETH,
	symbol: 'USDC',
	decimals: 6,
	token_address: '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913',
	price_usd: 1
};

function confirmModel() {
	const state = buildFlowState('sd3', m, identicon);
	if (state.base.kind !== 'send-confirm') throw new Error('kind');
	return state.base.model;
}
function formModel() {
	const state = buildFlowState('sd2', m, identicon);
	if (state.base.kind !== 'send-form') throw new Error('kind');
	return state.base.model;
}
function receiptModel() {
	const state = buildFlowState('sd4a', m, identicon);
	if (state.base.kind !== 'send-receipt') throw new Error('kind');
	return state.base.model;
}

function inputs(send: SendView): SendLiveInputs {
	return { send, fee: IDLE_FEE, m, currency: USD, identity, identicon };
}

/**
 * The real send core, answered by hand: every operation it asks for is held
 * until the test answers it by type.
 */
function realSend() {
	const core = new SendCore();
	const pending: SendEffect[] = [];
	const take = (json: string): SendView => {
		const result = JSON.parse(json) as { view: SendView; effects: SendEffect[] };
		pending.push(...result.effects);
		return result.view;
	};
	return {
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
}

const registry = (address: string, name = 'Wallet'): SendPayee => ({
	address,
	name,
	name_source: { type: 'registry' }
});

describe('S2 — the page that signs never names the payee without the address', () => {
	it('the real core: a registry name reaches the form and the confirm beside the address', () => {
		const send = realSend();
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
			tokens: [XDAI],
			chains: [{ chain_id: 100, network: 'gnosis', native_symbol: 'xDAI' }]
		});
		while (send.asks('prewarm_fees')) send.answer('prewarm_fees', { type: 'fees_prewarmed' });
		send.dispatch({ type: 'select_token', token_id: sendTokenId(XDAI) });
		send.answer('load_account_credential', { type: 'account_credential', public_key_hex: '04ab' });
		send.answer('estimate_fee', {
			type: 'fee_estimated',
			outcome: { type: 'failed', kind: 'quote_unavailable' }
		});
		send.dispatch({ type: 'set_recipient', recipient: DEV });
		const view = send.answer('resolve_identity', {
			type: 'identity_resolved',
			identity: { name: 'Wallet', source: 'passkey' }
		});

		expect(view.payees).toEqual([registry(DEV)]);

		const form = liveSendForm(formModel(), inputs(view));
		expect(form.recipient?.note).toBe(`Wallet · ${m['send.velaUser']}`);
		expect(form.recipient?.note).not.toContain('passkey');

		const to = liveSendConfirm(confirmModel(), inputs(view)).facts.find(
			(fact) => fact.label === m['send.toLabel']
		);
		expect(to).toMatchObject({
			value: 'Wallet · Vela User',
			detail: shortenAddress(DEV),
			mono: false,
			lead: { kind: 'identicon', address: DEV }
		});
	});

	const base = (over: Partial<SendView>): SendView => {
		const send = realSend();
		return { ...send.view(), ...over };
	};

	it("the person's own name is drawn untagged — and the address still beside it", () => {
		const view = base({
			stage: 'confirm',
			selected_token: XDAI,
			recipient: DEV,
			payees: [{ address: DEV, name: 'Savings', name_source: { type: 'own' } }]
		});
		const to = liveSendConfirm(confirmModel(), inputs(view)).facts.find(
			(fact) => fact.label === m['send.toLabel']
		);
		expect(to).toMatchObject({ value: 'Savings', detail: shortenAddress(DEV), mono: false });
	});

	it('nobody named: the address alone, in mono, with nothing under it', () => {
		const view = base({
			stage: 'confirm',
			selected_token: XDAI,
			recipient: DEV,
			payees: [{ address: DEV, name: null, name_source: null }],
			// What the resolver said is not the page's to draw: the core
			// dropped a name it could not place.
			recipient_identity: { name: 'Wallet', source: null }
		});
		const to = liveSendConfirm(confirmModel(), inputs(view)).facts.find(
			(fact) => fact.label === m['send.toLabel']
		);
		expect(to).toMatchObject({ value: shortenAddress(DEV), mono: true });
		expect(to?.detail).toBeUndefined();
	});

	it("the sweep's one payee is named the same way (sw-054)", () => {
		const view = base({
			stage: 'confirm',
			multi_select_mode: true,
			multi_chain_id: 8453,
			multi_selected_ids: [sendTokenId(ETH), sendTokenId(USDC)],
			tokens: [ETH, USDC],
			selected_token: ETH,
			recipient: DEV,
			payees: [registry(DEV)]
		});
		const to = liveSendConfirm(confirmModel(), inputs(view)).facts.find(
			(fact) => fact.label === m['send.toLabel']
		);
		expect(to).toMatchObject({ value: 'Wallet · Vela User', detail: shortenAddress(DEV) });
	});

	it("a split's rows are named by the core's payees, each over its address, and no To row", () => {
		const bob = '0x' + 'cd'.repeat(20);
		const view = base({
			stage: 'confirm',
			selected_token: XDAI,
			split_mode: true,
			confirm_amount: '0.3',
			recipient: DEV,
			recipients: [
				{ id: 'a', address: DEV, amount: '0.1', name: 'Payroll Alice' },
				{ id: 'b', address: bob, amount: '0.2', name: null }
			],
			payees: [
				{ address: DEV, name: 'Payroll Alice', name_source: { type: 'own' } },
				{ address: bob, name: null, name_source: null }
			]
		});
		const confirm = liveSendConfirm(confirmModel(), inputs(view));
		expect(confirm.facts.map((fact) => fact.label)).not.toContain(m['send.toLabel']);
		expect(confirm.breakdown?.map((row) => [row.label, row.detail, row.mono])).toEqual([
			['Payroll Alice', shortenAddress(DEV), false],
			[shortenAddress(bob), undefined, true]
		]);
	});
});

describe('S3 — the success screen lists every coin the operation sent', () => {
	const receipt = (over: Partial<SendReceiptView>): SendReceiptView => ({
		status: 'confirmed',
		hold_reason: null,
		kind: null,
		transfers: [],
		coins: [],
		amount: '',
		usd_value: 0,
		submitted_at_ms: null,
		typical_inclusion_s: null,
		...over
	});
	const sweep = (status: SendReceiptView['status']): SendView => ({
		...realSend().view(),
		stage: 'receipt',
		multi_select_mode: true,
		multi_chain_id: 8453,
		selected_token: ETH,
		recipient: DEV,
		recipient_identity: { name: 'Wallet', source: 'passkey' },
		payees: [registry(DEV)],
		tx_status: 'confirmed',
		user_op_hash: '0xop',
		tx_hash: status === 'confirmed' ? '0xtx' : null,
		receipt: receipt({
			status,
			kind: 'multi_select',
			coins: [
				{ amount: '0.000418', symbol: 'ETH', logo_urls: [], token_address: null, usd_value: 1.11 },
				{
					amount: '0.034929',
					symbol: 'USDC',
					logo_urls: [],
					token_address: USDC.token_address,
					usd_value: 0.03
				}
			],
			usd_value: 1.14
		})
	});

	it('a two-coin sweep names both coins, under a header that is not one coin’s', () => {
		const model = liveSendReceipt(receiptModel(), inputs(sweep('confirmed')));
		expect(model.stage).toBe('confirmed');
		expect(model.header.title).toBe(m['send.multiSendTitle']);
		expect(model.title).toBe(m['componentsTx.detail.sent']);
		expect(model.title).not.toContain('0.000418');
		expect(model.breakdownTitle).toBe(fill(m['componentsTx.receipt.assetsCount'], { n: 2 }));
		expect(model.breakdown?.map((row) => [row.label, row.value])).toEqual([
			['ETH', '0.000418 ETH'],
			['USDC', '0.034929 USDC']
		]);
		// Its one recipient is still the caption — the parts are coins, not people.
		expect(model.captions[0]).toBe(`${fill(m['history.toName'], { name: 'Wallet' })} · Base`);
	});

	it('…while it waits on the chain too', () => {
		const model = liveSendReceipt(receiptModel(), inputs(sweep('submitted')));
		expect(model.stage).toBe('submitted');
		expect(model.breakdown?.map((row) => row.label)).toEqual(['ETH', 'USDC']);
	});

	it("a split's headline is its total, read off the core", () => {
		const view: SendView = {
			...realSend().view(),
			stage: 'receipt',
			split_mode: true,
			selected_token: XDAI,
			tx_status: 'confirmed',
			tx_hash: '0xtx',
			receipt: receipt({
				kind: 'split',
				transfers: [
					{ to: DEV, to_name: null, amount: '0.5', symbol: 'xDAI', logo_urls: [], usd_value: 0.5 },
					{
						to: '0x' + 'cd'.repeat(20),
						to_name: null,
						amount: '0.25',
						symbol: 'xDAI',
						logo_urls: [],
						usd_value: 0.25
					}
				],
				coins: [
					{ amount: '0.75', symbol: 'xDAI', logo_urls: [], token_address: null, usd_value: 0.75 }
				],
				amount: '0.75',
				usd_value: 0.75
			})
		};
		const model = liveSendReceipt(receiptModel(), inputs(view));
		expect(model.title).toBe(fill(m['send.txConfirmedTitle'], { amount: '0.75', symbol: 'xDAI' }));
		expect(model.breakdown?.map((row) => row.value)).toEqual(['0.5 xDAI', '0.25 xDAI']);
	});
});
