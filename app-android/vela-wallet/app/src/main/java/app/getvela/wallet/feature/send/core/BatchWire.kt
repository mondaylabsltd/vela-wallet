package app.getvela.wallet.feature.send.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `batch_import` machine's wire (spec 045): the payroll batch the send
 * machine seeds its split rows from. Numerics from the Rust structs:
 * `decimals`, `line`, `rejected`, `recipient_count`, `max_recipients` are
 * `u32` (Int); `price_usd` and `rate` are `f64` (Double).
 */

@Serializable
data class BatchToken(
    val symbol: String,
    val decimals: Int,
    val balance: String,
    val price_usd: Double? = null,
)

@Serializable
enum class BatchUnit {
    @SerialName("fiat") Fiat,
    @SerialName("token") Token,
}

@Serializable
enum class BatchRateStatus {
    @SerialName("loading") Loading,
    @SerialName("ok") Ok,
    @SerialName("failed") Failed,
}

@Serializable
data class BatchPreviewRow(
    val line: Int,
    val name: String? = null,
    val address: String,
    val valid: Boolean = false,
    val dup: Boolean = false,
    val raw_amount: String = "",
    val token_amount: String = "",
    val ok: Boolean = false,
)

/** Why the parser refused a line (`BatchParseReason`). */
@Serializable
enum class BatchParseReason {
    @SerialName("no_address") NoAddress,
    @SerialName("no_amount") NoAmount,
}

/** One line the parser refused, as written (`BatchParseError`); `line` is `u32`. */
@Serializable
data class BatchParseError(
    val line: Int,
    val raw: String = "",
    val reason: BatchParseReason,
)

@Serializable
data class BatchRecipient(
    val address: String,
    val amount: String,
    val name: String? = null,
)

@Serializable
data class BatchView(
    val opened: Boolean = false,
    val unit: BatchUnit = BatchUnit.Fiat,
    val fiat_code: String = "",
    val raw_text: String = "",
    val file_name: String? = null,
    val busy: Boolean = false,
    val file_error: Boolean = false,
    val template_saved: Boolean = false,
    val priced: Boolean = false,
    val rate_status: BatchRateStatus = BatchRateStatus.Loading,
    val rate_input: String = "",
    val rate_edited: Boolean = false,
    val preview: List<BatchPreviewRow> = emptyList(),
    /** The lines the parser refused, each with its reason — in sheet order with `preview`. */
    val errors: List<BatchParseError> = emptyList(),
    val over_cap: Boolean = false,
    val rejected: Int = 0,
    val recipient_count: Int = 0,
    val total_token: String = "",
    val total_fiat: String? = null,
    val over_balance: Boolean = false,
    val can_apply: Boolean = false,
    val recipients: List<BatchRecipient> = emptyList(),
    val applied: Boolean = false,
)

@Serializable
sealed class BatchEvent {
    @Serializable
    @SerialName("open")
    data class Open(val token: BatchToken, val currency_code: String, val max_recipients: Int) : BatchEvent()

    @Serializable
    @SerialName("set_unit")
    data class SetUnit(val unit: BatchUnit) : BatchEvent()

    @Serializable
    @SerialName("set_fiat_code")
    data class SetFiatCode(val code: String) : BatchEvent()

    @Serializable
    @SerialName("set_raw_text")
    data class SetRawText(val text: String) : BatchEvent()

    @Serializable
    @SerialName("pick_file_requested")
    data object PickFileRequested : BatchEvent()

    @Serializable
    @SerialName("save_template_requested")
    data object SaveTemplateRequested : BatchEvent()

    @Serializable
    @SerialName("edit_rate")
    data class EditRate(val text: String) : BatchEvent()

    @Serializable
    @SerialName("reset_rate_to_auto")
    data object ResetRateToAuto : BatchEvent()

    @Serializable
    @SerialName("apply")
    data object Apply : BatchEvent()
}

@Serializable
sealed class BatchOperation {
    @Serializable
    @SerialName("fetch_usd_fiat_rate")
    data class FetchUsdFiatRate(val code: String) : BatchOperation()

    @Serializable
    @SerialName("pick_file")
    data object PickFile : BatchOperation()

    @Serializable
    @SerialName("save_template_file")
    data class SaveTemplateFile(val name: String, val contents: String, val mime: String) : BatchOperation()
}

@Serializable
sealed class BatchFileContent {
    @Serializable
    @SerialName("text")
    data class Text(val text: String) : BatchFileContent()

    @Serializable
    @SerialName("matrix")
    data class Matrix(val rows: List<List<String>>) : BatchFileContent()
}

@Serializable
sealed class BatchShellResult {
    @Serializable
    @SerialName("rate_resolved")
    data class RateResolved(val code: String, val rate: Double? = null) : BatchShellResult()

    @Serializable
    @SerialName("file_picked")
    data class FilePicked(val name: String, val content: BatchFileContent) : BatchShellResult()

    @Serializable
    @SerialName("file_pick_cancelled")
    data object FilePickCancelled : BatchShellResult()

    @Serializable
    @SerialName("file_pick_failed")
    data object FilePickFailed : BatchShellResult()

    @Serializable
    @SerialName("template_saved")
    data object TemplateSaved : BatchShellResult()

    @Serializable
    @SerialName("template_save_failed")
    data object TemplateSaveFailed : BatchShellResult()
}

/** The core's cap (`BATCH_MAX_RECIPIENTS`): the importer trims to it. */
const val BATCH_MAX_RECIPIENTS = 60
