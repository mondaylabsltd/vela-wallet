package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.diagnostics.VelaLog
import java.math.BigInteger
import java.util.concurrent.ConcurrentHashMap
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * r14-9: an execution is signed only at the network fee the person was shown
 * when they pressed.
 *
 * The sheet's fee line is the machine's look for that executor. A look the
 * person pressed on after the machine's freshness window (`PREVIEW_FRESH_MS`,
 * 15 s) is taken again by Execute before any passkey, and that new quote is
 * the fee the operation carries (`send_calls.fee`) — so a fee line that had
 * stood a while said "≈0.098172" and 0.0972 was charged. Whatever moved it
 * (a re-quote at the press, the gas price), the fee signed must be the fee
 * said: the sheet records the figure on screen at the press ([pressed]); the
 * shell compares it with the operation's fee before anything is signed
 * ([refusal]). The same: on it goes. Different: nothing is signed, the machine
 * hears "not sent" with the new figure (`fee_moved`), and the sheet now shows
 * the new look — the next press, inside the window, signs exactly it.
 */
object ShownFee {
    private val pressed = ConcurrentHashMap<String, String>()

    private fun key(chainId: Long, executor: String) = "$chainId:${executor.lowercase()}"

    /**
     * The person pressed Execute (or Approve and execute) as [executor] with
     * [amount] (the look's `fee.amount`) on screen; `null`: no fee was said
     * (the look did not price it) — nothing to hold the operation to.
     */
    fun pressed(chainId: Int, executor: String, amount: String?) {
        val k = key(chainId.toLong(), executor)
        if (amount == null) pressed.remove(k) else pressed[k] = amount
    }

    /**
     * Before an execution's operation is signed: `null` to go on, or the
     * answer that stops it — its fee is not the one the person pressed on.
     * The figure pressed on is used once.
     */
    fun refusal(op: MultisigOperation.SendCalls): MultisigShellResult? {
        if (op.purpose?.get("kind").let { (it as? JsonPrimitive)?.content } != "execute") return null
        val shown = pressed.remove(key(op.chain_id, op.from)) ?: return null
        val signing = (op.fee as? JsonObject)?.get("amount") ?: return null
        if (same(shown, signing)) return null
        VelaLog.event("multisig.execute", "fee moved since the press: nothing signed", "chain" to op.chain_id, "shown" to shown, "now" to amountOf(signing)?.toString())
        return MultisigShellResult.CallsFailed(
            MultisigShellFailure.FeeMoved(amount = amountOf(signing)?.toString() ?: "0", chainId = op.chain_id, executor = op.from).detail,
            maybe_sent = false,
        )
    }

    /** Two amounts as the machine writes them (decimal or `0x`), compared as numbers. */
    fun same(shown: String, signing: JsonElement): Boolean {
        val a = amountOf(JsonPrimitive(shown)) ?: return false
        val b = amountOf(signing) ?: return false
        return a == b
    }

    fun amountOf(value: JsonElement?): BigInteger? {
        val text = (value as? JsonPrimitive)?.content?.trim() ?: return null
        return runCatching {
            if (text.startsWith("0x") || text.startsWith("0X")) BigInteger(text.substring(2).ifEmpty { "0" }, 16) else BigInteger(text)
        }.getOrNull()
    }

    /** Tests: nothing pressed. */
    internal fun forget() = pressed.clear()
}
