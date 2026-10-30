package app.getvela.wallet.core.platform

import app.getvela.wallet.feature.wallet.core.AppLinks
import app.getvela.wallet.feature.wallet.core.PayLink

/**
 * The ways into the wallet from outside the app (native parity Phase 3):
 * launcher shortcuts (Send, Receive, Scan), the Quick Settings Scan tile, and
 * text shared into Vela. Each arrives as an intent on MainActivity and is
 * read here — plain Kotlin, so every branch is tested on the JVM.
 *
 * Nothing here decides what money moves: an entry only opens a flow, and
 * shared text is handed to the core that already rules on the same text when
 * it is scanned (`payment_request`'s `LinkOpened` for a pay link, the send
 * machine's `scan_resolved` for an address or an `ethereum:` request).
 */
object LaunchEntry {
    /** The three flows a shortcut or the tile opens. */
    enum class Flow(val action: String, val shortcutId: String) {
        Send("app.getvela.wallet.action.SEND", "send"),
        Receive("app.getvela.wallet.action.RECEIVE", "receive"),
        Scan("app.getvela.wallet.action.SCAN", "scan"),
    }

    /** What an intent asks of the wallet, beyond opening it. */
    sealed interface Ask {
        data class Open(val flow: Flow) : Ask

        /** A `/pay` link, shared as text: validated by the core on the wallet route. */
        data class PayLinkShared(val url: String) : Ask

        /** An address or an `ethereum:` request, shared as text: Send's `scan_resolved` decides. */
        data class PaymentShared(val text: String) : Ask

        /**
         * A web-wallet link this build hands to the browser (a multisig or an
         * outside signer's connect link, read by the core's codec — [AppLinks]).
         */
        data class WebLinkShared(val url: String) : Ask
    }

    /** The flow a shortcut's or the tile's action names, or `null`. */
    fun flowOf(action: String?): Flow? = Flow.entries.firstOrNull { it.action == action }

    /**
     * What an intent asks: [action] and, for `ACTION_SEND` of `text/plain`,
     * its [sharedText]. `null` — nothing beyond opening the app.
     */
    fun read(action: String?, type: String?, sharedText: String?): Ask? {
        flowOf(action)?.let { return Ask.Open(it) }
        if (action == ACTION_SEND && type?.startsWith("text/") == true) return shared(sharedText)
        return null
    }

    /**
     * Text shared into Vela, as another app shares it — often a sentence
     * around the thing ("Pay me: https://getvela.app/pay?…"). The first token
     * that is ours wins: a pay link or a web-wallet link (read by the core's
     * codec), then an `ethereum:` request, then an address. Anything else
     * asks nothing (the app just opens): a word is never guessed into a
     * recipient.
     */
    fun shared(text: String?, codec: AppLinks.LinkCodec = AppLinks.coreCodec): Ask? {
        val tokens = text.orEmpty().split(WHITESPACE).map { it.trim(*TRIM) }.filter { it.isNotEmpty() }
        tokens.firstNotNullOfOrNull { token ->
            when (val route = AppLinks.classify(token, codec)) {
                is AppLinks.Route.Pay -> Ask.PayLinkShared(token)
                is AppLinks.Route.Web -> Ask.WebLinkShared(route.url)
                else -> (PayLink.parse(token) as? PayLink.Pay)?.let { Ask.PayLinkShared(token) }
            }
        }?.let { return it }
        tokens.firstOrNull { it.startsWith("ethereum:", ignoreCase = true) }?.let { return Ask.PaymentShared(it) }
        tokens.firstOrNull { ADDRESS.matches(it) }?.let { return Ask.PaymentShared(it) }
        return null
    }

    const val ACTION_SEND = "android.intent.action.SEND"

    private val WHITESPACE = Regex("\\s+")
    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

    /** Punctuation a sentence leaves stuck to a link or an address. */
    private val TRIM = charArrayOf('"', '\'', '(', ')', '<', '>', '[', ']', ',', ';', '.', '!', '。', '，', '！', '（', '）', '“', '”', '‘', '’', '「', '」')

}
