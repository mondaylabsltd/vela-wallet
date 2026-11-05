// r14-8 (web send): the "You can send up to …" hint said a ceiling against a
// 0.110808 USDC fee while the confirm charged 0.0984, and 0.012408 stayed in
// the account. The rule: the ceiling is the confirm's OWN quote's — the fee
// drawn beside it and signed with it — re-derived each time that quote moves,
// and it is the very figure Max fills (which on this account used to fill 0:
// it held back 1.5× the fee).
//
// Driven through the REAL send core (wasm), answered by hand the way the
// executor answers it, and worded by this shell's own `liveSendConfirm` /
// `liveSendForm` — so what is asserted is what the page would say.
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { SendCore } from '$lib/core/client';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendOperation } from '$lib/core/generated/SendOperation';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { fromBaseUnits } from '$lib/services/eip681';
import { tokenAmountText } from '$lib/wallet/live';
import { fill } from '$lib/wallet/messages';
import { buildFlowState } from './fixtures';
import { liveSendConfirm, liveSendForm, sendTokenId, type SendLiveInputs } from './live-send';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const ACCOUNT = '0x' + 'aa'.repeat(20);
const RECIPIENT = '0x' + 'bb'.repeat(20);
const ARC = 5042002;
const INTERFACE = '0x3600000000000000000000000000000000000000';

/** Alice on Arc: 0.1196 native USDC (18 places), the tester's balance. */
const ARC_USDC: SendToken = {
	network: 'arc-testnet',
	chain_id: ARC,
	symbol: 'USDC',
	balance: '0.1196',
	decimals: 18,
	token_address: null,
	price_usd: 1,
	logo_urls: [],
	spam: false
};
const HELD = 119_600_000_000_000_000n;

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

/** A fee of `amount` (6 places) in Arc's `0x3600…` interface of the USDC sent. */
function interfaceFee(amount: string): FeeEstimateView {
	return {
		chain_id: ARC,
		total_wei: '0',
		max_fee_per_gas: '0',
		network_fee_per_gas: '20000000000',
		relayer_fee_per_gas: '0',
		bundler_gas_price: '20000000000',
		in_band_gas_basis: '20000000000',
		effective_gas_price: null,
		max_gas_price: null,
		total_gas: '450000',
		deployed: true,
		tier: 'standard',
		quoted: true,
		fee_asset: { type: 'erc20', token: INTERFACE, decimals: 6, amount, symbol: 'USDC' },
		fee_recipient: '0x' + '11'.repeat(20)
	};
}

type Out = { view: SendView; effects: { id: number; operation: SendOperation }[] };

/**
 * The send core, answered by hand: every quote it asks for with `quote` (a
 * refusal while there is none), the treasury covered, every check silent.
 * The clock never runs and nothing is submitted.
 */
class Drive {
	readonly core = new SendCore();
	view: SendView;
	quote: FeeEstimateView | null = null;
	private pending: Out['effects'] = [];

	constructor() {
		this.view = JSON.parse(this.core.view()) as SendView;
	}

	dispatch(event: SendEvent): void {
		this.take(this.core.dispatch(JSON.stringify(event)));
	}

	private take(text: string): void {
		const out = JSON.parse(text) as Out;
		if (out.view) this.view = out.view;
		this.pending.push(...out.effects);
		for (let step = 0; step < 64; step += 1) {
			const index = this.pending.findIndex((effect) => this.answer(effect.operation) !== null);
			if (index < 0) return;
			const [effect] = this.pending.splice(index, 1);
			const next = JSON.parse(
				this.core.resolve_effect(BigInt(effect.id), JSON.stringify(this.answer(effect.operation)))
			) as Out;
			if (next.view) this.view = next.view;
			this.pending.push(...next.effects);
		}
	}

	private answer(operation: SendOperation): SendShellResult | null {
		switch (operation.type) {
			case 'fetch_tokens':
				return {
					type: 'tokens_loaded',
					tokens: [ARC_USDC],
					chains: [{ chain_id: ARC, network: 'arc-testnet', native_symbol: 'USDC' }]
				};
			case 'prewarm_fees':
				return { type: 'fees_prewarmed' };
			case 'load_account_credential':
				return { type: 'account_credential', public_key_hex: '04deadbeef' };
			case 'estimate_fee':
				return {
					type: 'fee_estimated',
					outcome:
						this.quote === null
							? { type: 'failed', kind: 'quote_unavailable' }
							: { type: 'ok', estimate: this.quote }
				};
			case 'probe_treasury':
				return { type: 'treasury_probed', probe: { type: 'covered' } };
			case 'resolve_risk':
				return { type: 'risk_resolved', risk: null };
			case 'resolve_identity':
				return { type: 'identity_resolved', identity: null };
			case 'simulate_calls':
				return { type: 'sim_resolved', sim_json: null };
			default:
				return null;
		}
	}

	dispose(): void {
		this.core.free();
	}
}

function inputs(send: SendView): SendLiveInputs {
	return {
		send,
		fee: IDLE_FEE,
		m,
		currency: { code: 'USD', rate: 1, committed: true, pending: null },
		identity: { name: 'Alice', address: ACCOUNT, identiconSvg: '<svg/>' },
		identicon
	};
}

function formModel() {
	const state = buildFlowState('sd2', m, identicon);
	if (state.base.kind !== 'send-form') throw new Error('kind');
	return state.base.model;
}

function confirmModel() {
	const state = buildFlowState('sd3', m, identicon);
	if (state.base.kind !== 'send-confirm') throw new Error('kind');
	return state.base.model;
}

/** The ceiling sentence as the page words it for `most` base units. */
function upTo(most: bigint): string {
	return fill(m['send.sameFeeTokenMax'], {
		amount: tokenAmountText(fromBaseUnits(most, 18), 'down'),
		symbol: 'USDC'
	});
}

/** The fee the confirm draws (and signs), in the 18 places of the coin sent. */
function drawnFee(view: SendView): bigint {
	const asset = view.fee?.fee_asset;
	if (asset?.type !== 'erc20') throw new Error('an interface fee is drawn');
	return BigInt(asset.amount) * 10n ** 12n;
}

function units(figure: string): bigint {
	const [whole, frac = ''] = figure.split('.');
	return BigInt(whole + frac.padEnd(18, '0').slice(0, 18));
}

/** Open Alice's send on Arc, USDC picked, Bob typed. */
function opened(): Drive {
	const drive = new Drive();
	drive.dispatch({
		type: 'open',
		account: { id: 'cred-1', address: ACCOUNT, name: 'Alice' },
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
	drive.dispatch({ type: 'select_token', token_id: sendTokenId(ARC_USDC) });
	drive.dispatch({ type: 'set_recipient', recipient: RECIPIENT });
	return drive;
}

describe('r14-8: the most that can be sent is the confirm’s own quote’s', () => {
	it('the hint on the confirm says balance − the fee drawn there, and says it again after a re-quote', () => {
		const drive = opened();
		try {
			drive.dispatch({ type: 'set_amount', amount: '0.05' });
			// Continue: the pre-check prices 0.110808 — the tester's first figure.
			drive.quote = interfaceFee('110808');
			drive.dispatch({ type: 'continue' });
			let send = drive.view;
			expect(send.stage).toBe('confirm');
			expect(send.can_confirm).toBe(false);
			expect(liveSendConfirm(confirmModel(), inputs(send)).alert).toContain(
				upTo(HELD - drawnFee(send))
			);
			expect(liveSendConfirm(confirmModel(), inputs(send)).alert).toContain(
				upTo(8_792_000_000_000_000n)
			);

			// The fee card's next quote lands on the confirm: 0.0984.
			drive.dispatch({ type: 'fee_updated', estimate: interfaceFee('98400') });
			send = drive.view;
			expect(drawnFee(send)).toBe(98_400_000_000_000_000n);
			const said = liveSendConfirm(confirmModel(), inputs(send)).alert ?? '';
			// The ceiling is the one the fee BESIDE it allows — never the
			// earlier quote's 0.008792.
			expect(said).toContain(upTo(HELD - drawnFee(send)));
			expect(said).toContain(upTo(21_200_000_000_000_000n));
			expect(said).not.toContain(upTo(8_792_000_000_000_000n));
			expect(BigInt(send.same_asset_fee_issue?.max_transfer_amount ?? '-1')).toBe(
				HELD - drawnFee(send)
			);
		} finally {
			drive.dispose();
		}
	});

	it('Max fills the hint’s figure, and on the confirm it is the confirm’s quote’s — nothing stranded', () => {
		const drive = opened();
		try {
			// The form's quote: 0.110808. The hint and Max say one figure.
			drive.dispatch({ type: 'fee_updated', estimate: interfaceFee('110808') });
			drive.dispatch({ type: 'set_amount', amount: '0.05' });
			expect(liveSendForm(formModel(), inputs(drive.view)).alert).toContain(
				upTo(8_792_000_000_000_000n)
			);
			drive.dispatch({ type: 'tap_max' });
			expect(units(drive.view.token_amount)).toBe(8_792_000_000_000_000n);

			// Continue: the pre-check prices 0.0984. The confirm opens on THAT
			// quote, the Max with it, and says that it moved.
			drive.quote = interfaceFee('98400');
			drive.dispatch({ type: 'continue' });
			let send = drive.view;
			expect(send.stage).toBe('confirm');
			expect(units(send.confirm_amount) + drawnFee(send)).toBe(HELD);
			expect(units(send.confirm_amount)).toBe(21_200_000_000_000_000n);
			expect(send.max_moved).not.toBeNull();
			expect(send.can_confirm).toBe(true);
			expect(liveSendConfirm(confirmModel(), inputs(send)).maxNote).toBe(
				fill(m['send.maxMoved'], {
					from: send.max_moved?.from ?? '',
					to: send.max_moved?.to ?? '',
					symbol: 'USDC'
				})
			);

			// A re-quote up on the confirm: the Max follows it, never over.
			drive.dispatch({ type: 'fee_updated', estimate: interfaceFee('104000') });
			send = drive.view;
			expect(units(send.confirm_amount) + drawnFee(send)).toBe(HELD);
			expect(send.same_asset_fee_issue).toBeNull();
			expect(send.can_confirm).toBe(true);
		} finally {
			drive.dispose();
		}
	});
});
