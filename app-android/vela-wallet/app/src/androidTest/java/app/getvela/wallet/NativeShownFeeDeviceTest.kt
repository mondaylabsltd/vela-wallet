package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.multisig.MsLocal
import app.getvela.wallet.feature.multisig.MultisigHost
import app.getvela.wallet.feature.multisig.MultisigHostContext
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * §24 "Natives carry the fee shown" (docs/multisig-machines.md) on a device,
 * over the REAL sheets ([MultisigHost]) and machines in the simulated world
 * [MultisigWorld]: Execute carries the figure painted above it (`shown_fee`),
 * so a quote that lands between the paint and the press signs nothing — the
 * core's `fee_moved`, said beside Execute once — and the next press signs the
 * new figure.
 *
 * "Between the paint and the press" is made exact by holding the frame clock
 * (`mainClock.autoAdvance = false`): the new look is in the core, the screen
 * still draws the old figure, and the press lands on what is drawn. The
 * composition's effects run on the main dispatcher (not a test dispatcher),
 * so the press itself is handled while no frame is drawn.
 */
@RunWith(AndroidJUnit4::class)
class NativeShownFeeDeviceTest {

    @get:Rule
    val compose = createComposeRule()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val scopes = mutableListOf<CoroutineScope>()
    private val hubs = mutableListOf<MultisigHub>()

    @After
    fun stop() {
        hubs.forEach { it.dispose() }
        scopes.forEach { it.cancel() }
    }

    private val strings: I18nRuntime = instrumentation.targetContext.let { context ->
        I18nRuntime(
            readOnDemand = { namespace, tag -> runCatching { context.assets.open(I18nCatalogs.assetPath(tag, namespace)).use { it.readBytes() } }.getOrNull() },
        ) { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply {
            initialize("en")
            include(I18nCatalogs.MULTISIG)
        }
    }

    private fun world() = MultisigWorld { name ->
        instrumentation.context.assets.open("$name.hex").use { it.readBytes().toString(Charsets.UTF_8).trim() }
    }

    private fun scope() = CoroutineScope(SupervisorJob() + Dispatchers.IO).also { scopes += it }

    private fun hub(world: MultisigWorld): MultisigHub =
        MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), NETWORKS, NETWORKS, emptyList()) },
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = scope(),
        ).also { hubs += it }

    private fun show(hub: MultisigHub, world: MultisigWorld) {
        val words = MultisigWords(
            strings = strings,
            localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
            networkName = { mapOf(100 to "Gnosis", 8453 to "Base")[it] },
            nativeSymbol = { mapOf(100 to "XDAI", 8453 to "ETH")[it] },
        )
        val context = MultisigHostContext(
            words = words,
            accounts = world.locals.map { MsLocal(world.fixtures[it].address, world.fixtures[it].name) },
            contacts = emptyList(),
            spaceNetworks = listOf(100),
            otherSpaceName = "Testnets",
            origin = "https://wallet.getvela.app",
            locale = "en",
            tokenInfo = { _, _ -> "XDAI" to 18 },
            explorerTx = { _, _ -> null },
            onAddNetwork = {},
            onFund = {},
            askNotifications = {},
        )
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { MultisigHost(hub, context) }
            }
        }
    }

    private fun present(tag: String) = compose.onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun waitFor(tag: String, timeoutMs: Long = 30_000) = compose.waitUntil(timeoutMs) { present(tag) }

    private fun enabled(tag: String): Boolean? =
        compose.onAllNodesWithTag(tag).fetchSemanticsNodes().firstOrNull()?.config?.contains(SemanticsProperties.Disabled)?.not()

    /** The words a tagged line draws (its own text and its children's). */
    private fun text(tag: String): String =
        compose.onAllNodes(hasTestTag(tag) or hasAnyAncestor(hasTestTag(tag)), useUnmergedTree = true).fetchSemanticsNodes()
            .mapNotNull { node -> node.config.getOrNull(SemanticsProperties.Text)?.joinToString("") { it.text } }
            .joinToString(" ")

    private fun tap(tag: String, timeoutMs: Long = 30_000) {
        waitFor(tag, timeoutMs)
        compose.waitUntil(timeoutMs) { enabled(tag) == true }
        runCatching { compose.onAllNodesWithTag(tag)[0].performScrollTo() }
        compose.onAllNodesWithTag(tag)[0].performClick()
        compose.waitForIdle()
        // The world answers neither safety check: "… anyway", confirmed as a person would.
        val anyway = compose.onAllNodesWithTag("multisig.sign.anywayConfirm")
        if (anyway.fetchSemanticsNodes().isNotEmpty()) anyway[0].performClick()
        compose.waitForIdle()
    }

    private fun <T : Any> eventually(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            pick()?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("never: $what")
    }

    private fun booted(hub: MultisigHub, safe: String) = runBlocking {
        withTimeout(30_000) { hub.boot() }
        hub.activate(safe)
        eventually("the multisig followed") { hub.proposalsView.value.followed?.takeIf { it.equals(safe, ignoreCase = true) } }
    }

    /** A 2-of-3 of fixture accounts 0, 1, 2, kept here, deployed on Gnosis. */
    private fun twoOfThree(world: MultisigWorld): Pair<String, List<String>> {
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x5e")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        return safe to owners
    }

    /** A transfer proposed as the send screen proposes it: its review, then Propose. Its hash. */
    private fun propose(hub: MultisigHub, safe: String, to: String): String {
        var settled: JsonElement? = null
        val job = scope().launch {
            settled = hub.propose(100, mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to to, "amount" to "1000000000000000000"))), address = safe)
        }
        waitFor("multisig.draft.title")
        tap("multisig.draft.propose")
        eventually("proposed", 60_000) { settled?.takeIf { job.isCompleted } }
        assertEquals("done", settled.kind)
        return (settled["safe_tx_hash"].str ?: settled["safeTxHash"].str)!!
    }

    @Test
    fun aQuoteLandingBetweenThePaintAndThePressSignsNothingAndTheNextPressSignsIt() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, owners) = twoOfThree(world)
        // Member 1's own look fails: approve, then Execute as member 0.
        world.quoteRefusedFor += owners[1].lowercase()
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        tap("multisig.proposal.approve", 60_000)
        eventually("approved") { world.mailbox.single()["approvals"].arr.size.takeIf { it == 2 } }
        // Ready; Execute's look priced at 0x1 is on screen.
        waitFor("multisig.proposal.fee", 60_000)
        compose.waitUntil(30_000) { enabled("multisig.proposal.execute") == true }
        val drawn = text("multisig.proposal.fee")
        assertTrue(drawn, drawn.isNotEmpty())

        // Execute pressed: the world reads neither safety check, so its
        // "… anyway" asks once more. While it asks, nothing more is painted …
        runCatching { compose.onAllNodesWithTag("multisig.proposal.execute")[0].performScrollTo() }
        compose.onAllNodesWithTag("multisig.proposal.execute")[0].performClick()
        waitFor("multisig.sign.anywayConfirm")
        compose.mainClock.autoAdvance = false
        // … and the relay's price moves: a new look lands in the core, fresh.
        world.quoteAmount = "0x2"
        world.skewMs = 20_000.0
        hub.sendProposal(ev("preview", "chain_id" to 100, "safe_tx_hash" to hash, "executor" to owners[0]))
        eventually("the new look, not drawn") {
            hub.proposalsView.value.item(100, hash)?.let { hub.proposalsView.value.preview(it, owners[0]) }
                ?.takeIf { it["fee"]["amount"].str == "0x2" }
        }
        assertEquals("the screen still says the figure pressed on", drawn, text("multisig.proposal.fee"))
        // The button's own click (its semantics action): an injected touch
        // would move the held clock by a frame between down and up.
        compose.onAllNodesWithTag("multisig.sign.anywayConfirm")[0].performSemanticsAction(SemanticsActions.OnClick)
        // The press is the machine's before any frame is drawn.
        eventually("the press reached the machine") {
            hub.proposalsView.value.action("execute:$hash")?.takeIf { it.kind != null }
        }
        compose.mainClock.autoAdvance = true

        // The press carried the figure drawn: nothing signed at the new one,
        // the stop said beside Execute, once, with the new figure.
        val settled = eventually("the press settled", 60_000) {
            hub.proposalsView.value.action("execute:$hash")?.takeIf { it.kind != "working" }
                ?: world.sentFees.firstOrNull()
        }
        assertEquals("fee_moved, not signed at ${world.sentFees}: $settled", "failed", settled.kind)
        assertTrue("$settled", settled["detail"].str.orEmpty().contains("fee_moved"))
        assertTrue("nothing sent: ${world.sent.size}", world.sent.isEmpty())
        waitFor("multisig.proposal.note.executeFailed", 30_000)
        assertEquals(1, compose.onAllNodesWithTag("multisig.proposal.note.executeFailed", useUnmergedTree = true).fetchSemanticsNodes().size)
        val said = text("multisig.proposal.note.executeFailed")
        assertEquals(said, 1, said.split("That didn't go through").size - 1)
        assertTrue(said, said.contains("Network fee"))
        // (1 and 2 wei read the same at the line's precision: the core
        // compares base units, so the figure carried is the look's own.)

        // Pressed again on the figure now shown: signed at exactly it.
        tap("multisig.proposal.execute", 60_000)
        eventually("sent", 60_000) { world.sentFees.firstOrNull() }
        assertEquals("0x2", (world.sentFees.single()["amount"] as JsonPrimitive).content)
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
