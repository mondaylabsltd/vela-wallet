package app.getvela.wallet

import android.graphics.Bitmap
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTestFlags
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.crux.Wire
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
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Round 16's Android findings on a device (docs/multisig-machines.md §26),
 * over the REAL sheets and machines in the simulated world [MultisigWorld]:
 *
 * - r16-4: the one press whose fee cannot be worked out at the press signs
 *   nothing — no approval either — and says so beside it, with Check again;
 * - r16-15: the one-time setup's head and its lines are one block.
 *
 * Each case leaves a screenshot in the app's external files (`r16/`).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R16NativeDeviceTest {

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

    private fun words(world: MultisigWorld) = MultisigWords(
        strings = strings,
        localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
        networkName = { mapOf(100 to "Gnosis", 8453 to "Base")[it] },
        nativeSymbol = { mapOf(100 to "XDAI", 8453 to "ETH")[it] },
    )

    private fun show(hub: MultisigHub, world: MultisigWorld) {
        val context = MultisigHostContext(
            words = words(world),
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

    private fun shown(text: String) = compose.onAllNodesWithText(text, substring = true, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun <T : Any> eventually(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            pick()?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("never: $what")
    }

    /** The screen as drawn now, kept for the round's before/after record. */
    private fun shot(name: String) {
        val dir = File(instrumentation.targetContext.getExternalFilesDir(null), "r16").apply { mkdirs() }
        compose.waitForIdle()
        val bitmap = instrumentation.uiAutomation.takeScreenshot() ?: return
        File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private fun booted(hub: MultisigHub, safe: String) = runBlocking {
        withTimeout(30_000) { hub.boot() }
        hub.activate(safe)
        eventually("the multisig followed") { hub.proposalsView.value.followed?.takeIf { it.equals(safe, ignoreCase = true) } }
    }

    private fun propose(hub: MultisigHub, safe: String, to: String): String {
        var settled: JsonElement? = null
        val job = scope().launch {
            settled = hub.propose(100, mapOf("kind" to "transfers", "transfers" to listOf(mapOf("token" to null, "to" to to, "amount" to "1000000000000000"))), address = safe)
        }
        waitFor("multisig.draft.title")
        tap("multisig.draft.propose")
        eventually("proposed", 60_000) { settled?.takeIf { job.isCompleted } }
        assertEquals("done", settled.kind)
        return (settled["safe_tx_hash"].str ?: settled["safeTxHash"].str)!!
    }

    // -- r16-4: both or nothing -----------------------------------------------------------

    @Test
    fun aOnePressWhoseFeeIsUnknownAtThePressSignsNothingAndSaysWhyWithCheckAgain() {
        val world = world()
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "R16 Both", "0x16")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        // The other member here completes it: the one press, its fee drawn.
        waitFor("multisig.proposal.oneTapFee", 90_000)
        compose.waitUntil(30_000) { enabled("multisig.proposal.approve") == true }
        val words = words(world)
        assertTrue(shown(words.t("proposal.approveAndExecute")))
        // Beside it, the approval alone (the web's and iOS's second button).
        waitFor("multisig.proposal.approveOnly")
        val passkeys = world.passkeys.size

        // The look ages; at the press the network will not price it (the
        // tester's rate-limited Arc).
        world.skewMs = 20_000.0
        world.quoteRefusedFor += owners[1].lowercase()
        tap("multisig.proposal.approve", 60_000)
        waitFor("multisig.proposal.note.executeFailed", 60_000)
        assertTrue(shown(words.t("proposal.feeUnpriced")))
        // Nothing signed, nothing approved behind the person's back.
        Thread.sleep(1_500)
        assertEquals("no passkey", passkeys, world.passkeys.size)
        assertTrue("nothing sent", world.sent.isEmpty())
        assertEquals("the approval was not given", 1, world.mailbox.single()["approvals"].arr.size)
        // Its way on is Check again — one, beside the stop.
        assertTrue(shown(words.t("proposal.feeRetry")))
        assertFalse("one Check again, not two", present("multisig.proposal.oneTapFeeRetry"))
        shot("a-r16-4-unpriced-after")

        // The network prices it again: Check again lifts the stop and the fee is drawn.
        world.quoteRefusedFor.clear()
        tap("multisig.proposal.failureRetry")
        compose.waitUntil(60_000) { !present("multisig.proposal.note.executeFailed") && present("multisig.proposal.oneTapFee") }
        assertTrue(shown(words.t("proposal.approveAndExecute")))
        shot("a-r16-4-checked-again-after")
    }

    // -- r16-15: the setup is one block ------------------------------------------------------

    @Test
    fun anUndeployedMultisigsSetupHeadAndItsLinesAreOneBlock() {
        val world = world()
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "R16 Und", "0x5b")
        val safe = g["address"].str!!
        world.records[safe.lowercase()] = Wire.json.parseToJsonElement(
            """{"found":true,"pending":false,"genesis":${g["genesis"]},"recordedOn":[100],"readFrom":100}""",
        )
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.oneTapSetup", 90_000)
        runCatching { compose.onAllNodesWithTag("multisig.proposal.oneTapSetup")[0].performScrollTo() }
        compose.waitForIdle()
        val words = words(world)
        val head = words.t("proposal.feeSetupPlain", "network" to "Gnosis").substringBefore("Gnosis")
        fun top(text: String) = compose.onAllNodesWithText(text, substring = true, useUnmergedTree = true)
            .fetchSemanticsNodes().first().boundsInRoot
        val headBox = top(head)
        val itemBox = top(words.t("proposal.setupMultisig"))
        val density = instrumentation.targetContext.resources.displayMetrics.density
        val gapDp = (itemBox.top - headBox.bottom) / density
        // One block: the item follows its head at the block's line spacing,
        // never the sheet's section gap (the tester's "large gap").
        assertTrue("gap ${gapDp}dp", gapDp in -1f..8f)
        shot("a-r16-15-setup-after")
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
