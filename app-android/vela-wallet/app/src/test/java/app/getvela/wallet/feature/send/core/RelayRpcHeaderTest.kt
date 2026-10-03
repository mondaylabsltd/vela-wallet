package app.getvela.wallet.feature.send.core

import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Spec 098 §5: a request to the relay names the chain's RPC under the name the
 * relay reads — key and all — and a request with nothing to name carries none.
 */
class RelayRpcHeaderTest {
    private fun built(url: String?) =
        Request.Builder().url("https://relay.example/v1/treasury/1337").relayRpcHeader(url).build()

    @Test
    fun namesTheChainsRpcUnderTheRelaysName() {
        val request = built("https://rpc.one/v2/KEY")
        assertEquals("https://rpc.one/v2/KEY", request.header("x-vela-rpc-url"))
        // Not the pre-081 name the relay never read.
        assertNull(request.header("X-Rpc-Url"))
    }

    @Test
    fun namesNothingWhenThereIsNothingToName() {
        assertNull(built(null).header(RELAY_RPC_URL_HEADER))
        assertNull(built("").header(RELAY_RPC_URL_HEADER))
    }
}
