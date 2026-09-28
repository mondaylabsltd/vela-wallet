package app.getvela.wallet.core.net

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.netHealthStep

/**
 * Whether this device can reach a server at all — answered by what the calls
 * did, never by what the radio claims (the manifest's standing rule: no
 * ACCESS_NETWORK_STATE, because a connectivity check says "connected" behind a
 * captive portal and "no network" on a working VPN).
 *
 * The rule is the core's (`net_health_step`, spec 082 RE3): three calls in a
 * row that never reached a server are offline, and the first answered call is
 * the way back (`came_back`). This object only keeps the state the rule steps
 * and publishes it; the transport says which calls count (a timeout says
 * nothing either way: a slow server is not a missing network).
 */
object NetHealth {
    private val _online = MutableStateFlow(true)
    val online: StateFlow<Boolean> = _online

    private var misses = 0u

    /** A call was answered, with any status. */
    @Synchronized
    fun reached() = step(reached = true)

    /** A call never reached a server. */
    @Synchronized
    fun unreached() = step(reached = false)

    private fun step(reached: Boolean) {
        val next = netHealthStep(misses, _online.value, reached)
        misses = next.misses
        when (next.edge) {
            "went_offline" -> VelaLog.event("net", "offline", "misses" to next.misses.toInt())
            "came_back" -> VelaLog.event("net", "reachable again")
        }
        _online.value = next.online
    }

    /** Tests: back to the fresh state. */
    @Synchronized
    fun reset() { misses = 0u; _online.value = true }
}
