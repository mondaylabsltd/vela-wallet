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

    /** The answer to [ClearOperation.RpcGetCode] (cs-13). */
    @SerialName("code") Code,

    /** `name()` of an NFT collection a `setApprovalForAll` grants. */
    @SerialName("name") Name,
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
    /** cs-14: who put this line's label on the sheet. */
    val source: ClearLineSource = ClearLineSource.Vela,
    /** The address is the signing account itself ("This account"). */
    val this_account: Boolean = false,
    /** cs-4: the token's name is only its own contract's word. */
    val token_named_by_contract: Boolean = false,
)

/**
 * Who put one line on the sheet — what is CERTAIN and what is INTERPRETED
 * (clear-signing audit, cs-14; the core's `ClearLineSource`). Only [Vela]
 * is a reading of the bytes themselves.
 */
@Serializable
enum class ClearLineSource {
    @SerialName("vela") Vela,
    @SerialName("built_in") BuiltIn,
    @SerialName("fetched") Fetched,
    @SerialName("selector_db") SelectorDb,
    @SerialName("shape") Shape,
}

/**
 * How a reading stands, line by line (cs-1…cs-19; the core's
 * `ClearReading`): the facts a sheet needs to keep what is certain apart
 * from what is interpreted. Graded once, when the reading concludes.
 */
@Serializable
data class ClearReading(
    val source: ClearLineSource = ClearLineSource.Vela,
    val descriptor_owner: String? = null,
    val descriptor_path: String? = null,
    val contract_named_by: ClearLineSource? = null,
    /** Words a description put on this request that Vela's own decode contradicts. */
    val contradictions: List<String> = emptyList(),
    /** The public 4-byte database's name for this selector — never a headline. */
    val selector_name: String? = null,
    /** An output or recipient is an address that is not the signing account. */
    val goes_elsewhere: Boolean = false,
    /** A minimum output of dust: the call accepts almost nothing back. */
    val dust_minimum: Boolean = false,
    val allowance: Boolean = false,
    val allowance_adds: Boolean = false,
    val shape_intent: String? = null,
    val collection_name: String? = null,
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
    /** Where every part of this reading came from, and what the certain decode says against it (cs-1…cs-19). */
    val reading: ClearReading = ClearReading(),
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

/**
 * A stretch of a drawn message line: text as written, or a hidden
 * character's visible marker (`⟨U+202E⟩`, [marker]), which the sheet draws
 * muted — the core's `ClearTextRun`.
 */
@Serializable
data class ClearTextRun(val text: String, val marker: Boolean = false)

/** One drawn line of a message (`ClearMessageView.display_lines`) — the core's `ClearMessageLine`. */
@Serializable
sealed class ClearMessageLine {
    @Serializable
    @SerialName("text")
    data class Text(val runs: List<ClearTextRun> = emptyList()) : ClearMessageLine()

    /** [count] empty lines, folded so nothing can sit unseen below a screenful of nothing. */
    @Serializable
    @SerialName("blank")
    data class Blank(val count: Int) : ClearMessageLine()
}

@Serializable
data class ClearMessageView(
    /** The message carries invisible characters. */
    val has_invisible_chars: Boolean = false,
    val payload: String,
    val is_hex: Boolean = false,
    val decoded_text: String? = null,
    val binary_preview: String? = null,
    val non_printable: Boolean = false,
    val siwe: ClearSiweFields? = null,
    val binding: ClearSiweBinding? = null,
    val danger_class: ClearDangerClass = ClearDangerClass.Plain,
    /**
     * What the sheet DRAWS for the message (cs-10, cs-11): the readable text
     * — else the binary preview, else the payload — one entry a line, every
     * hidden or direction-changing character a visible `⟨U+…⟩` marker, and
     * each long run of empty lines folded into one [ClearMessageLine.Blank]
     * that counts them. [payload] is what is signed, untouched.
     */
    val display_lines: List<ClearMessageLine> = emptyList(),
)

@Serializable
data class ClearSigningView(
    /** cs-13: the target has no code — a name or descriptor is not applied to it. */
    val no_code: Boolean = false,
    /** The reading could not be made: the person may sign anyway, said as that. */
    val sign_anyway: Boolean = false,
    /** The request carries invisible characters (a disguised look-alike). */
    val has_invisible_chars: Boolean = false,
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

    /**
     * cs-13: does the target have code? Asked only when the request said
     * `probe_code`, before any name or descriptor is applied to it. Answered
     * [ClearShellResult.RpcAnswer] with `probe = code`, `to = address`, the
     * raw code (`null` when unreachable).
     */
    @Serializable
    @SerialName("rpc_get_code")
    data class RpcGetCode(val chain_id: Int, val address: String) : ClearOperation()
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
        /** The signing account — so a line can say "this account" (cs-2). */
        val from: String? = null,
        /** The core may ask for the target's code ([ClearOperation.RpcGetCode], cs-13): this shell answers it. */
        val probe_code: Boolean = false,
    ) : ClearSigningEvent()

    /** 089 S1: a `wallet_sendCalls`, handed over whole — the core reads EVERY call. */
    @Serializable
    @SerialName("resolve_batch")
    data class ResolveBatch(
        val params_json: String,
        val chain_id: Int,
        val locale: ClearLocale = ClearLocale(),
        val from: String? = null,
        val probe_code: Boolean = false,
    ) : ClearSigningEvent()

    @Serializable
    @SerialName("resolve_typed_data")
    data class ResolveTypedData(
        val typed_data_json: String,
        val chain_id: Int,
        val locale: ClearLocale = ClearLocale(),
        val from: String? = null,
    ) : ClearSigningEvent()

    @Serializable
    @SerialName("message_presented")
    data class MessagePresented(val method: ClearSignMethod, val params: List<String>, val request_origin: String? = null) : ClearSigningEvent()

    @Serializable
    @SerialName("cleared")
    data object Cleared : ClearSigningEvent()
}
