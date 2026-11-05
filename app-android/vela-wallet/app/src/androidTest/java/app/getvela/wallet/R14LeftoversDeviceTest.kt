package app.getvela.wallet

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTestFlags
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.designsystem.components.AddressBreaks
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.multisig.MsLocal
import app.getvela.wallet.feature.multisig.MsText
import app.getvela.wallet.feature.multisig.MsType
import app.getvela.wallet.feature.multisig.MultisigHost
import app.getvela.wallet.feature.multisig.MultisigHostContext
import app.getvela.wallet.feature.multisig.MultisigSwitchAccountSection
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.settings.SettingsActions
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsRoute
import app.getvela.wallet.feature.settings.SettingsScreenState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Round 14's leftovers on a device (docs/multisig-machines.md §21), over the
 * REAL sheets and the REAL machines in the simulated world [MultisigWorld]:
 *
 * - Settings → Switch account offers Create multisig and Add multisig, each
 *   closing the Settings sheet and opening the same flow the header opens;
 * - a rate-limited add is the core's busy, and Add reads Try again;
 * - a rate-limited import is the core's busy with Try again on its row;
 * - a short form inside a subtitle is never split at its "…".
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.R14LeftoversDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R14LeftoversDeviceTest {

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

    private fun strings(lang: String = "en"): I18nRuntime = instrumentation.targetContext.let { context ->
        I18nRuntime(
            readOnDemand = { namespace, tag -> runCatching { context.assets.open(I18nCatalogs.assetPath(tag, namespace)).use { it.readBytes() } }.getOrNull() },
        ) { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply {
            initialize(lang)
            include(I18nCatalogs.MULTISIG)
        }
    }

    private val strings = strings()

    private fun world() = MultisigWorld { name ->
        instrumentation.context.assets.open("$name.hex").use { it.readBytes().toString(Charsets.UTF_8).trim() }
    }

    private fun hub(world: MultisigWorld): MultisigHub =
        MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), NETWORKS, NETWORKS, emptyList()) },
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = CoroutineScope(SupervisorJob() + Dispatchers.IO).also { scopes += it },
        ).also { hubs += it }

    private fun words(world: MultisigWorld) = MultisigWords(
        strings = strings,
        localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
        networkName = { mapOf(100 to "Gnosis", 8453 to "Base")[it] },
        nativeSymbol = { mapOf(100 to "XDAI", 8453 to "ETH")[it] },
    )

    private fun context(world: MultisigWorld) = MultisigHostContext(
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

    private fun show(hub: MultisigHub, world: MultisigWorld) {
        val context = context(world)
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
    }

    private fun type(tag: String, text: String) {
        waitFor(tag)
        val field = compose.onAllNodes(androidx.compose.ui.test.hasAnyAncestor(hasTestTag(tag)) and androidx.compose.ui.test.hasSetTextAction())
            .fetchSemanticsNodes().let { if (it.isNotEmpty()) compose.onAllNodes(androidx.compose.ui.test.hasAnyAncestor(hasTestTag(tag)) and androidx.compose.ui.test.hasSetTextAction())[0] else null }
            ?: compose.onAllNodes(hasTestTag(tag) and androidx.compose.ui.test.hasSetTextAction())[0]
        runCatching { field.performScrollTo() }
        field.performTextInput(text)
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

    private fun booted(hub: MultisigHub) = runBlocking { withTimeout(30_000) { hub.boot() } }

    /** A 1-of-1 of fixture account 0, its genesis with the service, deployed on Gnosis — not kept here. */
    private fun recorded(world: MultisigWorld, salt: String): String {
        val owner = world.address(0)
        val g = world.genesis(listOf(owner), 1, "Ops", salt)
        val address = g["address"].str!!
        world.records[address.lowercase()] = Wire.json.parseToJsonElement(
            """{"found":true,"pending":false,"genesis":${g["genesis"]},"recordedOn":[100],"readFrom":100}""",
        )
        world.deploy(100, address, MultisigWorld.Safe(listOf(owner), 1))
        return address
    }

    // -- item 1: Settings → Switch account reaches multisig create / add ------------

    @Test
    fun settingsSwitchAccountOffersCreateAndAddMultisigClosingItselfFirst() {
        val world = world()
        world.locals += 0
        val hub = hub(world)
        booted(hub)
        val words = words(world)
        val context = context(world)
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SettingsRoute(
                        model = model,
                        actions = SettingsActions(multisigSection = { close -> MultisigSwitchAccountSection(hub, words, close) }),
                    )
                    MultisigHost(hub, context)
                }
            }
        }
        fun openSwitchAccount() {
            compose.onNodeWithText(model.account.action, substring = true).performScrollTo().performClick()
            compose.waitForIdle()
            waitFor("multisig.switcher.create")
        }
        openSwitchAccount()
        assertTrue("Add multisig beside Create", present("multisig.switcher.add"))
        assertTrue("under the Vela accounts' own Create / Sign in", shown(model.accountsSheet.title))

        tap("multisig.switcher.create")
        // The Settings sheet closed first; the create flow is the one open.
        compose.waitUntil(10_000) { !present("multisig.switcher.create") }
        assertEquals(MultisigSheet.Create, hub.sheet.value)
        waitFor("multisig.create")

        hub.sheet.value = null
        compose.waitUntil(10_000) { !present("multisig.create") }
        openSwitchAccount()
        tap("multisig.switcher.add")
        compose.waitUntil(10_000) { !present("multisig.switcher.add") }
        assertTrue(hub.sheet.value is MultisigSheet.Add)
        waitFor("multisig.add")
    }

    // -- item 2: a rate-limited add is busy, and Add says Try again -----------------

    @Test
    fun aRateLimitedAddIsNamedBusyAndAddReadsTryAgainUntilItAdds() {
        val world = world()
        world.locals += 0
        val address = recorded(world, "0x71")
        world.rateLimited = setOf(100)
        val hub = hub(world)
        booted(hub)
        show(hub, world)
        hub.sheet.value = MultisigSheet.Add()
        type("multisig.add.address", address)
        tap("multisig.add.add")
        // Nothing answers where it is used while Gnosis is limiting: the core
        // asks which network; the person names it and adds.
        tap("multisig.add.network.100", 40_000)
        tap("multisig.add.add")
        val tryAgain = words(world).t("review.state.tryAgain")
        val busy = words(world).t("add.busy", "network" to "Gnosis")
        // One bounded pause (the core's RATE_LIMIT_PAUSE_MS), then named.
        compose.waitUntil(40_000) { shown(busy) }
        compose.waitUntil(10_000) {
            compose.onAllNodes(hasTestTag("multisig.add.add") and hasText(tryAgain)).fetchSemanticsNodes().isNotEmpty()
        }
        assertTrue("nothing added", world.storedAccounts().isEmpty())

        world.rateLimited = emptySet()
        tap("multisig.add.add")
        eventually("added", 40_000) { world.storedAccounts().firstOrNull { it["address"].str.equals(address, ignoreCase = true) } }
        compose.waitUntil(10_000) { hub.sheet.value == null }
    }

    // -- item 4: a rate-limited import is busy, with Try again on its row -------------

    @Test
    fun aRateLimitedImportIsNamedBusyWithTryAgainOnItsRow() {
        val world = world()
        world.locals += 0
        val address = recorded(world, "0x72")
        val hub = hub(world)
        booted(hub)
        show(hub, world)
        hub.sheet.value = MultisigSheet.Add()
        waitFor("multisig.add")
        val export = """{"version":"3.0","data":{"addedSafes":{"100":{"$address":{"owners":[{"value":"${world.address(0)}"}],"threshold":1}}},"undeployedSafes":{},"addressBook":{"100":{"$address":"Ops"}}}}"""
        hub.send(ev("parse_export", "json" to export))
        waitFor("multisig.import.safe")
        world.rateLimited = setOf(100)
        tap("multisig.import.safe")
        tap("multisig.import.go")
        val busy = words(world).t("add.busy", "network" to "Gnosis")
        waitFor("multisig.import.retry", 40_000)
        assertTrue("the core's busy words, never 'unavailable'", shown(busy))
        assertTrue(!shown(words(world).t("add.unavailable")))
        assertTrue("nothing imported", world.storedAccounts().isEmpty())

        world.rateLimited = emptySet()
        tap("multisig.import.retry")
        eventually("imported", 40_000) { world.storedAccounts().firstOrNull { it["address"].str.equals(address, ignoreCase = true) } }
        compose.waitUntil(10_000) { !present("multisig.import.retry") }
    }

    // -- item 5: a short form in a subtitle is never split at its "…" ---------------

    @Test
    fun aShortFormInsideASubtitleNeverSplitsAtAnyWidth() {
        lateinit var measurer: TextMeasurer
        val zh = strings("zh")
        // A switcher / settings subtitle as the shells compose it: the short
        // form, then the multisig's tags, in a CJK locale.
        val short = MultisigWords.short("0x777E989C821a7F1b031696f044a2fF114261875a")
        val subtitle = "成员 $short · 未共享 · 只读 · 3 人中的 2 人"
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides zh) {
                VelaTheme(darkTheme = false) {
                    measurer = rememberTextMeasurer()
                    Box(Modifier.width(140.dp)) { MsText(subtitle, MsType.Sub, Modifier.testTag("subtitle")) }
                }
            }
        }
        compose.waitForIdle()
        // A reader (and every text match) reads the plain words, no joiner.
        val read = compose.onAllNodesWithTag("subtitle", useUnmergedTree = true).fetchSemanticsNodes().single()
            .config[SemanticsProperties.Text].joinToString("") { it.text }
        assertEquals(subtitle, read)

        val style = TextStyle(fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
        val density = compose.density.density
        val drawn = AddressBreaks.wholeShortForms(subtitle)
        val start = drawn.indexOf(short.substringBefore('…'))
        val end = drawn.indexOf(short.substringAfter('…')) + short.substringAfter('…').length - 1
        val narrowest = measurer.measure(AddressBreaks.wholeShortForms(short), style).size.width
        var wrapped = 0
        for (px in narrowest..(420 * density).toInt()) {
            val layout = measurer.measure(drawn, style, constraints = Constraints(maxWidth = px))
            if (layout.lineCount > 1) wrapped += 1
            assertEquals(
                "the short form splits at ${px / density}dp: ${(0 until layout.lineCount).map { drawn.substring(layout.getLineStart(it), layout.getLineEnd(it)) }}",
                layout.getLineForOffset(start), layout.getLineForOffset(end),
            )
        }
        assertTrue("the subtitle did wrap at some widths", wrapped > 0)
        // The control: unjoined, it DOES split after the "…" at some width.
        val looseStart = subtitle.indexOf(short)
        val looseEnd = looseStart + short.length - 1
        val split = (narrowest..(420 * density).toInt()).count { px ->
            val layout = measurer.measure(subtitle, style, constraints = Constraints(maxWidth = px))
            layout.getLineForOffset(looseStart) != layout.getLineForOffset(looseEnd)
        }
        assertTrue("without the joiners the short form splits at some width (the check has teeth)", split > 0)
        // The joiners take no room.
        assertEquals(measurer.measure(short, style).size.width, measurer.measure(AddressBreaks.wholeShortForms(short), style).size.width)
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
