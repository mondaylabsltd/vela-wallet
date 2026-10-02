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

/**
 * Where a description came from (spec 081 FR-008) — the ground `verified`
 * stands on. `Fetched` is the descriptor service's word over plain HTTP, from
 * a base URL the person can edit; `None` is a sheet no descriptor described.
 * Defaulted, like every other field here, so a core built before the field
 * existed still decodes — and it defaults to the claim that promises least.
 */
@Serializable
enum class ClearProvenance {
    @SerialName("built_in") BuiltIn,

    @SerialName("pinned_match") PinnedMatch,

    @SerialName("fetched") Fetched,

    @SerialName("standard") Standard,

    @SerialName("selector_db") SelectorDb,

    @SerialName("none") None,
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

    /** Spec 082 RC1: empty calldata — a plain value transfer, drawn from [ClearSigningView.plain_send]. */
    @SerialName("plain_send") PlainSend,

    /** 089 S1: a batch of two or more calls — every call drawn, from [ClearSigningView.batch]. */
    @SerialName("batch") Batch,
}

/**
 * A dApp's plain value transfer, as the core read it (spec 082 RC1–RC5):
 * the recipient in EIP-55, the exact wei, the amount scaled by 18 in the
 * reader's number marks, and whether nothing moves (`no_value`: "Send · 0",
 * no minus, a neutral confirm). The coin's symbol is the fee row's.
 */
@Serializable
data class ClearPlainSend(
    val to: String,
    val value_wei: String,
    val amount: String,
    val no_value: Boolean = false,
)

/**
 * 089 S1: one call of a batch as the core read it — by the ladder a lone
 * transaction climbs. [surface] is `ClearSign` (from [result]), `PlainSend`
 * (from [plain_send]) or `BlindTransaction`; never omitted.
 */
@Serializable
data class ClearBatchCall(
    val index: Int,
    val surface: ClearSurface = ClearSurface.BlindTransaction,
    val result: ClearSignResult? = null,
    val plain_send: ClearPlainSend? = null,
    val to: String? = null,
    /** Spec 096 F5: the target's name, when the wallet itself knows the contract on this chain. */
    val to_name: String? = null,
    val data_bytes: Int = 0,
    val value_wei: String? = null,
    val amount: String? = null,
    val risk: ClearRisk = ClearRisk.Caution,
)

/** 089 S1: every call of a batch, what it moves in all, and its worst call's risk. */
@Serializable
data class ClearBatchView(
    val calls: List<ClearBatchCall> = emptyList(),
    val total_value_wei: String? = null,
    val total_amount: String? = null,
    val risk: ClearRisk = ClearRisk.Caution,
)

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
    data class ConfirmIntent(
        val intent: String,
        /** The core's name for `intent` when it has one (`ClearTerm`: the key leaf under `componentsUi.signing`). */
        val intent_term: String? = null,
    ) : ClearConfirm()
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
    /** `label` / `value` as words the shell translates (`ClearTerm`). See `SigningLive.localizedTerms`. */
    val label_term: String? = null,
    val value_term: String? = null,
)

@Serializable
data class ClearSignResult(
    val intent: String,
    /** `intent` as a word the shell translates (`ClearTerm`). */
    val intent_term: String? = null,
    val contract_name: String? = null,
    val owner: String? = null,
    val fields: List<ClearSignField> = emptyList(),
    val risk: ClearRisk = ClearRisk.Normal,
    val contract_address: String? = null,
    val verified: Boolean = false,
    val provenance: ClearProvenance = ClearProvenance.Fetched,
    val sign_type: ClearSignType = ClearSignType.Transaction,
    val partial: Boolean = false,
    val best_effort: Boolean = false,
    val to_own_token: Boolean = false,
    /** Spec 096 F5: the call signs an order whose amounts live off chain (a CoW pre-signature). */
    val terms_off_chain: Boolean = false,
)

/** Spec 096 F4: the chain's own coin a lone call sends, exact. */
@Serializable
data class ClearNativeValue(val value_wei: String, val amount: String)

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
    /** Some iff [surface] is [ClearSurface.PlainSend]. */
    val plain_send: ClearPlainSend? = null,
    /** 089 S1: Some iff [surface] is [ClearSurface.Batch] — and then [result] and [plain_send] are null. */
    val batch: ClearBatchView? = null,
    /**
     * Spec 093: the verb the request's record keeps — Activity's title.
     * Copied to `SignApproveOpts.intent`; `null` = nothing may be recorded.
     */
    val record_intent: String? = null,
    /**
     * Spec 096 F4: the coin a lone contract call sends, when its reading does
     * not say it — on `ClearSign` and `BlindTransaction` only.
     */
    val native_value: ClearNativeValue? = null,
    /** Spec 097: what the reading named — the contract and the coins; copied to `SignApproveOpts.reading`. */
    val record_reading: app.getvela.wallet.feature.wallet.core.DappReading? = null,
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

    /** 089 S1: a `wallet_sendCalls`, handed over whole — the core reads EVERY call. */
    @Serializable
    @SerialName("resolve_batch")
    data class ResolveBatch(
        val params_json: String,
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
