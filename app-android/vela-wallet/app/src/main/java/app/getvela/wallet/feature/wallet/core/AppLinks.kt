package app.getvela.wallet.feature.wallet.core

import java.net.URI
import org.json.JSONObject

/**
 * The web wallet's links, wherever they reach the app (native parity Phase 3,
 * owner decision 2 of 2026-10-30 as narrowed by the coordinator). The manifest
 * claims, with `autoVerify`:
 *
 * - `https://(wallet.)getvela.app/pay?…` — a payment request ([PayLink]);
 * - on Android 15+ only (the fragment is matchable there), a multisig
 *   transaction to approve `https://wallet.getvela.app/{locale}/wallet#ms=…`
 *   and a multisig to add `#msa=…` — also on the front page `/{locale}#…`
 *   (web `multisig/core/link.ts`).
 *
 * The outside signer's page `/{locale}/connect#…` (web
 * `multisig/external/connect-link.ts`) is NOT claimed: it is for members who
 * sign with another wallet. One that reaches the app anyway — shared into it,
 * scanned in Explore — is still read here and offered in the browser.
 *
 * The fragment is read with the core's own link codec (`multisig_link_call`,
 * the wasm `multisigLinkCall`), never a second parser.
 *
 * Multisig is not in the phone app yet (audit §0.1: native M1–M6 are owed), so
 * a multisig or connect link is handed to the person's browser — the web
 * wallet, which opens it exactly as before — rather than swallowed. The
 * connect page is a page for ANOTHER wallet's browser anyway (MetaMask, Rabby…).
 * When the native multisig screens land, [Route.Web]'s proposal and invite
 * kinds are where they plug in.
 */
object AppLinks {
    /** The hosts the manifest claims; each serves `/.well-known/assetlinks.json`. */
    val HOSTS: Set<String> = setOf("wallet.getvela.app", "getvela.app")

    /** A locale segment as the web routes it: `en`, `zh`, `zh-TW`, `es-MX`, `pt-BR`… */
    private val LOCALE = Regex("^[a-z]{2}(-[A-Za-z]{2})?$")

    enum class WebKind {
        /** `#ms=` that reads: a transaction to approve. */
        Proposal,

        /** `#msa=` that reads: a multisig to add. */
        Invite,

        /** A multisig link that does not read — the web page says how it is damaged. */
        Broken,

        /** The outside signer's connect page. */
        Connect,
    }

    sealed interface Route {
        /** A payment request: validated by the core's `payment_request` (`LinkOpened`) on the wallet route. */
        data class Pay(val link: PayLink.Pay) : Route

        /** A link this build hands to the web wallet in the browser, untouched. */
        data class Web(val url: String, val kind: WebKind) : Route

        /** The wallet page with nothing to act on: opening the app is the whole answer. */
        data object Home : Route
    }

    /** The core's link codec: method + JSON arguments → JSON answer. */
    fun interface LinkCodec {
        fun call(method: String, argsJson: String): String
    }

    /** The real codec — the core over uniffi. */
    val coreCodec = LinkCodec { method, args -> uniffi.vela_core_uniffi.multisigLinkCall(method, args) }

    /**
     * What an https link on a claimed host asks for, or `null` when it is not
     * one of ours (another host, scheme or path) — the caller then routes it
     * as before ([PayLink] covers `velawallet://`).
     */
    fun classify(raw: String?, codec: LinkCodec = coreCodec): Route? {
        val text = raw?.trim().orEmpty()
        val uri = runCatching { URI(text) }.getOrNull() ?: return null
        if (uri.scheme?.lowercase() != "https") return null
        val host = uri.host?.lowercase() ?: return null
        if (host !in HOSTS) return null
        val segments = uri.rawPath.orEmpty().split('/').filter { it.isNotEmpty() }
        if (segments == listOf("pay")) return (PayLink.parse(text) as? PayLink.Pay)?.let(Route::Pay)
        // Multisig and connect links are the web wallet's, on its own host.
        if (host != "wallet.getvela.app") return null
        if (segments.isEmpty() || segments.size > 2 || !LOCALE.matches(segments[0])) return null
        val hash = uri.rawFragment.orEmpty()
        // The front page carries a multisig link too (web `keepLink`); nothing else of it is ours.
        if (segments.size == 1) return walletLink(text, hash, codec).takeIf { it is Route.Web }
        return when (segments[1]) {
            "connect" -> Route.Web(text, WebKind.Connect)
            "wallet" -> walletLink(text, hash, codec)
            else -> null
        }
    }

    private fun walletLink(url: String, hash: String, codec: LinkCodec): Route {
        val fragment = "#$hash"
        if (!answer(codec, "isMultisigLink", fragment).let { it == "true" }) return Route.Home
        val kind = when {
            answer(codec, "parseProposalLink", fragment).isPresent() -> WebKind.Proposal
            answer(codec, "parseAddLink", fragment).isPresent() -> WebKind.Invite
            else -> WebKind.Broken
        }
        return Route.Web(url, kind)
    }

    private fun String?.isPresent(): Boolean = this != null && this != "null"

    /** The codec's answer, or `null` when it refused (a link the core cannot read is a broken one). */
    private fun answer(codec: LinkCodec, method: String, hash: String): String? =
        runCatching { codec.call(method, JSONObject().put("hash", hash).toString()).trim() }.getOrNull()
}
