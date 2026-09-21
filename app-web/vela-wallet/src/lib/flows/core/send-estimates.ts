// Ported from src/services/wallet-state-core/send-types.ts @ f9bcb278 (its fee
// codec half) — the wire↔estimate conversions and the remembered-estimate
// registry. Every bigint crosses as a decimal string; a truncated `total_wei`
// prices the confirm screen wrong, so these are fund-safety codecs.
import type { FeeAssetView } from '$lib/core/generated/FeeAssetView';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
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
