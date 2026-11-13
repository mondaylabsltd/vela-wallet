package app.getvela.wallet

import android.graphics.Bitmap
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
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
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
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
 * Round 17's Android findings on a device, over the REAL sheets and machines
 * in the simulated world [MultisigWorld]:
 *
 * - r17-12: an execution the relay never took is said by its kind ("The
 *   network didn't take it…"), never "Something went wrong";
 * - r17-16: that one press's failure is not drawn beside Execute once a later
 *   Approve went through; an approval the service did not take is worded.
 *
 * Each case leaves a screenshot in the app's external files (`r17/`).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R17NativeDeviceTest {

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

    private fun shot(name: String) {
        val dir = File(instrumentation.targetContext.getExternalFilesDir(null), "r17").apply { mkdirs() }
        compose.waitForIdle()
        val bitmap = instrumentation.uiAutomation.takeScreenshot() ?: return
        File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private fun scrollTo(tag: String) {
        runCatching { compose.onAllNodesWithTag(tag, useUnmergedTree = true)[0].performScrollTo() }
        compose.waitForIdle()
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

    /** A 2-of-3 with two members here, one transaction proposed by the first: the second's press completes it. */
    private fun opened(world: MultisigWorld, name: String, salt: String): Pair<MultisigHub, List<String>> {
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, name, salt)
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val hub = hub(world)
        booted(hub, safe)
        show(hub, world)
        val hash = propose(hub, safe, "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        hub.sheet.value = MultisigSheet.Proposal(100, hash)
        waitFor("multisig.proposal.oneTapFee", 90_000)
        waitFor("multisig.proposal.approveOnly")
        return hub to owners
    }

    // -- r17-12 + r17-16 (1) ----------------------------------------------------------------

    @Test
    fun aOnePressTheRelayNeverTookIsSaidByItsKindAndGoesOnceALaterApproveWentThrough() {
        val world = world()
        val (_, _) = opened(world, "R17 Stale", "0x17")
        val words = words(world)

        // The relay never takes it: nothing sent, said by its kind.
        world.sendFailure = MultisigShellFailure.NotSent.detail
        tap("multisig.proposal.approve", 60_000)
        waitFor("multisig.proposal.note.executeFailed", 60_000)
        assertTrue(shown(words.t("proposal.notSent")))
        assertFalse("never the generic line", shown(words.t("proposal.reason.unknown")))
        assertEquals("nothing approved behind the person's back", 1, world.mailbox.single()["approvals"].arr.size)
        scrollTo("multisig.proposal.note.executeFailed")
        shot("a-r17-12-not-sent-after")

        // The approval alone goes through: the one press's failure is stale.
        world.sendFailure = null
        tap("multisig.proposal.approveOnly", 60_000)
        eventually("approved", 60_000) { world.mailbox.single()["approvals"].arr.size.takeIf { it == 2 } }
        waitFor("multisig.proposal.execute", 60_000)
        compose.waitUntil(30_000) { !present("multisig.proposal.note.executeFailed") }
        assertFalse(shown(words.t("proposal.notSent")))
        scrollTo("multisig.proposal.execute")
        shot("b-r17-16-stale-failure-gone-after")
    }

    // -- r17-16 (2) ---------------------------------------------------------------------------

    @Test
    fun anApprovalTheServiceDidNotTakeIsWordedBesideSendAgain() {
        val world = world()
        opened(world, "R17 Kept", "0x18")
        val words = words(world)
        // The service is down at the approval: it is kept here and sent again on its own.
        world.serviceDown = true
        tap("multisig.proposal.approveOnly", 60_000)
        waitFor("multisig.proposal.note.outbox", 60_000)
        val away = words.t("proposal.outboxAway", "seconds" to "").substringBefore(" s.").trim().substringBefore("—").trim()
        val sending = words.t("proposal.outboxSending")
        compose.waitUntil(30_000) { shown(away) || shown(sending) || shown(words.t("proposal.outboxNotSent")) }
        scrollTo("multisig.proposal.note.outbox")
        shot("c-r17-16-kept-approval-worded-after")
        world.serviceDown = false
    }

    // -- r17-14 -----------------------------------------------------------------------------

    @Test
    fun theTestnetSpacesSendPricesNoAmountAndNoFee() {
        val usdc = app.getvela.wallet.feature.send.core.SendToken(network = "Arc Testnet", chain_id = 5042002, symbol = "USDC", balance = "0.2", decimals = 6, price_usd = 1.0)
        val cny = app.getvela.wallet.feature.settings.core.CurrencyView(code = "CNY", rate = 7.1, committed = true)
        val chains = mapOf(5042002 to "Arc Testnet")
        fun ctx(money: app.getvela.wallet.feature.wallet.WalletLive.Money) =
            app.getvela.wallet.feature.send.SendLive.Context(strings, chains, emptyMap(), money, "Main", "0xD400866e00B055B20752a826CD5C89b811de130b")
        val pickFallback = (app.getvela.wallet.feature.flows.FlowFixtures.build(app.getvela.wallet.feature.flows.FlowState.SD1, strings).base as app.getvela.wallet.feature.flows.FlowBase.SendPick).model
        val formFallback = (app.getvela.wallet.feature.flows.FlowFixtures.build(app.getvela.wallet.feature.flows.FlowState.SD2N, strings).base as app.getvela.wallet.feature.flows.FlowBase.SendForm).model
        val typed = app.getvela.wallet.feature.send.core.SendView(
            tokens = listOf(usdc), selected_token = usdc, amount = "0.1", token_amount = "0.1", denom_toggle_shown = true, denom_toggle_enabled = true,
        )
        var money by androidx.compose.runtime.mutableStateOf(app.getvela.wallet.feature.wallet.WalletLive.Money.of(cny, "en"))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    androidx.compose.foundation.layout.Column {
                        app.getvela.wallet.feature.flows.SendPickBody(app.getvela.wallet.feature.send.SendLive.pick(pickFallback, typed, ctx(money)))
                        app.getvela.wallet.feature.flows.SendFormBody(
                            app.getvela.wallet.feature.send.SendLive.form(formFallback, typed, app.getvela.wallet.feature.send.core.FeeView(), ctx(money)),
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
        assertTrue("mainnet: the worth is drawn", shown("CN¥"))
        shot("d-r17-14-send-mainnet-priced-before")
        money = money.withoutValue()
        compose.waitForIdle()
        assertFalse("testnet: no worth anywhere", shown("CN¥"))
        shot("d-r17-14-send-testnet-no-worth-after")
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
