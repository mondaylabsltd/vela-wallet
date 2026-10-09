package app.getvela.wallet.core.crux

import org.json.JSONObject

/**
 * One machine driven by hand, at once, on the calling thread — for a gallery
 * board whose view the real core wrote, never for a live surface (those run on
 * [CoreDriver]).
 *
 * Every effect the core asks for is offered to [answer]: a result JSON
 * answers it now, `null` leaves it out — a timer, a read the board wants
 * still running. [resolve] answers one left out, later. So a board says
 * "the account read came back unreachable, and the core's re-ask is out"
 * by answering exactly those two, and draws what the core then says.
 */
class CoreScript(
    private val bridge: CoreBridge,
    private val answer: (operation: JSONObject) -> String?,
) {
    private val pending = ArrayList<Pair<ULong, JSONObject>>()

    /** Send one event (its JSON), then answer what [answer] will. */
    fun dispatch(eventJson: String): CoreScript {
        take(JSONObject(bridge.dispatch(eventJson)))
        drain()
        return this
    }

    /** Answer the first effect left out whose operation is a `type`, with [resultJson]; `false` when none is. */
    fun resolve(type: String, resultJson: String): Boolean {
        val index = pending.indexOfFirst { it.second.optString("type") == type }
        if (index < 0) return false
        val (id, _) = pending.removeAt(index)
        take(JSONObject(bridge.resolveEffect(id, resultJson)))
        drain()
        return true
    }

    /** The operations the core is still waiting on, by `type`. */
    fun waiting(): List<String> = pending.map { it.second.optString("type") }

    /** The view as the core wrote it now. */
    fun viewJson(): String = bridge.view()

    private fun take(result: JSONObject) {
        result.optJSONArray("cancelled_effect_ids")?.let { cancelled ->
            val ids = (0 until cancelled.length()).map { cancelled.optLong(it).toULong() }.toSet()
            pending.removeAll { it.first in ids }
        }
        val effects = result.optJSONArray("effects") ?: return
        for (i in 0 until effects.length()) {
            val effect = effects.optJSONObject(i) ?: continue
            val operation = effect.optJSONObject("operation") ?: continue
            pending += effect.optLong("id").toULong() to operation
        }
    }

    private fun drain() {
        while (true) {
            val next = pending.firstNotNullOfOrNull { (id, operation) -> answer(operation)?.let { Triple(id, operation, it) } } ?: return
            pending.removeAll { it.first == next.first }
            take(JSONObject(bridge.resolveEffect(next.first, next.third)))
        }
    }
}
