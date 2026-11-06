package app.getvela.wallet

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.HistoryBody
import app.getvela.wallet.feature.flows.HistoryModel
import app.getvela.wallet.feature.multisig.MsLocal
import app.getvela.wallet.feature.multisig.MultisigHost
import app.getvela.wallet.feature.multisig.MultisigHostContext
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletHomeModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Round 16's leftovers on a device:
 *
 * - 5: the home's Activity and History say which network the incoming scan
 *   could not read — over the rows, or in the empty state's place (never the
 *   skeleton, never "No activity yet").
 * - 3: a proposal sheet opened without a link, over a multisig service that
 *   answers its rate limit, says busy with Try again — never "Couldn't reach
 *   the multisig service" (the REAL sheet and machines, in [MultisigWorld]).
 *
 * Each state is kept as a picture in the app's cache (`r16l-*.png`).
 */
@RunWith(AndroidJUnit4::class)
class R16LeftoversDeviceTest {

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

    private fun shot(name: String) {
        instrumentation.uiAutomation.takeScreenshot()?.let { picture ->
            java.io.File(instrumentation.targetContext.cacheDir, "r16l-$name.png").outputStream().use {
                picture.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
            }
        }
    }

    private fun text(tag: String): String =
        compose.onAllNodes(hasTestTag(tag) or hasAnyAncestor(hasTestTag(tag)), useUnmergedTree = true).fetchSemanticsNodes()
            .mapNotNull { node -> node.config.getOrNull(SemanticsProperties.Text)?.joinToString("") { it.text } }
            .joinToString(" ")

    // -- item 5: Activity names the networks it could not read ----------------------------

    private val account = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private val chainNames = mapOf(137 to "Polygon", 100 to "Gnosis")

    private fun received(id: String) = FeedRow.Item(
        FeedItem(
            id = id, direction = FeedDirection.In, counterparty = "0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D",
            value = "0.3", symbol = "USDC", decimals = 6, usd_value = 0.3, chain_id = 100,
            timestamp = 1_790_000_000.0, day_start_ms = 1_789_948_800_000.0,
        ),
    )

    private fun feed(unread: List<Int>, rows: List<FeedRow> = emptyList()): FeedView {
        val all = if (rows.isEmpty()) rows else listOf(FeedRow.Header(id = "day-1", day_start_ms = 1_789_948_800_000.0, timestamp = 1_790_000_000.0)) + rows
        val key = if (unread.size == 1) "home.activityUnreadOne" else if (unread.isEmpty()) null else "home.activityUnreadMany"
        return FeedView(rows = all, home_rows = all, account = account, settled = unread.isEmpty(), unread_chains = unread, unread_key = key)
    }

    private fun home(feed: FeedView) = WalletLive.home(
        WalletFixtures.buildMobileState(WalletScreenState.H1, strings),
        BalanceView(address = account),
        feed,
        CurrencyView(code = "USD", committed = true),
        strings,
        chainNames,
        shownAccount = account,
    )

    @Test
    fun theHomeActivitySaysTheNetworkItCouldNotRead() {
        val line = strings.t("home.activityUnreadOne", mapOf("network" to "Polygon"))
        var current by mutableStateOf<WalletHomeModel>(home(feed(listOf(137))))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { WalletScreen(model = current) }
            }
        }
        // No rows: the line in the empty state's place.
        compose.onNodeWithTag("home.activity.empty").performScrollTo().assertExists()
        assertTrue(text("home.activity.empty"), text("home.activity.empty").contains(line))
        compose.onNodeWithText(strings.t("home.emptyNoActivity")).assertDoesNotExist()
        compose.onNodeWithTag("home.activity.unread").assertDoesNotExist()
        shot("home-unread-empty")

        // Rows: the line above them.
        current = home(feed(listOf(137), rows = listOf(received("r1"))))
        compose.waitForIdle()
        compose.onNodeWithTag("home.activity.unread").performScrollTo().assertExists()
        assertEquals(line, text("home.activity.unread"))
        shot("home-unread-rows")

        // Every network read: no line.
        current = home(feed(emptyList(), rows = listOf(received("r1"))))
        compose.waitForIdle()
        compose.onNodeWithTag("home.activity.unread").assertDoesNotExist()
    }

    @Test
    fun historySaysTheNetworksItCouldNotRead() {
        val fallback = (FlowFixtures.build(FlowState.A1, strings).base as FlowBase.History).model
        val many = strings.t("home.activityUnreadMany", mapOf("n" to "2"))
        var current by mutableStateOf<HistoryModel>(FlowLive.history(fallback, feed(listOf(100, 137)), strings, chainNames = chainNames, shownAccount = account))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { Column { HistoryBody(model = current) } }
            }
        }
        assertEquals(many, text("history.empty"))
        shot("history-unread-empty")
        current = FlowLive.history(fallback, feed(listOf(100, 137), rows = listOf(received("r1"))), strings, chainNames = chainNames, shownAccount = account)
        compose.waitForIdle()
        assertEquals(many, text("history.unread"))
        assertTrue(compose.onAllNodesWithTag("history.empty").fetchSemanticsNodes().isEmpty())
        shot("history-unread-rows")
    }

    // -- item 3: a busy multisig service on the sheet -------------------------------------

    private fun world() = MultisigWorld { name ->
        instrumentation.context.assets.open("$name.hex").use { it.readBytes().toString(Charsets.UTF_8).trim() }
    }

    @Test
    fun aSheetOpenedWithoutALinkOverABusyServiceSaysBusyWithTryAgain() {
        val world = world()
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x62")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val hub = MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = CoroutineScope(SupervisorJob() + Dispatchers.IO).also { scopes += it },
        ).also { hubs += it }
        runBlocking {
            withTimeout(60_000) { hub.boot() }
            hub.activate(safe)
        }
        val deadline = System.currentTimeMillis() + 30_000
        while (hub.proposalsView.value.followed == null && System.currentTimeMillis() < deadline) Thread.sleep(25)
        world.serviceBusy = true
        hub.sendProposal(ev("retry"))
        val busyBy = System.currentTimeMillis() + 60_000
        while (!hub.proposalsView.value.busy && System.currentTimeMillis() < busyBy) Thread.sleep(25)
        assertTrue("the machine says busy", hub.proposalsView.value.busy)

        val words = MultisigWords(
            strings = strings,
            localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
            networkName = { mapOf(100 to "Gnosis")[it] },
            nativeSymbol = { mapOf(100 to "XDAI")[it] },
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
        hub.sheet.value = MultisigSheet.Proposal(100, "0x" + "ee".repeat(32))
        compose.waitUntil(30_000) { compose.onAllNodesWithTag("multisig.proposal.missing", useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
        val said = text("multisig.proposal.missing")
        assertEquals(words.t("service.busyNote"), said)
        assertFalse(said, said.contains(words.t("proposal.unread.linkService", "network" to "Gnosis")))
        compose.onNodeWithTag("multisig.proposal.missingRetry").assertExists()
        shot("sheet-busy")
    }
}
