package app.getvela.wallet

import app.getvela.wallet.dev.ServiceLaunchInputs
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Native parity F0.5 (debug builds only): the harness's endpoints and RPCs
 * from the launch line, MERGED into what the device holds — an endpoint not
 * named keeps its value, a network override keeps everything but its RPC.
 */
class ServiceLaunchInputsTest {

    @Test
    fun `named endpoints are laid over the stored ones, in the stored spelling`() {
        val merged = JSONObject(
            ServiceLaunchInputs.mergeEndpoints(
                """{"passkeyIndexURL":"https://index.example","bundlerServiceURL":"https://relay.example"}""",
                """{"bundlerServiceURL":"http://127.0.0.1:8990","genesis_service_url":"http://127.0.0.1:8987"}""",
            ),
        )
        assertEquals("https://index.example", merged.getString("passkeyIndexURL"))
        assertEquals("http://127.0.0.1:8990", merged.getString("bundlerServiceURL"))
        assertEquals("http://127.0.0.1:8987", merged.getString("genesisServiceURL"))
        assertFalse(merged.has("genesis_service_url"))
    }

    @Test
    fun `a name that is not an endpoint is refused, not stored`() {
        val thrown = runCatching { ServiceLaunchInputs.mergeEndpoints(null, """{"rpcURL":"x"}""") }.exceptionOrNull()
        assertTrue(thrown is IllegalArgumentException)
    }

    @Test
    fun `an RPC override keeps the network's other settings and adds a missing network`() {
        val merged = JSONArray(
            ServiceLaunchInputs.mergeRpcOverrides(
                """[{"chainId":100,"rpcURL":"https://gnosis.example","explorerURL":"https://scan","bundlerURL":"","onlyMine":true}]""",
                """{"100":"http://127.0.0.1:8945","8453":"http://127.0.0.1:8946"}""",
            ),
        )
        assertEquals(2, merged.length())
        val gnosis = merged.getJSONObject(0)
        assertEquals("http://127.0.0.1:8945", gnosis.getString("rpcURL"))
        assertEquals("https://scan", gnosis.getString("explorerURL"))
        assertTrue(gnosis.getBoolean("onlyMine"))
        val base = merged.getJSONObject(1)
        assertEquals(8453, base.getInt("chainId"))
        assertEquals("", base.getString("explorerURL"))
    }

    @Test
    fun `the list form reads as the web harness writes it`() {
        val merged = JSONArray(ServiceLaunchInputs.mergeRpcOverrides(null, """[{"chainId":100,"rpcURL":"http://127.0.0.1:8945"}]"""))
        assertEquals("http://127.0.0.1:8945", merged.getJSONObject(0).getString("rpcURL"))
    }

    @Test
    fun `the launch inputs are debug-only extras`() {
        for (name in listOf(ServiceLaunchInputs.SERVICE_ENDPOINTS, ServiceLaunchInputs.RPC_OVERRIDES, ServiceLaunchInputs.PEERCONNECT_RELAY)) {
            assertTrue(name, name in LaunchExtras.DEBUG_ONLY)
            assertEquals(null, LaunchExtras.honoured(name, "x", debug = false))
        }
    }
}
