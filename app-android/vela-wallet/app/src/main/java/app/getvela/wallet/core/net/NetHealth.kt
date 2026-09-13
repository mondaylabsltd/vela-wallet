package app.getvela.wallet.core.net

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Whether this device can reach a server at all — answered by what the calls
 * did, never by what the radio claims (the manifest's standing rule: no
 * ACCESS_NETWORK_STATE, because a connectivity check says "connected" behind a
 * captive portal and "no network" on a working VPN).
 *
 * Spec 047 FR-006: three consecutive calls that never reached a server — the
 * executor's `Network` outcome, a socket that could not open — mean offline;
 * the first call that is answered, with any status, means online again. A
 * timeout says nothing either way: a slow server is not a missing network.
 */
object NetHealth {
    private const val MISSES_BEFORE_OFFLINE = 3

    private val _online = MutableStateFlow(true)
    val online: StateFlow<Boolean> = _online

    private var misses = 0

    @Synchronized
    fun reached() {
        misses = 0
        if (!_online.value) {
            VelaLog.event("net", "reachable again")
            _online.value = true
        }
    }

    @Synchronized
    fun unreached() {
        misses += 1
        if (misses >= MISSES_BEFORE_OFFLINE && _online.value) {
            VelaLog.event("net", "offline", "misses" to misses)
            _online.value = false
        }
    }

    /** Tests: back to the fresh state. */
    @Synchronized
    fun reset() { misses = 0; _online.value = true }
}
