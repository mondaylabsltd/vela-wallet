package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.ComposeUiTestFlags
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
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
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.onboarding.core.RelyingParty
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonElement
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpSafeOpHash
import uniffi.vela_core_uniffi.userOpStampValidity

/**
 * Round 21 (§35) on a device, through the app's OWN core library:
 *
 * - SEC-PARENT-2: a site asking a connected multisig to call ITSELF with an
 *   admin selector (`changeThreshold`) is refused by the real multisig
 *   machine at its dApp entry — nothing drafted, nothing signed — and the
 *   sheet says it in the catalog's words under "Vela can't sign this";
 *   leaving answers the site `-32603`.
 * - review 0-1: this debug build on this emulator asks the core for its
 *   passkeys' relying party and is told `localhost` — never getvela.app.
 * - review 1-6: the phone's core stamps the SafeOp's window and refuses to
 *   hash one that never expires.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class R21DoorsDeviceTest {

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

    private fun present(tag: String) = compose.onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun shown(text: String) = compose.onAllNodesWithText(text, substring = true, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun <T : Any> eventually(what: String, timeoutMs: Long = 30_000, pick: () -> T?): T {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            pick()?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("never: $what")
    }

    @Test
    fun aSitesSelfCallOnTheMultisigIsRefusedOnThisPhoneInTheCatalogsWords() {
        val world = world()
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "R21 Self", "0x21")
        val safe = g["address"].str!!
        world.deploy(100, safe, MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val hub = MultisigHub(
            live = world,
            facts = { MultisigHub.Facts(MultisigWorld.SERVICE, world.localAccounts(), NETWORKS, NETWORKS, emptyList()) },
            tokenInfo = { _, _ -> "XDAI" to 18 },
            scope = CoroutineScope(SupervisorJob() + Dispatchers.IO).also { scopes += it },
        ).also { hubs += it }
        runBlocking { withTimeout(30_000) { hub.boot() } }
        val words = MultisigWords(
            strings = strings,
            localNames = world.locals.associate { world.fixtures[it].address.lowercase() to world.fixtures[it].name },
            networkName = { mapOf(100 to "Gnosis", 8453 to "Base")[it] },
            nativeSymbol = { mapOf(100 to "XDAI", 8453 to "ETH")[it] },
        )
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    MultisigHost(
                        hub,
                        MultisigHostContext(
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
                        ),
                    )
                }
            }
        }

        // A site's wallet_sendCalls: changeThreshold(1) on the multisig itself.
        val calls = """[{"version":"2.0.0","chainId":"0x64","from":"$safe","calls":[{"to":"$safe","value":"0x0","data":"0x694e80c3${"0".repeat(63)}1"}]}]"""
        val answers = CopyOnWriteArrayList<JsonElement>()
        compose.runOnIdle {
            hub.dappRequest("r21-1", safe, "wallet_sendCalls", Wire.json.parseToJsonElement(calls), "https://app.example", 100) { answers += it }
        }
        val refusal = eventually("refused at arrival") {
            hub.proposalsView.value.dapp["r21-1"].takeIf { it["stage"].str == "refused" }?.get("refusal")
        }
        assertEquals("self_call", refusal["data"]["reason"].str)
        assertEquals("changeThreshold", refusal["data"]["function"].str)
        assertEquals("componentsUi.signing.multisigSelfCallBlocked", refusal["data"]["key"].str)
        assertNull("nothing drafted", hub.proposalsView.value.draft)
        assertTrue("nothing signed", world.passkeys.isEmpty())

        // The sheet: its title and the catalog's sentence, from this APK's catalog.
        compose.waitUntil(30_000) { present("multisig.dapp.refused") }
        val sentence = strings.t("componentsUi.signing.multisigSelfCallBlocked", mapOf("function" to "changeThreshold"))
        assertTrue(sentence, sentence.contains("changeThreshold") && sentence.contains("Nothing was proposed"))
        assertTrue("the sentence is on the sheet", shown("Vela never lets a site do that"))
        assertTrue("its title is on the sheet", shown(strings.t("componentsUi.signing.selfCallBlockedTitle")))

        // Read, then left: the site hears -32603, never 4001.
        compose.runOnIdle { hub.dappLeave("r21-1") }
        val answer = eventually("answered") { answers.firstOrNull() }
        assertEquals(-32603L, answer["error"]["code"].long)
    }

    @Test
    fun thisDebugBuildOnThisEmulatorUsesLocalhostPasskeys() {
        assertTrue("this AVD reads as an emulator", RelyingParty.isEmulator())
        assertTrue("a debug build is a test build", BuildConfig.DEBUG)
        assertEquals(RelyingParty.Answer.Id("localhost"), RelyingParty.current)
    }

    @Test
    fun thePhonesCoreStampsTheWindowAndRefusesAnOperationThatNeverExpires() {
        val sender = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val draft = userOpDraft(
            sender = sender, nonce = "0x1", deployed = true, keyHexes = listOf("04" + "11".repeat(64)),
            calls = listOf(UserOpCall(to = "0x031d7D57c99CAF891e1C250554691Fd12D84772b", value = "1", data = "0x")),
            fee = UserOpFeeMode.InBand(gasFeeToken = null, amount = "1", recipient = sender),
            floors = userOpFloors(chainId = 100u, deployed = true, subCalls = 2u),
        )
        assertTrue("an unstamped operation is never hashed", runCatching { userOpSafeOpHash(draft, 100u) }.isFailure)
        val now = System.currentTimeMillis()
        val stamped = userOpStampValidity(draft, now.toDouble())
        assertEquals(0uL, stamped.validAfter)
        assertEquals((now / 1000 + 1800).toULong(), stamped.validUntil)
        assertEquals(32, userOpSafeOpHash(stamped, 100u).size)
    }

    private companion object {
        val NETWORKS = listOf(100, 8453)
    }
}
