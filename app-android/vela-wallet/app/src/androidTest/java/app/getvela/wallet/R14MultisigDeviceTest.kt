package app.getvela.wallet

import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTestFlags
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.core.app.ActivityScenario
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
import app.getvela.wallet.feature.multisig.core.ShownFee
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Round 14's Android findings on a device (docs/multisig-machines.md §20),
 * over the REAL sheets ([MultisigHost]) and the REAL machines, in the
 * simulated world [MultisigWorld] — no network, no funds:
 *
 * - r14-2: the newest link is the sheet shown — over a sheet already open,
 *   and over an older link whose transaction the core opens late;
 * - r14-2: an activity recreated never acts on its starting link again;
 * - r14-6: a review read a while ago starts at "Reading" when its sheet
 *   opens again, every sign button shut, until the core says;
 * - r14-9: an execution is signed only at the fee on screen when pressed.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R14MultisigDeviceTest {

    init {
        ComposeUiTestFlags.isStandardTestDispatcherSupportEnabled = true
    }

    @get:Rule
    val compose = createComposeRule(effectContext = StandardTestDispatcher())

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val scopes = mutableListOf<CoroutineScope>()
    private val hubs = mutableListOf<MultisigHub>()

    @After
    fun stop() {
        hubs.forEach { it.dispose() }
        scopes.forEach { it.cancel() }
        ShownFee.forget()
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
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x41")
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

    private fun link(safe: String, hash: String) = "#ms=100.$safe.$hash"

    /** The transaction the open sheet draws (its hash row's copyable value). */
    private fun shown(hub: MultisigHub): String? = (hub.sheet.value as? MultisigSheet.Proposal)?.safeTxHash?.lowercase()

    // -- r14-2: the newest link wins -------------------------------------------------------

    @Test
    fun aLinkOverAnOpenSheetIsShownAndAnOlderOneOpenedLateNeverTakesItBack() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, _) = twoOfThree(world)
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val first = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        val second = propose(hub, safe, "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d")

        // #0's sheet is open (a notification tapped) …
        hub.showProposal(100, first)
        waitFor("multisig.proposal.title")
        // … and #1's link arrives: #1 is the sheet shown.
        hub.takeLink(link(safe, second))
        eventually("the newest link's sheet") { shown(hub)?.takeIf { it == second.lowercase() } }
        waitFor("multisig.proposal.title")

        // An older link's transaction the core opens late is not shown over it.
        runBlocking { hub.route(jo("kind" to "open_proposal", "chain_id" to 100, "safe_tx_hash" to first)) }
        Thread.sleep(1_500)
        assertEquals(second.lowercase(), shown(hub))

        // Two links one after the other: the second wins, whichever the core answers last.
        hub.takeLink(link(safe, first))
        hub.takeLink(link(safe, second))
        Thread.sleep(3_000)
        compose.waitForIdle()
        assertEquals(second.lowercase(), shown(hub))
        Log.i(TAG, "newest link shown: $second")
    }

    // -- r14-2: the activity's starting link is acted on once ------------------------------

    @Test
    fun aRecreatedActivityNeverOpensItsStartingLinkAgain() {
        val app = instrumentation.targetContext.applicationContext as VelaWalletApplication
        val container = app.container
        val url = "https://wallet.getvela.app/en/wallet#ms=100.0x88cCA0EeDbF2C4426110bbFc998F048689266894.0x" + "ef".repeat(32)
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).setClass(instrumentation.targetContext, MainActivity::class.java)
        val seen = java.util.concurrent.CopyOnWriteArrayList<String>()
        val watching = scope().launch { container.pendingMultisigLink.collect { it?.let(seen::add) } }
        ActivityScenario.launch<MainActivity>(intent).use { scenario ->
            eventually("the link reached the app") { seen.firstOrNull() }
            // Whoever took it, it is taken: nothing waits now.
            container.pendingMultisigLink.value = null
            val before = seen.size
            // A configuration change, or the task brought back after the process was killed.
            scenario.recreate()
            Thread.sleep(3_000)
            assertEquals("the starting link was not acted on again: $seen", before, seen.size)
            assertNull(container.pendingMultisigLink.value)
        }
        watching.cancel()
    }

    // -- r14-6: a reopened review starts at Reading ----------------------------------------

    @Test
    fun aReviewReadAWhileAgoStartsAtReadingWhenItsSheetOpensAgain() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, _) = twoOfThree(world)
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        // Read: the review drawn, Approve offered (the world reads neither check: "… anyway").
        compose.waitUntil(60_000) { present("multisig.proposal.reviewUnchecked") && enabled("multisig.proposal.approve") == true }
        hub.sheet.value = null
        compose.waitForIdle()

        // A minute later, the network hung: the sheet opens again.
        val now = System.currentTimeMillis()
        hub.clock = { now + 60_000 }
        val network = CompletableDeferred<Unit>()
        world.hang = network
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.title")
        // Reading — never the old review with a plain, enabled button.
        compose.waitUntil(10_000) { present("multisig.proposal.reviewLoading") }
        assertFalse("nothing to sign while it is read", present("multisig.proposal.reviewUnchecked"))
        assertEquals("Approve shut", false, enabled("multisig.proposal.approve"))
        Thread.sleep(2_000)
        compose.waitForIdle()
        assertEquals("still shut while the network hangs", false, enabled("multisig.proposal.approve"))

        // The network back: read, and the door opens on the core's word.
        world.hang = null
        network.complete(Unit)
        compose.waitUntil(60_000) { present("multisig.proposal.reviewUnchecked") && enabled("multisig.proposal.approve") == true }

        // Opened again at once: a review that just landed stands (no second Reading).
        hub.clock = { System.currentTimeMillis() }
        hub.sheet.value = null
        compose.waitForIdle()
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.title")
        Thread.sleep(500)
        compose.waitForIdle()
        assertFalse("a fresh review is not read again", present("multisig.proposal.reviewLoading"))
    }

    // -- r14-9: the fee signed is the fee shown --------------------------------------------

    @Test
    fun anExecutionIsSignedOnlyAtTheFeeOnScreenWhenPressed() {
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

        // The look grows stale and the relay's price moves: Execute takes a new look.
        world.quoteAmount = "0x2"
        world.skewMs = 20_000.0
        tap("multisig.proposal.execute", 60_000)
        // Nothing signed at a fee nobody was shown; the new one said.
        waitFor("multisig.proposal.note.executeFailed", 30_000)
        assertTrue("nothing sent: ${world.sent.size}", world.sent.isEmpty())

        // Pressed again on the fee now shown: signed at exactly it.
        tap("multisig.proposal.execute", 60_000)
        eventually("sent", 60_000) { world.sentFees.firstOrNull() }
        val fee = world.sentFees.single()["amount"]
        assertEquals("0x2", (fee as JsonPrimitive).content)
    }

    private companion object {
        const val TAG = "r14-device"
        val NETWORKS = listOf(100, 8453)
    }
}
