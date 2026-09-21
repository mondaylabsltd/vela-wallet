package app.getvela.wallet.feature.send.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `fee_speed` machine's wire types, in Kotlin (spec 069) — a transcription
 * of `rust/crates/vela-core/src/app/fee_speed.rs`. The web mirrors are
 * `app-web/vela-wallet/src/lib/core/generated/FeeSpeed*.ts` / `TierQuote.ts`.
 *
 * Every rule of the speed control is that machine's: the tier in force, the
 * free upgrade, the one-speed statement, each tier's gas bid as text, and
 * which other tiers to keep priced. It asks the shell for nothing — the
 * shell keeps the fee sessions and reconciles them against [FeeSpeedView].
 */

/** One fee session's state, as the shell holds it. */
@Serializable
data class TierQuote(
    /** A measurement is out — the core's `busy` OR the shell's own read before it. */
    val busy: Boolean = false,
    /** Its settled quote, of whatever tier it last priced; the core judges the tier. */
    val fee: FeeEstimateView? = null,
)

/** A preview session: the tier it prices, and its state. */
@Serializable
data class TierPreviewQuote(
    val tier: FeeTier,
    val busy: Boolean = false,
    val fee: FeeEstimateView? = null,
)

@Serializable
sealed class FeeSpeedEvent {
    /** The stored default and the resolved number preset (`comma_dot`, …). */
    @Serializable
    @SerialName("configure")
    data class Configure(val preferred: FeeTier, val number: String) : FeeSpeedEvent()

    /** A send starts or ends: forget the pick, the upgrade, the fold, the quotes. */
    @Serializable
    @SerialName("reset")
    data object Reset : FeeSpeedEvent()

    /** A free upgrade is only decided on the form. */
    @Serializable
    @SerialName("stage_changed")
    data class StageChanged(val on_form: Boolean) : FeeSpeedEvent()

    @Serializable
    @SerialName("toggle")
    data object Toggle : FeeSpeedEvent()

    /** One-shot: never reaches the stored preference. Folds the control. */
    @Serializable
    @SerialName("pick")
    data class Pick(val tier: FeeTier) : FeeSpeedEvent()

    /** Every session's state, whole, after any of them changed. */
    @Serializable
    @SerialName("quotes_changed")
    data class QuotesChanged(
        val chain_id: Int? = null,
        val in_force: TierQuote,
        val previews: List<TierPreviewQuote>,
    ) : FeeSpeedEvent()
}

/** One option of the control, fastest first. */
@Serializable
data class FeeSpeedOptionView(
    val tier: FeeTier,
    val selected: Boolean = false,
    /** This tier's OWN settled quote — formatted exactly as the fee row formats one. */
    val fee: FeeEstimateView? = null,
    /** No figure of its own but one is coming: "…". Neither: "—". */
    val measuring: Boolean = false,
    /** Its gas bid as a range, formatted by the core over the whole set. */
    val gas_price: String? = null,
)

@Serializable
data class FeeSpeedView(
    /** The tier THIS send runs at — the session in force prices it, the submit names it. */
    val tier: FeeTier = FeeTier.Fast,
    val preferred: FeeTier = FeeTier.Fast,
    /** The other tiers to keep priced. */
    val previews: List<FeeTier> = emptyList(),
    val open: Boolean = false,
    val picked: Boolean = false,
    val free: Boolean = false,
    val free_note: Boolean = false,
    val single: Boolean = false,
    val gas_price_line: Boolean = true,
    val options: List<FeeSpeedOptionView> = emptyList(),
)

/** The machine asks for nothing; these exist because the bridge's contract is ops in, results out. */
@Serializable
sealed class FeeSpeedOperation

@Serializable
sealed class FeeSpeedShellResult
