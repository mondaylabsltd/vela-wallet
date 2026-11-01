package app.getvela.wallet

import app.getvela.wallet.feature.multisig.MultisigWiring
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Real-device pass 2026-11-01: a phone pointed at its own multisig service
 * (Settings → endpoints, or the harness's `vela.serviceEndpoints` launch input)
 * created its multisig on Vela's service, because the network machine's
 * not-yet-loaded DEFAULT was taken before the store was read.
 */
class MultisigServiceUrlTest {

    private val stored = """{"bundlerServiceURL":"http://127.0.0.1:9590","genesisServiceURL":"http://127.0.0.1:9587/"}"""

    @Test
    fun theStoredServiceIsUsedWhenSettingsHoldsNoneYet() {
        assertEquals("http://127.0.0.1:9587", MultisigWiring.resolveServiceUrl(fromSettings = null, storedEndpoints = stored))
        assertEquals("http://127.0.0.1:9587", MultisigWiring.resolveServiceUrl(fromSettings = "  ", storedEndpoints = stored))
    }

    @Test
    fun settingsOwnValueWins() {
        assertEquals(
            "https://my.service.example",
            MultisigWiring.resolveServiceUrl(fromSettings = "https://my.service.example/", storedEndpoints = stored),
        )
    }

    @Test
    fun velasServiceOnlyWhenNothingElseIsSet() {
        assertEquals(MultisigWiring.DEFAULT_SERVICE, MultisigWiring.resolveServiceUrl(null, null))
        assertEquals(MultisigWiring.DEFAULT_SERVICE, MultisigWiring.resolveServiceUrl(null, """{"bundlerServiceURL":"http://x"}"""))
        assertEquals(MultisigWiring.DEFAULT_SERVICE, MultisigWiring.resolveServiceUrl(null, "not json"))
    }
}
