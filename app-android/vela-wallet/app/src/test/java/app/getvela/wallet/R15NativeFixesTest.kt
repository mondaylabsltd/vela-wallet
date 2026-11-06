package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.importAlready
import app.getvela.wallet.feature.multisig.importRowStatus
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import java.io.File
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 15's Android findings (docs/multisig-machines.md §23):
 *
 * - r15-1: the home's Activity is the account in front's — never another
 *   account's rows, never "No activity yet" before its history was read.
 * - r15-7: an undeployed multisig's one press says its one-time setup, and a
 *   setup whose Safe contracts went unread is said there too.
 * - r15-8: a Safe imported by its row's Try again says "Imported".
 */
class R15NativeFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    // -- r15-1: Activity is the shown account's -----------------------------------------

    private val interleave = "0x88cCA0000000000000000000000000000266894"
    private val und = "0x7f3cE90000000000000000000000000000A0de87"

    private fun received(id: String) = FeedRow.Item(
        FeedItem(
            id = id, direction = FeedDirection.In, counterparty = "0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D",
            value = "0.3", symbol = "USDC", decimals = 6, usd_value = 0.3, chain_id = 5042002,
            timestamp = 1_790_000_000.0, day_start_ms = 1_789_948_800_000.0,
        ),
    )

    private fun feed(account: String, settled: Boolean, rows: List<FeedRow> = emptyList()): FeedView {
        val all = if (rows.isEmpty()) rows else listOf(FeedRow.Header(id = "day-1", day_start_ms = 1_789_948_800_000.0, timestamp = 1_790_000_000.0)) + rows
        return FeedView(rows = all, home_rows = all, account = account.lowercase(), settled = settled)
    }

    private fun home(feed: FeedView, shown: String?) = WalletLive.home(
        WalletFixtures.buildMobileState(WalletScreenState.H1, strings),
        BalanceView(address = shown ?: ""),
        feed,
        CurrencyView(code = "USD", committed = true),
        strings,
        emptyMap(),
        shownAccount = shown,
    )

    @Test
    fun `another account's rows are never drawn under the account in front`() {
        // The tester's frame: Interleave's "Received +0.3 USDC" rows under R15 Und.
        val model = home(feed(interleave, settled = true, rows = listOf(received("r1"), received("r2"))), shown = und)
        assertTrue("no row of another account", model.activityGroups.isEmpty())
        assertEquals("being read, never 'No activity yet'", SectionMode.Loading, model.activitySection.mode)
        // The same rows under their own account are drawn.
        val own = home(feed(interleave, settled = true, rows = listOf(received("r1"))), shown = interleave)
        assertEquals(SectionMode.Rows, own.activitySection.mode)
        assertEquals(1, own.activityGroups.single().rows.size)
        // Addresses compare without case.
        assertEquals(SectionMode.Rows, home(feed(interleave.uppercase().replace("0X", "0x"), settled = true, rows = listOf(received("r1"))), shown = interleave).activitySection.mode)
    }

    @Test
    fun `an empty Activity not looked at yet is being read, never 'No activity yet'`() {
        // The tester's other frame: Interleave's own home, its history unread.
        val unread = home(feed(interleave, settled = false), shown = interleave)
        assertEquals(SectionMode.Loading, unread.activitySection.mode)
        // Nothing told yet (a cold start before the feed hears the account).
        assertEquals(SectionMode.Loading, home(FeedView(), shown = interleave).activitySection.mode)
        // Read and scanned: the claim is the core's to make.
        val looked = home(feed(interleave, settled = true), shown = interleave)
        assertEquals(SectionMode.Empty, looked.activitySection.mode)
        assertEquals(strings.t("home.emptyNoActivity"), looked.activitySection.empty?.title)
        // Rows read before the scan came back are drawn.
        assertEquals(SectionMode.Rows, home(feed(interleave, settled = false, rows = listOf(received("r1"))), shown = interleave).activitySection.mode)
    }

    @Test
    fun `a multisig's unread history still says so over an Activity being read`() {
        val reading = home(feed(und, settled = false), shown = und)
        val said = WalletLive.withMultisigHistory(reading, WalletLive.MultisigHistory("unread", "Couldn't read this multisig's history", "Try again"))
        assertEquals(SectionMode.Empty, said.activitySection.mode)
        assertEquals("Try again", said.activitySection.empty?.retry)
    }

    @Test
    fun `the home tells the feed's account apart from the one shown`() {
        val source = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/navigation/VelaNavHost.kt").readText()
        // The live home names the account it shows; the fixture boards do not.
        assertTrue(source.contains("shownAccount = shownAddress"))
    }

    // -- r15-7: the one press says the one-time setup; an unread setup is said --------------

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

    private fun proposal(look: String, oneTap: Boolean): MsProposalModel {
        val view = MsProposalsView.of(
            json(
                """
                {"followed":"$safe","standings":{},
                 "previews":{"5042002:$hash:${alice.lowercase()}":$look},
                 "approve_execute":{"5042002:$hash":${if (oneTap) "[\"$alice\"]" else "[]"}},
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"0","proposer":"$bob","approvals":[{"owner":"$bob"}],"approved":1,"threshold":2,
                  "status":"pending","canApprove":["$alice"],"canExecute":["$alice"],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":false,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$bob","value":"0","data":"0x"}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":false,"owners":["$alice","$bob"],"threshold":2,"nonce":"0"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
        return MsProposalModel(view.items.single(), view, words, null)
    }

    private val pricedWithSetup =
        """{"kind":"priced","fee":{"amount":"156000000000000000"},"balance":"900000000000000000","canPay":true,"setup":{"multisig":true,"members":[],"executor":false,"amount":"40000000000000000"}}"""

    @Test
    fun `an undeployed multisig's one press says its one-time setup under the fee`() {
        val m = proposal(pricedWithSetup, oneTap = true)
        val lines = m.executionLines(alice, failure = null).map { it.text }
        assertEquals(words.t("proposal.feeLine", "fee" to "0.156 USDC", "name" to "Interleave"), lines.first())
        assertTrue("the setup's head: $lines", lines.any { it.startsWith("Includes one-time setup on Arc Testnet") })
        assertTrue(lines.contains(words.t("proposal.setupMultisig")))
        // Execute's lines are the same lines.
        assertEquals(lines, m.executionLines(alice, failure = null).map { it.text })
        // The sheet draws the one press's lines through the same function as Execute's.
        val sheet = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigProposalSheet.kt").readText()
        assertEquals("both the one press and Execute draw `executionLines`", 2, Regex("""model\.executionLines\(""").findAll(sheet).count())
    }

    @Test
    fun `a setup whose Safe contracts went unread is said where the one press would be`() {
        val unverified = """{"kind":"unverified","authority":{"kind":"unknown"}}"""
        val m = proposal(unverified, oneTap = false)
        val why = m.approveOnlyWhy(alice)
        assertEquals(words.t("authority.deployUnverified", "network" to "Arc Testnet"), why?.first)
        assertEquals(MsTone.Warning, why?.second)
        assertTrue(why!!.first.contains("couldn't check the Safe contracts"))
        // A setup read and refused is said as that (danger), never as unread.
        val refused = proposal("""{"kind":"refused","authority":{"kind":"refused","reasons":[{"kind":"module","address":"$bob"}]}}""", oneTap = false)
        assertEquals(MsTone.Danger, refused.approveOnlyWhy(alice)?.second)
        // A one press offered: its own fee line says it, nothing here.
        assertNull(proposal(pricedWithSetup, oneTap = true).approveOnlyWhy(alice))
    }

    // -- r15-8: a Safe imported by Try again says "Imported" ------------------------------

    @Test
    fun `a Safe imported by its row's Try again says Imported, never Already added`() {
        val imported = json("""{"kind":"imported"}""")
        // Added by this import (the row is here now): what this import came to.
        assertFalse(importAlready(here = true, outcome = imported))
        assertEquals(words.t("import.imported"), importRowStatus(words, here = true, outcome = imported))
        // Here before this sheet asked anything: already added.
        assertTrue(importAlready(here = true, outcome = null))
        assertEquals(words.t("import.already"), importRowStatus(words, here = true, outcome = null))
        // Busy first: said busy, with its network.
        val busy = json("""{"kind":"busy","chain_id":5042002}""")
        assertEquals(words.t("add.busy", "network" to "Arc Testnet"), importRowStatus(words, here = false, outcome = busy))
        assertNull(importRowStatus(words, here = false, outcome = null))
    }
}
