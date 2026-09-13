package app.getvela.wallet.feature.settings.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `display_currency` machine's wire types, in Kotlin.
 *
 * A transcription of `rust/crates/vela-core/src/app/display_currency.rs`, not a
 * parser: serde's `#[serde(tag = "type", rename_all = "snake_case")]` is exactly
 * a sealed hierarchy whose subclasses carry `@SerialName`. The generated mirrors
 * in `app-web/vela-wallet/src/lib/core/generated/Currency*.ts` are the reference,
 * and `CoreWireDriftTest` is what keeps this file honest when they change.
 *
 * This is the smallest of the twenty-four machines — four operations — which is
 * why it is the one that proves the road. Read it as the template.
 */

// -- what the screen renders -------------------------------------------------

/**
 * `CurrencyView`.
 *
 * **`rate` is nullable and must stay nullable.** `null` is not `1`: the core's
 * own comment spells out the consequence of confusing them — formatting may
 * degrade to the USD figure, converting may not, because a fiat amount
 * multiplied by a defaulted 1 is a real mispayment. Kotlin's type system is the
 * enforcement, and the drift test asserts the nullability survives.
 */
@Serializable
data class CurrencyView(
    val code: String,
    val rate: Double? = null,
    /** `false` ⇒ the USD placeholder is showing, not a settled choice. */
    val committed: Boolean = false,
)

// -- what the screen sends ---------------------------------------------------

/** `CurrencyEvent` — only the two variants a shell may raise. */
@Serializable
sealed class CurrencyEvent {
    /** Screen focus / first mount. A second one supersedes the first. */
    @Serializable
    @SerialName("refresh")
    data object Refresh : CurrencyEvent()

    /** An explicit pick in Settings. User choice wins over any in-flight seed. */
    @Serializable
    @SerialName("user_chose")
    data class UserChose(val code: String) : CurrencyEvent()
}

// -- what the core asks the shell to do --------------------------------------

/** `CurrencyOperation`. Sentences, not I/O — the shell decides *how*. */
@Serializable
sealed class CurrencyOperation {
    /** Read `vela.displayCurrency`. Absent ALWAYS means "never chose". */
    @Serializable
    @SerialName("read_stored_code")
    data object ReadStoredCode : CurrencyOperation()

    @Serializable
    @SerialName("write_stored_code")
    data class WriteStoredCode(val code: String) : CurrencyOperation()

    /**
     * The device region's currency, from the primary locale only.
     *
     * The one operation where Android is *more* capable than the web shell,
     * which answers `null` because a browser has no region. Answering `null`
     * here would cost a person their correct default currency.
     */
    @Serializable
    @SerialName("read_device_currency")
    data object ReadDeviceCurrency : CurrencyOperation()

    /** Price one currency: USD→code. `None` when nothing can price it now. */
    @Serializable
    @SerialName("resolve_rate")
    data class ResolveRate(val code: String) : CurrencyOperation()
}

// -- what the shell observed -------------------------------------------------

/** `CurrencyShellResult`. One variant per operation, and every one is owed. */
@Serializable
sealed class CurrencyShellResult {
    @Serializable
    @SerialName("stored_code")
    data class StoredCode(val code: String? = null) : CurrencyShellResult()

    @Serializable
    @SerialName("code_written")
    data object CodeWritten : CurrencyShellResult()

    @Serializable
    @SerialName("device_currency")
    data class DeviceCurrency(val code: String? = null) : CurrencyShellResult()

    @Serializable
    @SerialName("rate_resolved")
    data class RateResolved(val code: String, val rate: Double? = null) : CurrencyShellResult()
}
