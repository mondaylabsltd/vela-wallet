package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TimeFormatKey
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `clear_signing` machine's wire (spec 044) — what a request DOES and
 * how dangerous it is. `chain_id` is `u32`, `usd_value` `f64`, a timer's
 * `ms`/`token` `u32`, `tz_offset_minutes` `i32`.
 */

@Serializable
enum class ClearFieldRole {
    @SerialName("send_amount") SendAmount,

    @SerialName("receive_amount") ReceiveAmount,

    @SerialName("recipient") Recipient,

    @SerialName("spender") Spender,

    @SerialName("generic") Generic,
}

@Serializable
enum class ClearSignType {
    @SerialName("transaction") Transaction,

    @SerialName("signature") Signature,
}

@Serializable
enum class ClearSignMethod {
    @SerialName("personal_sign") PersonalSign,

    @SerialName("eth_sign") EthSign,
}

@Serializable
enum class ClearRisk {
    @SerialName("safe") Safe,

    @SerialName("normal") Normal,

    @SerialName("caution") Caution,

    @SerialName("danger") Danger,
}

@Serializable
enum class ClearDangerClass {
    @SerialName("plain") Plain,

    @SerialName("siwe_ok") SiweOk,

    @SerialName("siwe_phish") SiwePhish,

    @SerialName("opaque_hash") OpaqueHash,

    @SerialName("eth_sign") EthSign,
}

@Serializable
enum class ClearSurface {
    @SerialName("none") None,

    @SerialName("loading") Loading,

    @SerialName("clear_sign") ClearSign,

    @SerialName("eth_sign") EthSign,

    @SerialName("message_sign") MessageSign,

    @SerialName("blind_typed_data") BlindTypedData,

    @SerialName("blind_transaction") BlindTransaction,
}

@Serializable
enum class ClearProbe {
    @SerialName("supports_erc721") SupportsErc721,

    @SerialName("supports_erc1155") SupportsErc1155,

    @SerialName("decimals") Decimals,

    @SerialName("symbol") Symbol,
}

@Serializable
enum class ClearSiweBinding {
    @SerialName("ok") Ok,

    @SerialName("mismatch") Mismatch,

    @SerialName("unknown") Unknown,
}

@Serializable
enum class ClearDateFormat {
    @SerialName("ymd_slash") YmdSlash,

    @SerialName("iso") Iso,

    @SerialName("dmy_slash") DmySlash,

    @SerialName("dmy_dot") DmyDot,

    @SerialName("mdy_slash") MdySlash,
}

@Serializable
enum class ClearNumberFormat {
    @SerialName("comma_dot") CommaDot,

    @SerialName("dot_comma") DotComma,

    @SerialName("space_comma") SpaceComma,

    @SerialName("indian") Indian,
}

@Serializable
enum class ClearTimeFormat {
    @SerialName("h24") H24,

    @SerialName("h12") H12,
}

@Serializable
data class ClearLocale(
    val number_format: ClearNumberFormat = ClearNumberFormat.CommaDot,
    val date_format: ClearDateFormat = ClearDateFormat.Iso,
    val time_format: ClearTimeFormat = ClearTimeFormat.H24,
    val tz_offset_minutes: Int = 0,
) {
    companion object {
        /**
         * Spec 049: the person's resolved presets and the phone's UTC offset
         * now — the web's `toClearLocale(resolvedFormatKeys())` shape. The
         * constant that stood in before printed a dApp's amounts in a format
         * the person had not chosen.
         */
        fun fromFormats(formats: Formats, nowMs: Long = System.currentTimeMillis()): ClearLocale = ClearLocale(
            number_format = when (formats.resolvedNumber()) {
                NumberFormatKey.DotComma -> ClearNumberFormat.DotComma
                NumberFormatKey.SpaceComma -> ClearNumberFormat.SpaceComma
                NumberFormatKey.Indian -> ClearNumberFormat.Indian
                NumberFormatKey.CommaDot, NumberFormatKey.Auto -> ClearNumberFormat.CommaDot
            },
            date_format = when (formats.resolvedDate()) {
                DateFormatKey.YmdSlash -> ClearDateFormat.YmdSlash
                DateFormatKey.MdySlash -> ClearDateFormat.MdySlash
                DateFormatKey.DmySlash -> ClearDateFormat.DmySlash
                DateFormatKey.DmyDot -> ClearDateFormat.DmyDot
                DateFormatKey.Iso, DateFormatKey.Auto -> ClearDateFormat.Iso
            },
            time_format = if (formats.resolvedTime() == TimeFormatKey.H12) ClearTimeFormat.H12 else ClearTimeFormat.H24,
            // Minutes to ADD to UTC (the web negates `getTimezoneOffset()`).
            tz_offset_minutes = java.util.TimeZone.getDefault().getOffset(nowMs) / 60_000,
        )
    }
}

@Serializable
sealed class ClearConfirm {
    @Serializable
    @SerialName("sign")
    data object Sign : ClearConfirm()

    @Serializable
    @SerialName("confirm")
    data object Confirm : ClearConfirm()

    @Serializable
    @SerialName("confirm_intent")
    data class ConfirmIntent(val intent: String) : ClearConfirm()
}

@Serializable
data class ClearSignField(
    val label: String,
    val value: String,
    val format: String = "",
    val token_address: String? = null,
    val warning: Boolean = false,
    val unverified: Boolean = false,
    val role: ClearFieldRole = ClearFieldRole.Generic,
    val detail: Boolean = false,
    val expired: Boolean = false,
    val address: String? = null,
    val usd_value: Double? = null,
)

@Serializable
data class ClearSignResult(
    val intent: String,
    val contract_name: String? = null,
    val owner: String? = null,
    val fields: List<ClearSignField> = emptyList(),
    val risk: ClearRisk = ClearRisk.Normal,
    val contract_address: String? = null,
    val verified: Boolean = false,
    val sign_type: ClearSignType = ClearSignType.Transaction,
    val partial: Boolean = false,
    val best_effort: Boolean = false,
    val to_own_token: Boolean = false,
)

@Serializable
data class ClearBlindField(val key: String, val value: String)

@Serializable
data class ClearBlindTyped(
    val primary_type: String? = null,
    val has_domain: Boolean = false,
    val domain_name: String? = null,
    val verifying_contract: String? = null,
    val fields: List<ClearBlindField> = emptyList(),
)

@Serializable
data class ClearSiweFields(
    val domain: String,
    val domain_host: String? = null,
    val address: String? = null,
    val statement: String? = null,
    val uri: String? = null,
    val chain_id: Int? = null,
    val nonce: String? = null,
)

@Serializable
data class ClearMessageView(
    val payload: String,
    val is_hex: Boolean = false,
    val decoded_text: String? = null,
    val binary_preview: String? = null,
    val non_printable: Boolean = false,
    val siwe: ClearSiweFields? = null,
    val binding: ClearSiweBinding? = null,
    val danger_class: ClearDangerClass = ClearDangerClass.Plain,
)

@Serializable
data class ClearSigningView(
    val resolving: Boolean = false,
    val resolved: Boolean = false,
    val result: ClearSignResult? = null,
    val message: ClearMessageView? = null,
    val surface: ClearSurface = ClearSurface.None,
    val confirm: ClearConfirm = ClearConfirm.Confirm,
    val blind_typed: ClearBlindTyped? = null,
    val danger_haptic: Boolean = false,
)

@Serializable
sealed class ClearOperation {
    @Serializable
    @SerialName("http_get")
    data class HttpGet(val path: String) : ClearOperation()

    @Serializable
    @SerialName("rpc_eth_call")
    data class RpcEthCall(val chain_id: Int, val to: String, val data: String, val probe: ClearProbe) : ClearOperation()

    @Serializable
    @SerialName("selector_db_lookup")
    data class SelectorDbLookup(val selector: String) : ClearOperation()

    @Serializable
    @SerialName("timer")
    data class Timer(val ms: Int, val token: Int) : ClearOperation()

    @Serializable
    @SerialName("now")
    data object Now : ClearOperation()
}

@Serializable
sealed class ClearShellResult {
    @Serializable
    @SerialName("descriptor_fetched")
    data class DescriptorFetched(val path: String, val json: String? = null) : ClearShellResult()

    @Serializable
    @SerialName("rpc_answer")
    data class RpcAnswer(
        val probe: ClearProbe,
        val chain_id: Int,
        val to: String,
        val result: String? = null,
        val rpc_error: Boolean = false,
    ) : ClearShellResult()

    @Serializable
    @SerialName("selector_candidates")
    data class SelectorCandidates(val sigs: List<String> = emptyList()) : ClearShellResult()

    @Serializable
    @SerialName("timed_out")
    data class TimedOut(val token: Int) : ClearShellResult()

    @Serializable
    @SerialName("clock")
    data class Clock(val now_ms: Double) : ClearShellResult()
}

@Serializable
sealed class ClearSigningEvent {
    @Serializable
    @SerialName("resolve_transaction")
    data class ResolveTransaction(
        val to: String? = null,
        val data: String? = null,
        val value: String? = null,
        val chain_id: Int,
        val locale: ClearLocale = ClearLocale(),
    ) : ClearSigningEvent()

    @Serializable
    @SerialName("resolve_typed_data")
    data class ResolveTypedData(val typed_data_json: String, val chain_id: Int, val locale: ClearLocale = ClearLocale()) : ClearSigningEvent()

    @Serializable
    @SerialName("message_presented")
    data class MessagePresented(val method: ClearSignMethod, val params: List<String>, val request_origin: String? = null) : ClearSigningEvent()

    @Serializable
    @SerialName("cleared")
    data object Cleared : ClearSigningEvent()
}
