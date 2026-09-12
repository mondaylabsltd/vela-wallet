/**
 * The sweep, read off the core (spec 028 T444 — US3).
 *
 * Every fact these assert is a projection of `SendView`'s `multi_*` fields:
 * the tick per row is `multi_selected_ids`, the greying is `multi_chain_id`,
 * the per-token amounts are `multi_specs` (net of the gas reserve — the EXACT
 * figures a submit moves, invariant ⑪), and the CTA counts what the confirm
 * will sweep. The shell narrows nothing and re-decides nothing; if it did, one
 * of these would have to compute the thing it asserts, and that is the tell.
 */
import { describe, expect, it } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import type { WalletIdentity } from '$lib/wallet/identity';
import { buildFlowState } from './fixtures';
import {
	liveSendConfirm,
	liveSendForm,
	liveSendPick,
	sendTokenId,
	type SendLiveInputs
} from './live-send';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const identity: WalletIdentity = {
	name: 'My Wallet',
	address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
	identiconSvg: '<svg/>'
};
const USD = { code: 'USD', rate: 1, committed: true };
const IDLE_FEE: FeeView = {
	options: [],
	fee: null,
	busy: false,
	failure: null
} as unknown as FeeView;

const USDC: SendToken = {
	network: 'eth-mainnet',
	chain_id: 1,
	symbol: 'USDC',
	balance: '100',
	decimals: 6,
	token_address: '0x' + 'cc'.repeat(20),
	price_usd: 1,
	logo_urls: [],
	spam: false
};
const ETH: SendToken = {
	...USDC,
	symbol: 'ETH',
	token_address: null,
	balance: '1.5',
	decimals: 18,
	price_usd: 3000
};
const MATIC: SendToken = {
	...USDC,
	network: 'polygon-mainnet',
	chain_id: 137,
	symbol: 'MATIC',
	token_address: null,
	balance: '20',
	decimals: 18,
	price_usd: 0.5
};

const BASE: SendView = {
	stage: 'select_token',
	loading: false,
	locked: false,
	amount_locked: false,
	lock_error: null,
	resolving_lock: false,
	adding_network: false,
	add_network_msg: null,
	tokens: [ETH, USDC, MATIC],
	selected_token: null,
	recipient: '',
	amount: '',
	amount_fiat_code: null,
	denom_toggle_shown: false,
	denom_toggle_enabled: false,
	denom_toggle_reason: null,
	confirm_amount_issue: null,
	token_amount: '',
	confirm_amount: '',
	split_mode: false,
	recipients: [],
	split_over_balance: false,
	picker_target: null,
	multi_select_mode: false,
	multi_selected_ids: [],
	multi_valuable_ids: [],
	multi_chain_id: null,
	multi_specs: [],
	show_scanner: false,
	show_contact_picker: false,
	show_batch_import: false,
	estimating_gas: false,
	fee_busy: false,
	fee: null,
	gas_fee_token: null,
	amount_warning: null,
	same_asset_fee_issue: null,
	can_continue: false,
	can_confirm: false,
	sending: false,
	tx_status: 'idle',
	tx_error: null,
	tx_hash: null,
	user_op_hash: null,
	receipt: null,
	treasury_bootstrap: null,
	recipient_identity: null,
	recipient_risk: null,
	sim_json: null
};

function inputs(send: Partial<SendView>, sweepPicking = false): SendLiveInputs {
	return {
		send: { ...BASE, ...send },
		fee: IDLE_FEE,
		m,
		currency: USD,
		identity,
		identicon,
		sweepPicking
	};
}

function pick(state: 'sd1' | 'sd1b') {
	const model = buildFlowState(state, m, identicon).base;
	if (model.kind !== 'send-pick') throw new Error('not a picker');
	return model.model;
}

describe('the token id the shell echoes', () => {
	it("is the core's `SendToken::id()`, byte for byte", () => {
		// `{network}_{token_address|native}_{symbol}` — a drifted id would select
		// nothing and say nothing, which is the failure this pins.
		expect(sendTokenId(USDC)).toBe(`eth-mainnet_${USDC.token_address}_USDC`);
		expect(sendTokenId(ETH)).toBe('eth-mainnet_native_ETH');
	});
});

describe('SD1b — the picker in sweep mode', () => {
	it('shows no selection at all until the person opens the sweep', () => {
		const model = liveSendPick(pick('sd1'), inputs({}, false));
		expect(model.selection).toBeUndefined();
		expect(model.notice).toBeUndefined();
		expect(model.cta).toEqual({ label: m['send.multiSendTitle'], accent: false });
	});

	it("ticks exactly the core's `multi_selected_ids`, in row order", () => {
		const model = liveSendPick(
			pick('sd1b'),
			inputs({ multi_chain_id: 1, multi_selected_ids: [sendTokenId(USDC)] }, true)
		);
		expect(model.selection?.selected).toEqual([false, true, false]);
	});

	it('greys the rows on another chain once one is pinned, and none before', () => {
		const pinned = liveSendPick(pick('sd1b'), inputs({ multi_chain_id: 1 }, true));
		expect(pinned.selection?.dimmed).toEqual([false, false, true]);
		expect(pinned.notice?.text).toBe(
			m['send.multiSendChainNotice'].replace('{{network}}', 'Ethereum')
		);

		const unpinned = liveSendPick(pick('sd1b'), inputs({ multi_chain_id: null }, true));
		expect(unpinned.selection?.dimmed).toEqual([false, false, false]);
		expect(unpinned.notice).toBeUndefined();
	});

	it('counts the CTA from the same ids the confirm will sweep', () => {
		const model = liveSendPick(
			pick('sd1b'),
			inputs({ multi_chain_id: 1, multi_selected_ids: [sendTokenId(ETH), sendTokenId(USDC)] }, true)
		);
		expect(model.cta.accent).toBe(true);
		expect(model.cta.label).toBe(
			m['send.multiSendContinue'].replace('{{n}}', '2').replace('{{chain}}', 'Ethereum')
		);
	});

	it('keeps the CTA quiet while nothing is ticked', () => {
		const model = liveSendPick(pick('sd1b'), inputs({ multi_chain_id: 1 }, true));
		expect(model.cta.accent).toBe(false);
	});
});

describe('SD2d — the sweep form', () => {
	const sweeping: Partial<SendView> = {
		stage: 'enter_details',
		multi_select_mode: true,
		multi_chain_id: 1,
		multi_selected_ids: [sendTokenId(ETH), sendTokenId(USDC)],
		selected_token: ETH,
		recipient: '0x' + 'ab'.repeat(20)
	};

	function form() {
		const model = buildFlowState('sd2d', m, identicon).base;
		if (model.kind !== 'send-form') throw new Error('not a form');
		return model.model;
	}

	it("lists the core's picks with the reserved amounts, not the balances", () => {
		const model = liveSendForm(
			form(),
			inputs({
				...sweeping,
				// ETH net of its gas reserve; USDC untouched. HUMAN decimal strings,
				// as the core hands them — the batch builder is what makes base units.
				multi_specs: [
					{ token_address: null, decimals: 18, amount: '1.49' },
					{ token_address: USDC.token_address, decimals: 6, amount: '100' }
				]
			})
		);
		expect(model.mode).toBe('sweep');
		expect(model.token).toBeUndefined();
		expect(model.sweepRows?.map((row) => [row.symbol, row.amount])).toEqual([
			['ETH', '1.49'],
			['USDC', '100']
		]);
		expect(model.sweepSummary).toBe(
			m['send.multiSendSummary'].replace('{{n}}', '2').replace('{{chain}}', 'Ethereum')
		);
	});

	it('falls back to the full balance until the core has reserved', () => {
		const model = liveSendForm(form(), inputs({ ...sweeping, multi_specs: [] }));
		expect(model.sweepRows?.map((row) => row.amount)).toEqual(['1.5', '100']);
	});

	it('has one recipient, says so, and no amount field', () => {
		const model = liveSendForm(form(), inputs(sweeping));
		expect(model.amount).toBeUndefined();
		expect(model.addRecipient).toBeUndefined();
		expect(model.recipients).toBeUndefined();
		expect(model.recipient?.note).toBe(m['send.multiSendSameRecipient']);
		expect(model.recipient?.scanLabel).toBe(m['send.scanAria']);
	});
});

describe('SD3c — the sweep confirm', () => {
	it('prints N assets, the priced total, and one row per reserved spec', () => {
		const model = buildFlowState('sd3c', m, identicon).base;
		if (model.kind !== 'send-confirm') throw new Error('not a confirm');
		const confirm = liveSendConfirm(
			model.model,
			inputs({
				stage: 'confirm',
				multi_select_mode: true,
				multi_chain_id: 1,
				multi_selected_ids: [sendTokenId(ETH), sendTokenId(USDC)],
				selected_token: ETH,
				recipient: '0x' + 'ab'.repeat(20),
				multi_specs: [
					{ token_address: null, decimals: 18, amount: '1' },
					{ token_address: USDC.token_address, decimals: 6, amount: '100' }
				]
			})
		);
		expect(confirm.amount).toBe(m['componentsTx.receipt.assetsCount'].replace('{{n}}', '2'));
		// 1 ETH at $3,000 + 100 USDC at $1 — the total of the SPECS, not of the
		// balances (1.5 ETH would have been $4,600).
		expect(confirm.subline).toContain('$3,100.00');
		expect(confirm.breakdown?.map((row) => row.value)).toEqual([
			'1 ETH · ≈$3,000.00',
			'100 USDC · ≈$100.00'
		]);
	});
});
