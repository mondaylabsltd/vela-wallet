package app.getvela.wallet.feature.settings.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `sign_pref` machine's wire types (spec 071) — a transcription of
 * `rust/crates/vela-core/src/app/sign_pref.rs` on the [FeeTierPrefView]
 * template. The web mirrors are
 * `app-web/vela-wallet/src/lib/core/generated/SignPref*.ts`.
 *
 * How this device signs by default: the "Sign with" every signing sheet
 * starts at, and which Clear Signer page it opens. A pick on a signing sheet
 * is one request's and never comes here.
 */

@Serializable
data class SignPrefView(
    /** Always an offered name — `auto` when nothing was chosen. */
    val method: String = "auto",
    val method_committed: Boolean = false,
    /** Every "Sign with" value, in the order a picker lists them. */
    val offered: List<String> = listOf("auto", "platform", "hybrid", "security_key", "clear_signer"),
    /** The page the Clear Signer opens — always usable. */
    val signer_url: String = "https://sign.getvela.app/",
    val signer_url_is_default: Boolean = true,
    /** `invalid` | `insecure`: the last address typed was refused, nothing stored. */
    val signer_url_error: String? = null,
    /** A page off `getvela.app` can show a request but cannot use this wallet's passkeys. */
    val signer_uses_wallet_passkeys: Boolean = true,
)

@Serializable
sealed class SignPrefEvent {
    @Serializable
    @SerialName("refresh")
    data object Refresh : SignPrefEvent()

    /** Settings only. */
    @Serializable
    @SerialName("method_chosen")
    data class MethodChosen(val method: String) : SignPrefEvent()

    @Serializable
    @SerialName("signer_url_submitted")
    data class SignerUrlSubmitted(val text: String) : SignPrefEvent()

    @Serializable
    @SerialName("signer_url_reset")
    data object SignerUrlReset : SignPrefEvent()
}

@Serializable
sealed class SignPrefOperation {
    /** Read `vela.signMethod` and `vela.clearSignerUrl`, raw. */
    @Serializable
    @SerialName("read_stored")
    data object ReadStored : SignPrefOperation()

    @Serializable
    @SerialName("write_method")
    data class WriteMethod(val method: String) : SignPrefOperation()

    /** `null` removes the key: the official page. */
    @Serializable
    @SerialName("write_signer_url")
    data class WriteSignerUrl(val url: String? = null) : SignPrefOperation()
}

/** Stored values go back RAW — whether they are usable is the core's call. */
@Serializable
sealed class SignPrefShellResult {
    @Serializable
    @SerialName("stored")
    data class Stored(
        val method: String? = null,
        val signer_url: String? = null,
    ) : SignPrefShellResult()

    @Serializable
    @SerialName("written")
    data object Written : SignPrefShellResult()
}
