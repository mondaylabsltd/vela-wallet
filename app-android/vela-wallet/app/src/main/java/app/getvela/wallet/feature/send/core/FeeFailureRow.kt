package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * A failed fee row, as every surface that draws one says it and answers a
 * tap on it — the send form, its confirm and the signing sheet (PR 2
 * polish). The words and the tap are the core's ([FeeFailureView.tap],
 * [FeeFailureView.figure_key], [FeeFailureView.footer_key]); this only turns
 * them into what is drawn and what a tap does, once, so the three surfaces
 * cannot drift apart.
 *
 * The rule: a control does exactly what its words say. "Tap to retry" asks
 * again; "Pay with another coin" opens the coins; a dash promises nothing,
 * and when the core says a tap does nothing the row is no control at all.
 */
object FeeFailureRow {

    /** What a tap on a fee row does now. */
    enum class Tap {
        /** Ask again at once — the fee machine's `Requote`, a real new read. */
        Retry,

        /** Open the fee coins — the same list the coin opener opens. */
        OpenCoins,

        /** Nothing: the failure says no tap helps, or a re-ask is already out. */
        Ignore,
    }

    /**
     * What a tap on a row over [failure] does. No failure is the row's own
     * tap — its coins. While the core's re-ask is out ([FeeFailureView.retrying])
     * a second tap asks nothing, as the refresh control refuses one.
     */
    fun tap(failure: FeeFailureView?): Tap = when (failure?.tap) {
        null -> Tap.OpenCoins
        FeeFailureTap.Retry -> if (failure.retrying) Tap.Ignore else Tap.Retry
        FeeFailureTap.ChooseCoin -> Tap.OpenCoins
        FeeFailureTap.None -> Tap.Ignore
    }

    /** Whether the row over [failure] is a control at all: no tap target, no chevron, when a tap does nothing. */
    fun isControl(failure: FeeFailureView): Boolean = failure.tap != FeeFailureTap.None

    /** The row's figure keys this build draws; any other is the dash, never a dotted path. */
    private val FIGURES = setOf(I18nKeys.Flows.FEE_TAP_TO_RETRY, I18nKeys.Flows.FEE_PAY_WITH_ANOTHER_COIN)

    /** The lines under a held confirm this build draws. */
    private val FOOTERS = setOf(I18nKeys.Flows.FEE_RETRYING, I18nKeys.Flows.FEE_FAILED, I18nKeys.Flows.FEE_WOULD_FAIL)

    /** The row's figure in words: "Tap to retry", "Pay with another coin", else the dash. */
    fun figure(failure: FeeFailureView, s: VelaStrings): String =
        failure.figure_key?.takeIf { it in FIGURES }?.let(s::t) ?: DASH

    /** The line under the held confirm in words, or `null` for a key this build does not know. */
    fun footer(failure: FeeFailureView, s: VelaStrings): String? =
        failure.footer_key.takeIf { it in FOOTERS }?.let(s::t)

    /**
     * The failure a surface on [formChain] may draw: one for another chain's
     * question is dropped whole — no reason line, no figure, no footer — and
     * does not count as failed (the core's `is_for_chain`; [formChain] is the
     * send form's chain, `formChain(view)`).
     */
    fun forChain(fee: FeeView?, formChain: Int?): FeeFailureView? =
        fee?.failure?.takeIf { it.isForChain(formChain) }

    const val DASH = "—"
}
