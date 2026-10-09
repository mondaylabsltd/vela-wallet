/**
 * A failed fee, as every surface that draws one reads it (PR 2 polish) — the
 * send form's row, its confirm's fee line, the signing sheet's row, and the
 * bridge that tells the send machine the fee card failed.
 *
 * Two rules, both the core's, said once here so no surface keeps its own:
 *
 * - A failure is for one question (`FeeFailureView.chain_id`): right after a
 *   token switch the send form names another chain before the fee machine has
 *   been asked about it, and the old chain's failure flashed there. A surface
 *   draws a failure only for its own chain — `FeeFailureView::is_for_chain`.
 * - The row does exactly what its words say (`FeeFailureView.tap`): "Tap to
 *   retry" asks again, "Pay with another coin" opens the fee coins, and a dash
 *   with nothing behind it is no control at all. A row that said one and did
 *   the other was a control that lied.
 */
import type { FeeFailureView } from '$lib/core/generated/FeeFailureView';

/**
 * The failure a surface asking about `chainId` may draw — the core's
 * `FeeFailureView::is_for_chain`: one tagged with another chain is dropped
 * (no figure, no reason, no footer, not "failed"); one built without a run
 * (`chain_id` `null`) is taken as it is.
 */
export function failureForChain(
	failure: FeeFailureView | null | undefined,
	chainId: number | null
): FeeFailureView | null {
	if (failure === null || failure === undefined) return null;
	return failure.chain_id === null || failure.chain_id === chainId ? failure : null;
}

/** What a tap on a failed fee row does, as a surface acts on it. */
export type FailedFeeAction = 'requote' | 'open_coins' | 'none';

/**
 * The core's `tap`, as the action: `retry` asks again at once (`requote`) —
 * and nothing while a re-ask is already out (`busy`: it is the answer
 * awaited, and the row stays the same control through it); `choose_coin`
 * opens the fee coins; `nothing` is no control.
 */
export function failedFeeAction(failure: FeeFailureView, busy: boolean): FailedFeeAction {
	switch (failure.tap) {
		case 'retry':
			return busy ? 'none' : 'requote';
		case 'choose_coin':
			return 'open_coins';
		case 'nothing':
			return 'none';
	}
}

/**
 * What a tap on the signing sheet's fee row does — exactly what its drawing
 * promises (`signing/live.ts` `feeModel`). A failed fee does what the core
 * says: asks again at once (`requote` — nothing while a re-ask is out), opens
 * the fee coins (`toggle_coins`), or nothing. Otherwise two or more coins
 * open their list, and one coin is the host's own surface, if any.
 */
export function feeRowTap(
	fee: { failure: FeeFailureView | null; busy: boolean },
	coins: number
): 'requote' | 'toggle_coins' | 'host' | 'none' {
	if (fee.failure) {
		const action = failedFeeAction(fee.failure, fee.busy);
		return action === 'requote' ? 'requote' : action === 'open_coins' ? 'toggle_coins' : 'none';
	}
	return coins > 1 ? 'toggle_coins' : 'host';
}

/** Whether the failed row is a control at all: everything but `nothing`. */
export function failedFeeTappable(failure: FeeFailureView): boolean {
	return failure.tap !== 'nothing';
}

/**
 * The corpus keys a failed row's figure can be (`FeeFailureView.figure_key`):
 * "Tap to retry" when only a tap asks again, "Pay with another coin" when the
 * tap opens the coins. `null` (and a key this build does not carry) is the
 * dash — never a dotted path.
 */
export const FEE_FIGURE_KEYS = [
	'componentsUi.gas.estimateFailed',
	'componentsUi.gas.payWithAnotherCoin'
] as const;
