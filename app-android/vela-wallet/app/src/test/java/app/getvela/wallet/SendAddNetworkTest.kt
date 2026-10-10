package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.send.core.SendAddNetwork
import app.getvela.wallet.feature.send.core.SendAddNetworkOutcome
import app.getvela.wallet.feature.settings.core.NetBoards
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import app.getvela.wallet.feature.settings.core.NetWizardView
import java.io.File
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.yield
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * PR 3 final notes F6 / F27: Send's "add this network" is answered by the
 * network machine's own verdict, the moment it has one. The port waited ten
 * seconds for the chain to turn up as added and called every other ending
 * "not found" when the ten seconds were up.
 *
 * The stops are views the REAL `network_admin` machine wrote (`NetBoards`).
 */
class SendAddNetworkTest {
    private val strings = I18nRuntime { tag ->
        File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"), "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("en") }

    private fun settled(view: NetView, chainId: Long = NetBoards.CHAIN_ID) = SendAddNetwork.settled(view, chainId) { strings.t(it) }

    @Test
    fun `each way the network machine stops is its own answer, in the core's sentence`() {
        // No such chain in the catalog.
        assertEquals(SendAddNetworkOutcome.NotFound, settled(NetBoards.view(NetBoards.Stop.NotFound)))

        // Refused — and WHY rides along, the core's own words: no verifier is
        // not "contracts missing", and neither is "not found".
        val noP256 = settled(NetBoards.view(NetBoards.Stop.ScanNoP256)) as SendAddNetworkOutcome.NotCompatible
        assertEquals(strings.t("settingsModals.addNetwork.noP256Hint"), noP256.detail)
        assertTrue(noP256.detail!!, noP256.detail!!.contains("0x100"))
        val missing = settled(NetBoards.view(NetBoards.Stop.ScanMissingContracts)) as SendAddNetworkOutcome.NotCompatible
        assertEquals(strings.t("settingsModals.addNetwork.incompatibleHint"), missing.detail)

        // A check that reached no verdict is said as that — never as a refusal's reason.
        val unverified = settled(NetBoards.view(NetBoards.Stop.ScanCheckFailed)) as SendAddNetworkOutcome.NotCompatible
        assertEquals("Unable to verify — RPC request failed", unverified.detail)
        // No RPC listed: not added, and the sentence says what is needed.
        val noRpc = settled(NetBoards.view(NetBoards.Stop.NoRpcEndpoint)) as SendAddNetworkOutcome.NotCompatible
        assertEquals(strings.t("settingsModals.addNetwork.noRpcEndpoint"), noRpc.detail)

        // Already in the list is the answer Send was after.
        assertEquals(SendAddNetworkOutcome.Added, settled(NetBoards.view(NetBoards.Stop.AlreadyAdded), chainId = 1))
    }

    @Test
    fun `added is added, and a machine still working has no answer yet`() {
        val row = NetNetworkRow(id = "zircuit", chain_id = NetBoards.CHAIN_ID, display_name = "Zircuit", native_symbol = "ETH", is_custom = true)
        // The scan path saves and clears the wizard in one move.
        assertEquals(SendAddNetworkOutcome.Added, settled(NetView(loaded = true, networks = listOf(row), last_added_chain_id = NetBoards.CHAIN_ID)))
        for (phase in listOf(NetWizardPhase.Resolving, NetWizardPhase.Checking, NetWizardPhase.Searching)) {
            assertNull("$phase", settled(NetView(loaded = true, wizard = NetWizardView(phase = phase))))
        }
        // The wizard reset under the attempt, nothing saved: an error, not a wait.
        assertEquals(SendAddNetworkOutcome.Error, settled(NetView(loaded = true)))
    }

    /**
     * No clock: the answer arrives with the commit that settles the attempt.
     * A refusal used to sit out the whole ten seconds.
     */
    @Test
    fun `the answer arrives with the machine's verdict, not after a wait`() = runBlocking<Unit> {
        val commits = MutableStateFlow(0L)
        var applied = false
        // A stale stop from an earlier attempt is on the machine when this one is asked.
        var view = NetBoards.view(NetBoards.Stop.NotFound)
        val started = System.nanoTime()
        val answer = async {
            SendAddNetwork.await(commits, applied = { applied }, view = { view }, chainId = NetBoards.CHAIN_ID) { strings.t(it) }
        }
        yield()
        // Commits from before the event are not this attempt's: the stale stop is not its answer.
        commits.value = 1
        yield()
        assertFalse("an earlier attempt's stop was taken for this one's", answer.isCompleted)
        // The event is applied and the machine is working.
        applied = true
        view = NetView(loaded = true, wizard = NetWizardView(phase = NetWizardPhase.Checking))
        commits.value = 2
        yield()
        assertFalse(answer.isCompleted)
        // Its verdict: refused, no P-256 verifier.
        view = NetBoards.view(NetBoards.Stop.ScanNoP256)
        commits.value = 3
        val outcome = withTimeout(5_000) { answer.await() } as SendAddNetworkOutcome.NotCompatible
        assertEquals(strings.t("settingsModals.addNetwork.noP256Hint"), outcome.detail)
        val took = (System.nanoTime() - started) / 1_000_000
        assertTrue("answered in $took ms — never a ten-second wait", took < 5_000)
    }
}
