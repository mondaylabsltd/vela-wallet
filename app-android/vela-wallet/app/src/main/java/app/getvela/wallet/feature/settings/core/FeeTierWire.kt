package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.feature.send.core.FeeTier
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `fee_tier_pref` machine's wire types, in Kotlin (spec 068; Android's
 * since 069) — a transcription of `rust/crates/vela-core/src/app/fee_tier_pref.rs`
 * on the `CurrencyWire` template. The web mirrors are
 * `app-web/vela-wallet/src/lib/core/generated/FeeTierPref*.ts`.
 *
 * The default transaction speed: the tier every send STARTS at. A pick on the
 * send screen is one-shot and never comes here — only Settings raises
 * [FeeTierPrefEvent.UserChose].
 */

/** `FeeTierPrefView`. `tier` is always a real tier — the factory `fast` when nothing was chosen. */
@Serializable
data class FeeTierPrefView(
    val tier: FeeTier = FeeTier.Fast,
    /** `false` ⇒ the factory default is showing, not a choice. */
    val committed: Boolean = false,
    /** The tiers Settings may offer, fastest first — never the dead `rapid`. */
    val offered: List<FeeTier> = listOf(FeeTier.Fast, FeeTier.Standard, FeeTier.Slow),
)

@Serializable
sealed class FeeTierPrefEvent {
    /** First mount / screen focus. Coalesced while a read is out. */
    @Serializable
    @SerialName("refresh")
    data object Refresh : FeeTierPrefEvent()

    /** An explicit pick in Settings — and only in Settings. */
    @Serializable
    @SerialName("user_chose")
    data class UserChose(val tier: FeeTier) : FeeTierPrefEvent()
}

@Serializable
sealed class FeeTierPrefOperation {
    /** Read `vela.feeTier`. Absent ALWAYS means "never chose". */
    @Serializable
    @SerialName("read_stored_tier")
    data object ReadStoredTier : FeeTierPrefOperation()

    @Serializable
    @SerialName("write_stored_tier")
    data class WriteStoredTier(val tier: String) : FeeTierPrefOperation()
}

/** The stored value goes back RAW — whether it is a tier is the core's call. */
@Serializable
sealed class FeeTierPrefShellResult {
    @Serializable
    @SerialName("stored_tier")
    data class StoredTier(val raw: String? = null) : FeeTierPrefShellResult()

    @Serializable
    @SerialName("tier_written")
    data object TierWritten : FeeTierPrefShellResult()
}
