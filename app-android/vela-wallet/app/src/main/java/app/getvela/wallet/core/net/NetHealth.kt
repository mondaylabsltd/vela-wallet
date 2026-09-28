package app.getvela.wallet.core.net

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.NetHealthState
import uniffi.vela_core_uniffi.netHealthFresh
import uniffi.vela_core_uniffi.netHealthStep

/**
 * Whether this device can reach a server at all — answered by what the calls
 * did, never by what the radio claims (the manifest's standing rule: no
 * ACCESS_NETWORK_STATE, because a connectivity check says "connected" behind a
 * captive portal and "no network" on a working VPN).
 *
 * The rule is the core's (`net_health_step`, spec 082 RE3, RJ14): offline is
 * three misses in a row from at least two sources — the chains the calls
 * read — AND nothing reached for ten seconds; any answered call resets the
 * run, and the first one after offline is the way back (`came_back`). One
 * chain whose nodes are all down is that chain's notice, never "offline":
 * counting calls from any chain flapped offline / back every few seconds while
 * the other twenty-three answered (G53). This object only keeps the state the
 * rule steps, hands it back on the next call, and publishes `online`; the
 * transport says which calls count (a timeout says nothing either way: a slow
 * server is not a missing network).
 */
object NetHealth {
    private val _online = MutableStateFlow(true)
    val online: StateFlow<Boolean> = _online

    private var state: NetHealthState = netHealthFresh()

    /** The clock the quiet window is measured on (epoch ms); tests pin it. */
    @Volatile
    var clock: () -> Double = { System.currentTimeMillis().toDouble() }

    /** A call to [source] (the chain it read; `null` = none) was answered, with any status. */
    @Synchronized
    fun reached(source: Int?) = step(reached = true, source = source)

    /** A call to [source] never reached a server. */
    @Synchronized
    fun unreached(source: Int?) = step(reached = false, source = source)

    private fun step(reached: Boolean, source: Int?) {
        val next = netHealthStep(state, reached, source?.takeIf { it >= 0 }?.toUInt(), clock())
        state = next.state
        when (next.edge) {
            "went_offline" -> VelaLog.event(
                "net", "offline",
                "misses" to next.state.misses.toInt(),
                "sources" to (next.state.sources.size + next.state.unsourced.toInt()),
            )
            "came_back" -> VelaLog.event("net", "reachable again", "chain" to source)
        }
        _online.value = next.state.online
    }

    /** Tests: back to the fresh state and the real clock. */
    @Synchronized
    fun reset() {
        state = netHealthFresh()
        _online.value = true
        clock = { System.currentTimeMillis().toDouble() }
    }
}
