// Ported from src/services/wallet-state-core/send-types.ts @ f9bcb278 (its fee
// codec half) — the wire↔estimate conversions and the remembered-estimate
// registry. Every bigint crosses as a decimal string; a truncated `total_wei`
// prices the confirm screen wrong, so these are fund-safety codecs.
import type { FeeAssetView } from '$lib/core/generated/FeeAssetView';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { TransactionFeeEstimate } from '$lib/services/safe-transaction';
import { fromWireAmount } from '$lib/services/amount-codec';

// ---------------------------------------------------------------------------
// Fee codec
// ---------------------------------------------------------------------------

function toFeeAssetWire(fee: TransactionFeeEstimate): FeeAssetView {
	const asset = fee.feeAsset;
	if (!asset || asset.kind === 'native') return { type: 'native' };
	return {
		type: 'erc20',
		token: asset.token,
		decimals: asset.decimals,
		amount: asset.amount.toString(),
		symbol: asset.symbol ?? null
	};
}

/** A live estimate onto the wire. Every bigint crosses as a decimal string. */
export function toFeeWire(fee: TransactionFeeEstimate): FeeEstimateView {
	return {
		chain_id: fee.chainId,
		total_wei: fee.totalWei.toString(),
		max_fee_per_gas: fee.maxFeePerGas.toString(),
		network_fee_per_gas: fee.networkFeePerGas.toString(),
		relayer_fee_per_gas: fee.relayerFeePerGas.toString(),
		bundler_gas_price: fee.bundlerGasPrice.toString(),
		in_band_gas_basis: fee.inBandGasBasis.toString(),
		// Display only (issue 684), so it crosses as "no figure" rather than as
		// a 0 when there is none — the picker draws nothing for `null` and
		// would draw "0 wei" for a zero.
		effective_gas_price: fee.effectiveGasPrice?.toString() ?? null,
		// The top of that figure's range (issue 685), absent the same way.
		max_gas_price: fee.maxGasPrice?.toString() ?? null,
		total_gas: fee.totalGas.toString(),
		deployed: fee.deployed,
		tier: fee.tier,
		quoted: fee.quoted,
		fee_asset: toFeeAssetWire(fee),
		fee_recipient: fee.feeRecipient ?? null
	};
}

/**
 * The wire estimate back into the shape `GasFeeCard` renders.
 *
 * `inBand` has no core field — the machine derives the signed quote from
 * `fee_recipient` alone (`send.rs:2847`), which is the only decision it makes on
 * it. The display flag is therefore reconstructed from the same fact, and
 * `rememberFee` below hands back the ORIGINAL object whenever the shell still
 * has it, so a Tempo estimate keeps its own `inBand` verbatim.
 */
export function fromFeeWire(view: FeeEstimateView): TransactionFeeEstimate {
	const asset = view.fee_asset;
	return {
		chainId: view.chain_id,
		totalWei: fromWireAmount(view.total_wei),
		maxFeePerGas: fromWireAmount(view.max_fee_per_gas),
		networkFeePerGas: fromWireAmount(view.network_fee_per_gas),
		relayerFeePerGas: fromWireAmount(view.relayer_fee_per_gas),
		bundlerGasPrice: fromWireAmount(view.bundler_gas_price),
		inBandGasBasis: fromWireAmount(view.in_band_gas_basis),
		// `fromWireAmount` would turn an absent figure into 0n, which this one
		// field must never be: "no honest price" and "the price is zero" are
		// different facts here (issue 684). `== null`, not `=== null`: a view
		// that arrives without the key at all (hand-built, or remembered by an
		// older build) is just as absent, and `fromWireAmount(undefined)` is 0n.
		effectiveGasPrice:
			view.effective_gas_price == null ? undefined : fromWireAmount(view.effective_gas_price),
		// The same rule for the top of its range (issue 685): an absent cap is
		// no range, and a `0n` here would draw one ending at zero.
		maxGasPrice: view.max_gas_price == null ? undefined : fromWireAmount(view.max_gas_price),
		totalGas: fromWireAmount(view.total_gas),
		deployed: view.deployed,
		tier: view.tier,
		quoted: view.quoted,
		inBand: view.fee_recipient != null,
		feeAsset:
			asset.type === 'native'
				? { kind: 'native' }
				: {
						kind: 'erc20',
						token: asset.token,
						decimals: asset.decimals,
						amount: fromWireAmount(asset.amount),
						...(asset.symbol != null ? { symbol: asset.symbol } : {})
					},
		...(view.fee_recipient != null ? { feeRecipient: view.fee_recipient } : {})
	};
}

/** Structural key of a wire estimate — the registry's identity. */
export function feeKey(view: FeeEstimateView): string {
	return JSON.stringify(view);
}

/**
 * The fee card's coin in force (`FeeView.fee_token`), as news for the send
 * machine — the bridge's half of `send::Event::FeeTokenChanged`, under the
 * core's one rule (iOS `SendStore.feeTokenChanged`, Android `FeeTokenWord`,
 * desktop `FeeTokenTold`).
 *
 * The send form's fee row names this coin while no estimate is in hand
 * (`SendView.fee_coin`): when nobody chose, the fee machine picks a coin that
 * can pay, and a quote that then fails leaves that coin in force with no
 * estimate to say so.
 *
 * The core files the word against the form's chain at the moment it is said,
 * and drops it while the form has no chain. So it is said only while the fee
 * session prices the form's own chain (both known), and whenever the pair
 * (chain, coin) differs from what this journey was last told — a coin first
 * seen before the form had a chain is told once it has one. {@link forget}
 * starts a journey that has been told nothing.
 */
export class FeeTokenWord {
	/** What this journey was last told; `null` = nothing yet. */
	#told: { chainId: number; token: string | null } | null = null;

	/**
	 * The `fee_token_changed` to dispatch for the card's coin `token`, priced
	 * on `pricing` (the fee session's chain) while the form is on `form` — or
	 * `null` when there is nothing to say.
	 */
	news(
		token: string | null,
		pricing: number | null,
		form: number | null
	): Extract<SendEvent, { type: 'fee_token_changed' }> | null {
		if (pricing === null || pricing !== form) return null;
		const told = this.#told;
		if (told !== null && told.chainId === pricing && told.token === token) return null;
		this.#told = { chainId: pricing, token };
		return { type: 'fee_token_changed', fee_token: token };
	}

	/** A new journey: nothing told yet. */
	forget(): void {
		this.#told = null;
	}
}

/**
 * The fee card's state, as news for the send machine — the bridge's half of
 * `send::Event::FeeBusyChanged` and `FeeFailedChanged` (PR 2 integration).
 *
 * `busy`: a measurement is out. `failed`: the card's `FeeView.failure` is set
 * — a failure, or the core's own re-ask after one — so the fee machine holds
 * no figure. The send machine holds its confirm while either is true, and on
 * the confirm page drops the figure it kept from Continue when the card
 * fails: between two of the core's re-asks the confirm used to open on a
 * figure the fee machine had discarded, and shut again with the next re-ask.
 *
 * Each word is said when it changes. The send machine starts not busy, so a
 * journey's first "not busy" is not said; its first word of the failure is
 * always said ({@link forget} starts a journey told nothing).
 */
export class FeeStateWord {
	#busy = false;
	/** What this journey was last told of the failure; `null` = nothing yet. */
	#failed: boolean | null = null;

	/** The events to dispatch for the card's view, in order — empty when nothing changed. */
	news(view: Pick<FeeView, 'busy' | 'failure'>): SendEvent[] {
		const out: SendEvent[] = [];
		if (view.busy !== this.#busy) {
			this.#busy = view.busy;
			out.push({ type: 'fee_busy_changed', busy: view.busy });
		}
		const failed = view.failure !== null;
		if (failed !== this.#failed) {
			this.#failed = failed;
			out.push({ type: 'fee_failed_changed', failed });
		}
		return out;
	}

	/** A new journey: nothing told yet. */
	forget(): void {
		this.#busy = false;
		this.#failed = null;
	}
}

/**
 * The estimates this process actually produced, by structural key.
 *
 * The shell is the ONLY producer of a `FeeEstimateView` (the executor's
 * `EstimateFee`, and `GasFeeCard`'s re-quote), so a view coming back out of the
 * core is nearly always one of ours. Handing the ORIGINAL object back keeps two
 * things a round trip would otherwise cost: the fields the wire does not carry
 * (`inBand` on a Tempo quote whose relay named no recipient), and reference
 * stability for `GasFeeCard`, which keys its own work off the estimate it is
 * given.
 *
 * Bounded and content-addressed: an entry is only ever re-read by a view that is
 * byte-identical to the one it was stored for, so a stale entry is impossible —
 * only forgettable.
 */
const MAX_REMEMBERED_FEES = 16;
const rememberedFees = new Map<string, TransactionFeeEstimate>();

export function rememberFee(fee: TransactionFeeEstimate): FeeEstimateView {
	const view = toFeeWire(fee);
	const key = feeKey(view);
	rememberedFees.delete(key);
	if (rememberedFees.size >= MAX_REMEMBERED_FEES) {
		const oldest = rememberedFees.keys().next().value;
		if (oldest !== undefined) rememberedFees.delete(oldest);
	}
	rememberedFees.set(key, fee);
	return view;
}

/** The original estimate behind a wire view, or a faithful reconstruction. */
export function resolveFee(view: FeeEstimateView | null): TransactionFeeEstimate | null {
	if (!view) return null;
	return rememberedFees.get(feeKey(view)) ?? fromFeeWire(view);
}

/** Test seam: forget every remembered estimate. */
export function _resetFeeRegistry(): void {
	rememberedFees.clear();
}
