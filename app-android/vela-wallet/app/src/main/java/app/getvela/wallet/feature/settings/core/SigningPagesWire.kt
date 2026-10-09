package app.getvela.wallet.feature.settings.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `signing_pages` machine's wire types (spec 102) — a transcription of
 * `rust/crates/vela-core/src/app/signing_pages.rs`. The web mirrors are
 * `app-web/vela-wallet/src/lib/core/generated/SigningPages*.ts`.
 *
 * Settings → Signing pages: the pages this device trusts to show and sign
 * requests, the official one always first. Which page an ACCOUNT signs on is
 * that account's own choice (`signingVenueChoices`), never this list's.
 */

/** A saved page, as stored and as `signingVenueChoices` takes it. */
@Serializable
data class SigningPage(val url: String, val name: String = "")

/** One row of Settings → Signing pages: the official page first, then the saved ones. */
@Serializable
data class SigningPageRow(
    val url: String,
    /** The person's label; empty ⇒ named by its host (or, for the official page, "Official"). */
    val name: String = "",
    /** The domain whose keys this page can use (R1) — drawn on the row. */
    val domain: String = "",
    val official: Boolean = false,
)

@Serializable
data class SigningPagesView(
    val pages: List<SigningPageRow> = listOf(
        SigningPageRow(url = "https://sign.getvela.app/", domain = "getvela.app", official = true),
    ),
    /** The saved pages as stored — what `signingVenueChoices` takes. */
    val saved: List<SigningPage> = emptyList(),
    /** `invalid` | `insecure` | `duplicate`: the last address was not added. */
    val add_error: String? = null,
    /** The list has been read; edits are offered only then. */
    val loaded: Boolean = false,
)

@Serializable
sealed class SigningPagesEvent {
    @Serializable
    @SerialName("refresh")
    data object Refresh : SigningPagesEvent()

    @Serializable
    @SerialName("page_added")
    data class PageAdded(val url: String, val name: String = "") : SigningPagesEvent()

    @Serializable
    @SerialName("page_renamed")
    data class PageRenamed(val url: String, val name: String) : SigningPagesEvent()

    @Serializable
    @SerialName("page_removed")
    data class PageRemoved(val url: String) : SigningPagesEvent()
}

@Serializable
sealed class SigningPagesOperation {
    /** Read `vela.signingPages` and the 071 `vela.trustedSignerUrl`, raw. */
    @Serializable
    @SerialName("read_stored")
    data object ReadStored : SigningPagesOperation()

    /** Persist the saved pages; with [remove_legacy_url], drop the imported 071 key too. */
    @Serializable
    @SerialName("write_pages")
    data class WritePages(
        val pages: List<SigningPage> = emptyList(),
        val remove_legacy_url: Boolean = false,
    ) : SigningPagesOperation()
}

/** Stored values go back RAW — whether they are usable is the core's call. */
@Serializable
sealed class SigningPagesShellResult {
    @Serializable
    @SerialName("stored")
    data class Stored(val pages_json: String? = null, val legacy_url: String? = null) : SigningPagesShellResult()

    @Serializable
    @SerialName("written")
    data object Written : SigningPagesShellResult()
}
