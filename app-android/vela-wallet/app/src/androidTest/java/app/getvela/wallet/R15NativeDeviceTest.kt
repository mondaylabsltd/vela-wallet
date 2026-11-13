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
import app.getvela.wallet.feature.multisig.core.ev
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
 * Round 15's Android findings on a device (docs/multisig-machines.md §23),
 * over the REAL sheets and machines in the simulated world [MultisigWorld]:
 *
 * - r15-7: an undeployed multisig's one press says its one-time setup (an
 *   unread setup is the JVM `R15NativeFixesTest`'s: the world cannot leave
 *   the Safe contracts unread while the balance read answers);
 * - r15-8: a rate-limited import's Try again lands as "Imported", never
 *   "Already added".
 *
 * Each case leaves a screenshot in the app's external files (`r15/`).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R15NativeDeviceTest {

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
        // R22: the world answers neither safety check — "… anyway", held:
        // pressed as a person using TalkBack presses it (twice), else tapped.
        compose.onAllNodesWithTag(tag)[0].confirmPress()
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
        val dir = File(instrumentation.targetContext.getExternalFilesDir(null), "r15").apply { mkdirs() }
        compose.waitForIdle()
        // The whole screen: a sheet is a window of its own, not the root's.
        val bitmap = instrumentation.uiAutomation.takeScreenshot() ?: return
        File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    /** A 2-of-3 of fixture accounts 0, 1, 2, kept here — NOT deployed anywhere. */
    private fun undeployed(world: MultisigWorld): Pair<String, List<String>> {
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "R15 Und", "0x5a")
        val safe = g["address"].str!!
        world.records[safe.lowercase()] = Wire.json.parseToJsonElement(
            """{"found":true,"pending":false,"genesis":${g["genesis"]},"recordedOn":[100],"readFrom":100}""",
        )
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        return safe to owners
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

    // -- r15-7: the one press says the one-time setup ----------------------------------

    @Test
    fun anUndeployedMultisigsOnePressSaysItsOneTimeSetup() {
        val world = world()
        world.locals += listOf(0, 1)
        val (safe, _) = undeployed(world)
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        // The other member here completes it in one press: its fee, then the
        // setup that press carries (deploying this multisig).
        waitFor("multisig.proposal.oneTapFee", 90_000)
        waitFor("multisig.proposal.oneTapSetup", 60_000)
        val words = words(world)
        assertTrue(shown(words.t("proposal.setupMultisig")))
        assertTrue(shown(words.t("proposal.feeSetupPlain", "network" to "Gnosis").substringBefore("Gnosis")))
        runCatching { compose.onAllNodesWithTag("multisig.proposal.oneTapSetup")[0].performScrollTo() }
        compose.waitForIdle()
        shot("a-r15-7-setup-after")
    }

    // -- r15-8: Try again lands as "Imported" ---------------------------------------------

    @Test
    fun aRateLimitedImportsTryAgainLandsAsImportedNeverAlreadyAdded() {
        val world = world()
        world.locals += 0
        val owner = world.address(0)
        val g = world.genesis(listOf(owner), 1, "Ops", "0x7a")
        val address = g["address"].str!!
        world.records[address.lowercase()] = Wire.json.parseToJsonElement(
            """{"found":true,"pending":false,"genesis":${g["genesis"]},"recordedOn":[100],"readFrom":100}""",
        )
        world.deploy(100, address, MultisigWorld.Safe(listOf(owner), 1))
        val hub = hub(world)
        runBlocking { withTimeout(30_000) { hub.boot() } }
        show(hub, world)
        hub.sheet.value = MultisigSheet.Add()
        waitFor("multisig.add")
        val export = """{"version":"3.0","data":{"addedSafes":{"100":{"$address":{"owners":[{"value":"$owner"}],"threshold":1}}},"undeployedSafes":{},"addressBook":{"100":{"$address":"Ops"}}}}"""
        hub.send(ev("parse_export", "json" to export))
        waitFor("multisig.import.safe")
        world.rateLimited = setOf(100)
        tap("multisig.import.safe")
        tap("multisig.import.go")
        waitFor("multisig.import.retry", 40_000)

        world.rateLimited = emptySet()
        tap("multisig.import.retry")
        eventually("imported", 40_000) { world.storedAccounts().firstOrNull { it["address"].str.equals(address, ignoreCase = true) } }
        val words = words(world)
        compose.waitUntil(15_000) { shown(words.t("import.imported")) }
        assertFalse("never 'Already added' for the Safe this import added", shown(words.t("import.already")))
        assertFalse(present("multisig.import.retry"))
        assertEquals("nothing left to import: no press that imports nothing", false, enabled("multisig.import.go"))
        shot("a-r15-8-imported-after")
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
