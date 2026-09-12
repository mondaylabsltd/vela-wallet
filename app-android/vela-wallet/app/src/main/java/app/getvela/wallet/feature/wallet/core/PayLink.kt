package app.getvela.wallet.feature.wallet.core

import java.net.URI
import java.net.URLDecoder

/**
 * A deep link's shape (spec 047 D8), tokenized only: `velawallet://pay?…`,
 * `https://wallet.getvela.app/pay?…` and `https://getvela.app/pay?…` carry
 * the `/pay` query the core validates (`LinkOpened`); `velawallet://open?url=`
 * names a page for the in-app browser. Everything else is `null`. Plain
 * `java.net.URI` so the rule is testable on the JVM.
 */
sealed class PayLink {
    data class Pay(val to: String?, val chain: String?, val token: String?, val amount: String?, val sym: String?, val dec: String?, val net: String?) : PayLink()
    data class Open(val url: String) : PayLink()

    companion object {
        fun parse(raw: String?): PayLink? {
            val uri = runCatching { URI(raw?.trim().orEmpty()) }.getOrNull() ?: return null
            val scheme = uri.scheme?.lowercase() ?: return null
            val host = uri.host?.lowercase() ?: uri.authority?.lowercase().orEmpty()
            val path = uri.path.orEmpty().trimEnd('/')
            val query = queryOf(uri.rawQuery)
            return when {
                scheme == "velawallet" && host == "pay" -> pay(query)
                scheme == "velawallet" && host == "open" -> query["url"]?.takeIf { it.startsWith("http://") || it.startsWith("https://") }?.let(::Open)
                scheme == "https" && (host == "wallet.getvela.app" || host == "getvela.app") && path == "/pay" -> pay(query)
                else -> null
            }
        }

        private fun pay(q: Map<String, String>): Pay =
            Pay(to = q["to"], chain = q["chain"], token = q["token"], amount = q["amount"], sym = q["sym"], dec = q["dec"], net = q["net"])

        internal fun queryOf(raw: String?): Map<String, String> {
            if (raw.isNullOrEmpty()) return emptyMap()
            val out = LinkedHashMap<String, String>()
            raw.split('&').forEach { pair ->
                if (pair.isEmpty()) return@forEach
                val eq = pair.indexOf('=')
                val key = if (eq >= 0) pair.substring(0, eq) else pair
                val value = if (eq >= 0) pair.substring(eq + 1) else ""
                out[decode(key)] = decode(value)
            }
            return out
        }

        private fun decode(s: String): String = runCatching { URLDecoder.decode(s.replace("+", "%2B"), "UTF-8") }.getOrDefault(s)
    }
}
