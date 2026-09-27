/**
 * The receive and token screens, about what was actually chosen (spec 028
 * Phase 9, T481–T484). Every assertion is "the model said which network /
 * which token", never a fixture's guess.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import { resolveWalletFlowMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { getAllNetworksSync } from '$lib/services/networks';
import { buildDesktopFlowState, buildFlowState } from './fixtures';
import { receiveNetworks, withLiveDesktopFlow, withLiveFlow, type FlowsLiveInputs } from './live';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const USD = { code: 'USD', rate: 1, committed: true };
const identity = {
	name: 'My Wallet',
	address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
	identiconSvg: '<svg/>'
};
const USDC_BNB: BalanceToken = {
	chain_id: 56,
	symbol: 'USDC',
	name: 'USD Coin',
	balance: '3.847256',
	decimals: 18,
	token_address: '0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d',
	price_usd: 1,
	spam: false
};
const BALANCE: BalanceView = {
	address: identity.address,
	display_total_usd: 3.85,
	balance_unknown: false,
	balance_partial: false,
	unreachable: false,
	notice: null,
	hidden: false,
	refreshing: false,
	last_refreshed_at_ms: null,
	tokens: [USDC_BNB],
	unpriced_tokens: [],
	failed_chain_ids: [],
	rate_limited_chain_ids: [],
	banner_chain_ids: [],
	holdings_loading: false,
	cached_total_usd: null,
	switcher: { open: false, loading: false, balances: [] }
};
const inputs: FlowsLiveInputs = {
	balance: BALANCE,
	currency: USD,
	m,
	emptyCopy: undefined,
	feed: null,
	identity,
	fm
};

describe('the receive list (T481)', () => {
	it('counts the networks it actually lists, and every row names its chain', () => {
		const live = withLiveFlow(buildFlowState('r1', fm, identicon), inputs);
		if (live.base.kind !== 'receive-list') throw new Error('r1 is the list');
		const rows = live.base.model.rows;
		expect(rows.length).toBe(getAllNetworksSync().length);
		expect(live.base.model.subtitle).toContain(String(rows.length));
		expect(rows.map((r) => r.chainId)).toEqual(receiveNetworks().map((n) => n.chainId));
		expect(rows[0]!.addressFull).toBe(identity.address);
	});
});

describe('the code screen (T482)', () => {
	it('is about the tapped network: its name in the title, its mark in the centre, its explorer', () => {
		const live = withLiveFlow(buildFlowState('r2', fm, identicon), {
			...inputs,
			receiveChainId: 56
		});
		if (live.sheet?.kind !== 'receive-qr') throw new Error('r2 shows the code');
		const qr = live.sheet.model;
		expect(qr.title).toContain('BNB Chain');
		expect(qr.centre.ticker).toBe('BNB');
		expect(qr.centre.badgeHidden).toBe(true);
		expect(qr.explorerUrl).toContain('bscscan.com/address/' + identity.address);
		expect(qr.account.lines.join('')).toBe(identity.address);
	});

	it('the asset variant is about the held token: symbol, chain, contract, its own mark', () => {
		const live = withLiveFlow(buildFlowState('r3', fm, identicon), {
			...inputs,
			selectedToken: USDC_BNB
		});
		if (live.sheet?.kind !== 'receive-qr') throw new Error('r3 shows the code');
		const qr = live.sheet.model;
		expect(qr.title).toContain('USDC');
		expect(qr.title).toContain('BNB Chain');
		expect(qr.contract?.copyValue).toBe(USDC_BNB.token_address);
		expect(qr.centre.ticker).toBe('USDC');
		expect(qr.centre.badgeHidden).toBe(false);
	});
});

describe('the phone token screen (T483)', () => {
	it('is the held token, with its whole contract to copy and its explorer page', () => {
		const live = withLiveFlow(buildFlowState('t2', fm, identicon), {
			...inputs,
			selectedToken: USDC_BNB
		});
		if (live.sheet?.kind !== 'token-detail') throw new Error('t2 is the token sheet');
		const t = live.sheet.model;
		expect(t.symbol).toBe('USDC');
		expect(t.chain).toBe('BNB Chain');
		// The asset list's ladder (spec 078): 4 places from 1, half up — the
		// same digits the home row and Send's token card show for it.
		expect(t.balance).toBe('3.8473 USDC');
		const contract = t.facts.find((f) => f.copyValue !== undefined);
		expect(contract?.copyValue).toBe(USDC_BNB.token_address);
		expect(contract?.value).not.toBe(USDC_BNB.token_address);
		expect(t.explorerUrl).toContain('/token/' + USDC_BNB.token_address);
		expect(t.explorerUrl).toContain('?a=' + identity.address);
	});

	it('stays the drawn picture without a token to be about', () => {
		const drawn = buildFlowState('t2', fm, identicon);
		const live = withLiveFlow(drawn, inputs);
		expect(live.sheet).toEqual(drawn.sheet);
	});
});

describe('the desktop column title (T484)', () => {
	it('follows the live body: a live send form is titled for its token', () => {
		const drawn = buildDesktopFlowState('dsd2', fm, identicon);
		const send: NonNullable<FlowsLiveInputs['send']> = {
			send: {
				stage: 'enter_details',
				loading: false,
				locked: false,
				amount_locked: false,
				lock_error: null,
				resolving_lock: false,
				adding_network: false,
				add_network_msg: null,
				tokens: [],
				selected_token: {
					network: 'arbitrum',
					chain_id: 42161,
					symbol: 'ETH',
					balance: '0.225852',
					decimals: 18,
					token_address: null,
					price_usd: 2450,
					logo_urls: [],
					spam: false
				},
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
				multi_all_valuable_ids: [],
				multi_chain_id: null,
				multi_specs: [],
				recipient_identity: null,
				recipient_risk: null,
				recipient_is_contract: false,
				amount_warning: null,
				fee: null,
				fee_issue: null,
				estimating_gas: false,
				can_continue: false,
				can_confirm: false,
				tx: 'idle',
				tx_error: null,
				receipt: null,
				show_scanner: false,
				show_contact_picker: false,
				show_batch_import: false,
				treasury_bootstrap: null,
				is_signing: false,
				multi_chain_id_locked: false
			} as unknown as NonNullable<FlowsLiveInputs['send']>['send'],
			fee: { options: [], fee: null } as unknown as NonNullable<FlowsLiveInputs['send']>['fee'],
			m: fm,
			currency: USD,
			identity,
			identicon,
			sweepPicking: false
		};
		const live = withLiveDesktopFlow(drawn, { ...inputs, send });
		expect(drawn.title).toContain('USDT');
		expect(live.title).toContain('ETH');
		expect(live.title).not.toContain('USDT');
	});
});
