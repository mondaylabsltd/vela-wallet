package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.rejectFailureLine
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.UnreachableNetwork
import java.io.File
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 16's Android findings (docs/multisig-machines.md §26):
 *
 * - r16-4: the one press does both or nothing, and a stop is said beside it.
 * - r16-5: a Reject that could not be drafted says why beside it, with Try again.
 * - r16-12: Settings → Switch account wears one check.
 * - r16-13: a figure not read this round is said as last seen.
 * - r16-14: while the review is not read the button and its note stand still.
 * - r16-15: the one-time setup is one block, its head and its lines together.
 */
class R16NativeFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private val sheetSource by lazy {
        File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigProposalSheet.kt").readText()
    }

    // -- the proposal sheet's model ---------------------------------------------------------

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val safe = "0x7f3cE90000000000000000000000000000A0de87"
    private val hash = "0x" + "cd".repeat(32)

    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Interleave"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
    )

    private val readyReview = """{"sign":{"allowed":true,"anyway":false},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""

    /**
     * Alice's approval would complete it. [state]: the review's state (`null`:
     * not asked yet), [look]: her look, [oneTap]: the core offers her the one
     * press, [execute]: the `execute:` action's state.
     */
    private fun proposal(state: String?, look: String?, oneTap: Boolean, execute: String? = null): MsProposalModel {
        val reviews = state?.let { """"reviews":{"5042002:$hash":${if (it.contains("ready")) readyReview else "null"}},"review_states":{"5042002:$hash":$it},""" }.orEmpty()
        val previews = look?.let { """"previews":{"5042002:$hash:${alice.lowercase()}":$it},""" }.orEmpty()
        val actions = execute?.let { """"actions":{"execute:$hash":$it},""" }.orEmpty()
        val view = MsProposalsView.of(
            json(
                """
                {"followed":"$safe","standings":{},$reviews$previews$actions
                 "approve_execute":{"5042002:$hash":${if (oneTap) "[\"$alice\"]" else "[]"}},
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"0","proposer":"$bob","approvals":[{"owner":"$bob"}],"approved":1,"threshold":2,
                  "status":"pending","canApprove":["$alice"],"canExecute":["$alice"],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":false,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$bob","value":"0","data":"0x"},"sign":{"allowed":true,"anyway":false}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":false,"owners":["$alice","$bob"],"threshold":2,"nonce":"0"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
        return MsProposalModel(view.items.single(), view, words, null)
    }

    private val ready = """{"kind":"ready"}"""
    private val unread = """{"kind":"failed","reason_key":"home.multisig.review.state.chainUnread"}"""
    private val priced = """{"kind":"priced","fee":{"amount":"120000"},"balance":"900000","canPay":true}"""
    private val unchecked = """{"kind":"unchecked"}"""
    private val unpriced = """{"kind":"unpriced"}"""

    // -- r16-4 ----------------------------------------------------------------------------

    @Test
    fun `the one press settles on its execution alone and never waits for an approval behind it`() {
        // The core does both or nothing (§26): the press reads the execution's
        // outcome; nothing approves after it.
        val onePress = sheetSource.substringAfter("if (oneTap) {").substringBefore("val settled = hub.proposals.request(model.approveEvent(")
        assertFalse("no wait for an approval the one press never gives", onePress.contains("settledOf(v, \"approve\")"))
        assertTrue(onePress.contains("model.executeEvent(owner, oneTap = true)"))
    }

    @Test
    fun `a one press stopped for a fee it could not work out says so beside it, with Check again as its way on`() {
        val stop = """{"kind":"stopped","preview":$unpriced}"""
        val m = proposal(ready, unpriced, oneTap = true, execute = stop)
        assertTrue(m.oneTapShown(alice))
        val failure = m.executeFailure!!
        assertEquals(words.t("proposal.feeUnpriced"), failure.text)
        assertEquals(MsTone.Warning, failure.tone)
        // Said once: the stop stands in the fee line's place, and its own way on
        // is Check again (a new look) — no second "Check again" under it.
        assertTrue(m.executionLines(alice, failure).none { it.id == "fee" })
        assertTrue(m.failureChecksFee)
        assertFalse(m.oneTapCheckAgain(alice))
        // A look that priced it since lifts the stop in the core; with no stop,
        // the look's own Check again is offered while it is unpriced.
        assertTrue(proposal(ready, unpriced, oneTap = true).oneTapCheckAgain(alice))
        assertFalse(proposal(ready, priced, oneTap = true).oneTapCheckAgain(alice))
    }

    @Test
    fun `beside the one press the approval alone is offered, as on the web and iOS`() {
        assertTrue(proposal(ready, priced, oneTap = true).approveAloneBeside(alice))
        assertFalse("no one press, no second button", proposal(ready, """{"kind":"fails","why":{"kind":"reverts"}}""", oneTap = false).approveAloneBeside(alice))
        assertTrue(sheetSource.contains("multisig.proposal.approveOnly\""))
    }

    @Test
    fun `while this device executes it the chip says Executing, never the mailbox's count`() {
        // r16-8 (iOS's finding; the same chip here).
        val working = proposal(ready, priced, oneTap = true, execute = """{"kind":"working"}""")
        assertEquals(words.t("status.executing"), working.statusLine)
        assertTrue(proposal(ready, priced, oneTap = true).statusLine != words.t("status.executing"))
    }

    // -- r16-5 ----------------------------------------------------------------------------

    @Test
    fun `a Reject that could not be drafted says why beside it`() {
        val unreadReject = json("""{"kind":"unread","reason_key":"home.multisig.proposal.unread.chain"}""")
        val line = rejectFailureLine(words, unreadReject, 5042002)!!
        assertEquals(words.unreadText("home.multisig.proposal.unread.chain", 5042002), line.text)
        assertTrue(line.text.contains("Arc Testnet"))
        assertEquals(words.t("proposal.mailboxDown"), rejectFailureLine(words, json("""{"kind":"mailboxUnreadable"}"""), 5042002)?.text)
        // Nothing to say for the person's own withdrawal, or a cancellation proposed.
        assertNull(rejectFailureLine(words, json("""{"kind":"cancelled"}"""), 5042002))
        assertNull(rejectFailureLine(words, json("""{"kind":"done","safe_tx_hash":"$hash","kept":false}"""), 5042002))
        assertNull(rejectFailureLine(words, null, 5042002))
        // The person's own passkey "cancel" is their word, never a failure (iOS's rule).
        val closed = """{"kind":"passkey","failure":"cancelled"}""".replace("\"", "\\\"")
        assertNull(rejectFailureLine(words, json("""{"kind":"failed","detail":"$closed"}"""), 5042002))
        // Drawn beside Reject, with Try again.
        assertTrue(sheetSource.contains("multisig.proposal.rejectFailed"))
        assertTrue(sheetSource.contains("multisig.proposal.rejectRetry"))
    }

    // -- r16-12 ---------------------------------------------------------------------------

    @Test
    fun `Settings Switch account wears one check while a multisig is in front`() {
        val base = SettingsFixtures.buildState(app.getvela.wallet.feature.settings.SettingsScreenState.ST2, strings)
        val accounts = listOf("Main" to alice, "Other" to bob)
        val switcher = BalanceSwitcherView()
        val usd = CurrencyView(code = "USD", committed = true)
        val shown = SettingsLive.withAccounts(base, accounts, 0, switcher, usd, strings, multisigShown = true).accountsSheet
        assertTrue("no Vela account row is checked", shown.rows.none { it.selected })
        val plain = SettingsLive.withAccounts(base, accounts, 0, switcher, usd, strings).accountsSheet
        assertEquals(listOf(true, false), plain.rows.map { it.selected })
        val host = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/navigation/VelaNavHost.kt").readText()
        assertTrue("the live Settings says whether a multisig is in front", host.contains("multisigShown = settingsMultisigShown"))
    }

    // -- r16-13 ---------------------------------------------------------------------------

    private val usdc = BalanceToken(chain_id = 5042002, symbol = "USDC", name = "USDC", balance = "0.827013", decimals = 6, token_address = null, price_usd = 1.0)

    private fun home(view: BalanceView, noValue: String?) = WalletLive.home(
        WalletFixtures.buildMobileState(WalletScreenState.H1, strings),
        view,
        FeedView(),
        CurrencyView(code = "USD", committed = true),
        strings,
        mapOf(5042002 to "Arc Testnet"),
        noValue = noValue,
        shownAccount = alice,
    )

    @Test
    fun `a holding its network did not answer is said as last seen, in the testnet space too`() {
        val arcDown = BalanceView(
            address = alice, tokens = listOf(usdc), failed_chain_ids = listOf(5042002),
            unreachable_networks = listOf(UnreachableNetwork(5042002, "held", 0.827013, "assets.lastSeen")),
        )
        val testnet = home(arcDown, noValue = "No real value").assetRows.single()
        assertTrue(testnet.stale)
        assertEquals(strings.t("assets.lastSeen", mapOf("amount" to "0.827013 USDC")), testnet.balance)
        val mainnet = home(arcDown, noValue = null).assetRows.single()
        assertTrue(mainnet.stale)
        assertEquals("0.827013 USDC", mainnet.balance)
        assertEquals(strings.t("assets.lastSeen", mapOf("amount" to "$0.83")), (mainnet.fiat as AssetFiatModel.Value).text)
        // Read this round: the figure is current.
        val read = home(BalanceView(address = alice, tokens = listOf(usdc)), noValue = "No real value").assetRows.single()
        assertFalse(read.stale)
        assertEquals("0.827013 USDC", read.balance)
    }

    @Test
    fun `rows kept from last session are last seen until the first read of the account ends`() {
        // A cold start with Arc hung: the restored rows under "Checking…".
        val checking = BalanceView(address = alice, tokens = listOf(usdc), checking_key = "componentsUi.funding.checking")
        val row = home(checking, noValue = "No real value").assetRows.single()
        assertTrue(row.stale)
        assertEquals(strings.t("assets.lastSeen", mapOf("amount" to "0.827013 USDC")), row.balance)
    }

    // -- r16-14 ---------------------------------------------------------------------------

    @Test
    fun `while the review is not read the button reads Approve whatever the looks say, and says nothing of the fee`() {
        for (state in listOf(null, unread, """{"kind":"loading"}""")) {
            for (look in listOf(null, unchecked, unpriced, priced)) {
                val m = proposal(state, look, oneTap = look != null)
                assertFalse("state=$state look=$look", m.canSign)
                assertFalse("the label stays Approve: state=$state look=$look", m.oneTapShown(alice))
                assertFalse(m.saysExecution)
                assertNull(m.approveOnlyWhy(alice).takeIf { m.saysExecution })
                assertFalse(m.oneTapCheckAgain(alice))
            }
        }
        // Read: the one press and its lines.
        val read = proposal(ready, unchecked, oneTap = true)
        assertTrue(read.oneTapShown(alice))
        assertTrue(read.saysExecution)
        // The sheet draws the fee lines only while a press can sign.
        assertTrue(sheetSource.contains("if (oneTap && chosen != null && model.saysExecution)"))
    }

    // -- r16-15 ---------------------------------------------------------------------------

    @Test
    fun `the one-time setup is one block - its head and its lines drawn together`() {
        val withSetup = """{"kind":"priced","fee":{"amount":"120000"},"balance":"900000","canPay":true,"setup":{"multisig":true,"members":[],"executor":false,"amount":"21929"}}"""
        val lines = proposal(ready, withSetup, oneTap = true).executionLines(alice, null)
        assertEquals(listOf("fee", "setup0", "setup1"), lines.map { it.id })
        // Drawn through one block, not one note per line (the sheet's spacing
        // between notes put a gap under the head's colon).
        assertTrue(sheetSource.contains("ExecutionLines(model.executionLines("))
        assertEquals(0, Regex("""model\.executionLines\([^)]*\)\.forEach""").findAll(sheetSource).count())
    }
}
