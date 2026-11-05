package app.getvela.wallet.core.format

import org.json.JSONObject
import uniffi.vela_core_uniffi.checksumAddress
import uniffi.vela_core_uniffi.multisigLinkCall

/**
 * THE short form of an address, on every surface of this app — the header,
 * Settings, Activity, the confirm screen, a sheet, a list: the core's
 * `names.short` (r11-6, r12-7), its EIP-55 case, the first eight and last six
 * characters (`0x88cCA0…266894`).
 *
 * Six and six is the core's decision, not this shell's: a 4+4 look-alike is a
 * 2^32 search for an address-poisoning sender, 6+6 a 2^48 one, and the short
 * form is what a person compares. Every Android place that shortens an
 * address calls this; none keeps its own `take(6)…takeLast(4)` (round 12: the
 * header, Settings, Activity and the confirm still said `0x88cC…6894`, and
 * Activity a lowercase `0x3e9b…692d` beside the sheet's EIP-55 one).
 */
object ShortAddress {
    /** `address` shortened by the core; if the core cannot be asked, the same rule over its EIP-55 form. */
    fun of(address: String): String =
        runCatching {
            val answer = multisigLinkCall("names.short", JSONObject().put("address", address).toString())
            JSONObject("{\"v\":$answer}").optString("v").takeIf { it.isNotEmpty() }
        }.getOrNull() ?: fallback(address)

    /** The core's rule, said again for a build whose core cannot answer (never a shorter form). */
    private fun fallback(address: String): String {
        val full = runCatching { checksumAddress(address) }.getOrDefault(address)
        return if (full.length <= 14) full else "${full.take(8)}…${full.takeLast(6)}"
    }
}
