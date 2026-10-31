package app.getvela.wallet.feature.wallet.core

import org.json.JSONObject

/**
 * r9-4: what a scanned (or pasted, or shared) text is FOR, read through the
 * core's link reading before anything treats it as a recipient. Every
 * scanner — Send's own and a recipient row's, the Quick Settings tile and
 * the launcher's Scan (both open Send's), the home's Scan, Explore's — asks
 * here first, so a multisig invite never lands in Send's "To" as text.
 *
 * - a multisig link — a transaction to approve (`#ms=`) or a multisig to add
 *   (`#msa=`), readable or damaged — on the web wallet's host, or carried by
 *   any other page's address, or as its bare fragment: the accounts
 *   machine's one link flow (the invite opens the add sheet, the proposal its
 *   sheet). WHAT it is, is the core's (`carriedLinkHash` → `isMultisigLink`),
 *   never a parser here; the address it came from is trusted for nothing (the
 *   invite's multisig is read from the chain, a proposal from the mailbox
 *   and the chain);
 * - a payment request (`https://…/pay?…`, `velawallet://pay?…`): the core's
 *   `payment_request`, as an App Link is;
 * - the outside signer's connect page: the browser (it is for another
 *   wallet's browser);
 * - anything else (an address, `ethereum:…`): Send's own reading
 *   (`scan_resolved`), which accepts or refuses it.
 */
object ScannedLink {
    sealed interface Route {
        /** The accounts machine's link flow, with the core's own hash (`#ms=…` / `#msa=…`). */
        data class Multisig(val hash: String) : Route

        /** A payment request, for the wallet route's `/pay` handling. */
        data class Pay(val link: String) : Route

        /** The outside signer's connect page: opened in the browser. */
        data class Connect(val url: String) : Route

        /** Not a link of ours: Send reads it. */
        data class Send(val text: String) : Route
    }

    fun route(raw: String, codec: AppLinks.LinkCodec = AppLinks.coreCodec): Route {
        val text = raw.trim()
        when (val claimed = AppLinks.classify(text, codec)) {
            is AppLinks.Route.Pay -> return Route.Pay(text)
            is AppLinks.Route.Web -> return when (claimed.kind) {
                AppLinks.WebKind.Connect -> Route.Connect(text)
                else -> multisigHash(text, codec)?.let(Route::Multisig) ?: Route.Send(text)
            }
            AppLinks.Route.Home, null -> Unit
        }
        multisigHash(text, codec)?.let { return Route.Multisig(it) }
        if (PayLink.parse(text) is PayLink.Pay) return Route.Pay(text)
        return Route.Send(text)
    }

    /**
     * The multisig link [text] carries, as the core reads it (`#ms=…`,
     * `#msa=…`, decoded and trimmed), or `null` when it is none: a whole
     * page address's fragment, else the text itself as a bare fragment. A
     * text with white space inside it is not one link.
     */
    fun multisigHash(text: String, codec: AppLinks.LinkCodec = AppLinks.coreCodec): String? {
        val trimmed = text.trim()
        if (trimmed.isEmpty() || trimmed.any(Char::isWhitespace)) return null
        val fragment = if (trimmed.contains('#')) trimmed.substringAfter('#') else trimmed
        val carried = runCatching {
            codec.call("carriedLinkHash", JSONObject().put("hash", "#$fragment").toString()).trim()
        }.getOrNull() ?: return null
        // The codec answers JSON: a string — empty when it is not a multisig link.
        val hash = runCatching { org.json.JSONTokener(carried).nextValue() as? String }.getOrNull().orEmpty()
        return hash.takeIf { it.startsWith("#") && it.length > 1 }
    }
}
