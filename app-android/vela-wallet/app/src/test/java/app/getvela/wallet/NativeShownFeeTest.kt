package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalItem
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * §24 "Natives carry the fee shown" (docs/multisig-machines.md): every press
 * that executes — Execute and the one press "Approve and execute" — carries
 * the figure the sheet drew (`shown_fee`), so a quote that landed between the
 * paint and the press signs nothing: the core stops with `fee_moved`, said
 * beside the pressed button once, and the next press signs the new figure.
 *
 * Through the REAL proposals machine (the host core over uniffi) and
 * [MultisigWorld]: the model the sheet painted builds the press.
 */
class NativeShownFeeTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val code = File(repoRoot, "rust/crates/vela-core/tests/vectors/safe-code")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = emptyMap(),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"

    private suspend fun <T : Any> poll(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T =
        withTimeoutOrNull(timeoutMs) {
            var found = pick()
            while (found == null) {
                delay(25)
                found = pick()
            }
            found
        } ?: error("never came: $what")

    private class Setup(val world: MultisigWorld, val hub: MultisigHub, val owners: List<String>, val hash: String)

    /** A 2-of-3 deployed on Gnosis, members 0 and 1 here; a transfer proposed (and approved) by member 0. */
    private fun withProposed(body: suspend (Setup) -> Unit) {
        val world = MultisigWorld { name -> File(code, "$name.hex").readText().trim() }
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x5f")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val hub = MultisigHub(
                live = world,
                facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
                scope = scope,
            )
            try {
                runBlocking {
                    withTimeout(120_000) {
                        hub.boot()
                        hub.activate(safe)
                        poll("followed") { hub.proposalsView.value.followed }
                        val transfer = mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to bob, "amount" to "1000")))
                        hub.proposals.request(ev("draft", "chain_id" to 100, "proposal" to transfer)) { v -> v["draft"] as? JsonObject }
                        poll("the draft's review read") { hub.proposalsView.value.draft?.takeIf { it.reviewState.kind == "ready" } }
                        val proposed = hub.proposals.request(ev("confirm_draft", "as_member" to owners[0], "anyway_confirmed" to hub.proposalsView.value.draft?.let { app.getvela.wallet.feature.multisig.draftDoor(it, refuseBlind = false).anyway })) { v ->
                            (v["actions"] as? JsonObject)?.get("propose")?.takeIf { it.kind != "working" }
                        }
                        assertEquals("proposed: $proposed", "done", proposed.kind)
                        val hash = (proposed["safe_tx_hash"].str ?: proposed["safeTxHash"].str)!!
                        poll("listed") { item(hub, hash) }
                        body(Setup(world, hub, owners, hash))
                    }
                }
            } finally {
                hub.dispose()
            }
        } finally {
            scope.cancel()
        }
    }

    private fun item(hub: MultisigHub, hash: String): MsProposalItem? =
        hub.proposalsView.value.items.firstOrNull { it.safeTxHash.equals(hash, ignoreCase = true) }

    /** The sheet as painted now. */
    private fun painted(s: Setup) = MsProposalModel(item(s.hub, s.hash)!!, s.hub.proposalsView.value, words, null)

    private fun look(s: Setup, executor: String): JsonElement? = item(s.hub, s.hash)?.let { s.hub.proposalsView.value.preview(it, executor) }

    private fun executeState(s: Setup): JsonElement? = s.hub.proposalsView.value.action("execute:${s.hash}")

    /**
     * The sheet painted [executor]'s look priced at 0x1; then the relay's
     * price moved and a new look (0x2, fresh) landed before the press —
     * never drawn. Returns the model as painted.
     */
    private suspend fun aQuoteLandsAfterThePaint(s: Setup, executor: String): MsProposalModel {
        s.hub.sendProposal(ev("review", "chain_id" to 100, "safe_tx_hash" to s.hash, "blind" to false))
        poll("its review read") { item(s.hub, s.hash)?.let { s.hub.proposalsView.value.reviewState(it) }?.takeIf { it.kind == "ready" } }
        s.hub.sendProposal(ev("preview", "chain_id" to 100, "safe_tx_hash" to s.hash, "executor" to executor))
        poll("priced at 0x1") { look(s, executor)?.takeIf { it.kind == "priced" } }
        val drawn = painted(s)
        assertEquals("the figure drawn", "0x1", drawn.shownFee(executor))
        // The look grows stale, the price moves, and a new look lands — fresh.
        s.world.quoteAmount = "0x2"
        s.world.skewMs = 20_000.0
        s.hub.sendProposal(ev("preview", "chain_id" to 100, "safe_tx_hash" to s.hash, "executor" to executor))
        poll("the new look, not drawn") { look(s, executor)?.takeIf { it["fee"]["amount"].str == "0x2" } }
        return drawn
    }

    private suspend fun pressed(s: Setup, event: JsonObject): JsonElement {
        s.hub.sendProposal(event)
        delay(50)
        return withTimeoutOrNull(60_000) {
            var settled = executeState(s)?.takeIf { it.kind != "working" }
            while (settled == null) {
                delay(25)
                settled = executeState(s)?.takeIf { it.kind != "working" }
            }
            settled
        } ?: error("the press never settled: ${executeState(s)}; sent ${s.world.sentFees}; actions ${s.hub.proposalsView.value.actions}")
    }

    @Test
    fun `Execute carries the figure drawn, so a quote landed after the paint signs nothing and the next press signs it`() = withProposed { s ->
        s.hub.sendProposal(painted(s).approveEvent(s.owners[1]))
        poll("ready") { item(s.hub, s.hash)?.takeIf { it.status == "ready" } }
        val executor = s.owners[0]
        val drawn = aQuoteLandsAfterThePaint(s, executor)

        val press = drawn.executeEvent(executor, oneTap = false)
        assertEquals("execute", press["type"].str)
        assertEquals(JsonPrimitive("0x1"), press["shown_fee"])
        val stopped = pressed(s, press)
        assertEquals("$stopped", "failed", stopped.kind)
        assertTrue("$stopped", stopped["detail"].str.orEmpty().contains("fee_moved"))
        assertTrue("nothing signed at a fee nobody was shown: ${s.world.sentFees}", s.world.sentFees.isEmpty())

        // Said beside Execute, once, in the core's words, with the new figure:
        // §39 (r22-4) why it stopped — the fee went up — never "That didn't go through".
        val now = painted(s)
        val said = now.executeFailure!!.text
        val fee = "${app.getvela.wallet.feature.multisig.core.MsRules.feeFigure("2")} XDAI"
        assertEquals(
            "The network fee went up to ≈ $fee since you pressed; nothing was signed. Press again to pay $fee.",
            said,
        )
        assertTrue(said, !said.contains("That didn't go through"))
        assertTrue("never at the head too", now.sheetNotes.none { it.text == said })
        assertEquals("the next press carries the figure now drawn", "0x2", now.shownFee(executor))

        // Signed and sent: it then waits on the chain (`working`) — the fee sent is what counts.
        s.hub.sendProposal(now.executeEvent(executor, oneTap = false))
        poll("signed", 60_000) { s.world.sentFees.firstOrNull() }
        assertEquals(listOf(JsonPrimitive("0x2")), s.world.sentFees.map { it["amount"] })
    }

    @Test
    fun `the one press carries the figure drawn under it too`() = withProposed { s ->
        // Member 1's approval completes it: "Approve and execute", its look drawn under it.
        val presser = s.owners[1]
        val drawn = aQuoteLandsAfterThePaint(s, presser)
        poll("the one press offered") { item(s.hub, s.hash)?.let { s.hub.proposalsView.value.approveExecuteBy(it) }?.takeIf { it.isNotEmpty() } }

        val press = drawn.executeEvent(presser, oneTap = true)
        assertEquals("approve_execute", press["type"].str)
        assertEquals(JsonPrimitive("0x1"), press["shown_fee"])
        val stopped = pressed(s, press)
        assertEquals("$stopped", "failed", stopped.kind)
        assertTrue("$stopped", stopped["detail"].str.orEmpty().contains("fee_moved"))
        assertTrue(s.world.sentFees.isEmpty())

        val now = painted(s)
        assertEquals("0x2", now.shownFee(presser))
        s.hub.sendProposal(now.executeEvent(presser, oneTap = true))
        poll("signed", 60_000) { s.world.sentFees.firstOrNull() }
        assertEquals(listOf(JsonPrimitive("0x2")), s.world.sentFees.map { it["amount"] })
    }

    @Test
    fun `no figure drawn, none carried`() = withProposed { s ->
        s.hub.sendProposal(painted(s).approveEvent(s.owners[1]))
        poll("ready") { item(s.hub, s.hash)?.takeIf { it.status == "ready" } }
        // Not looked at yet: "Checking the fee…" — no figure on screen.
        val model = painted(s)
        val nobody = "0x0000000000000000000000000000000000000001"
        assertNull(model.shownFee(nobody))
        val press = model.executeEvent(nobody, oneTap = false)
        assertNull(press["shown_fee"])
        assertEquals(setOf("type", "chain_id", "safe_tx_hash", "executor", "anyway_confirmed", "refuse_blind"), press.keys)
    }
}
