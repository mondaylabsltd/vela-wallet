package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MultisigNotifier
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MultisigHub
import java.io.File
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 11's Android shell fixes, read through the same builders the screens
 * and the background round use — the core's views in (as JSON), what the app
 * says and draws out.
 */
class R11ShellFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val msA = "0x555495eA72069b6e6c8b1fA0bC5dE4b0c7A23cC3"
    private val msB = "0xD2750000000000000000000000000000000068D3"

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private fun words() = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Interleave"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { if (it == 5042002) "USDC" else null },
    )

    private fun tx(n: Int) = "0x" + n.toString(16).padStart(64, '0')
    private fun key(n: Int) = "5042002:${tx(n)}"

    private val accounts = MsAccountsView.of(
        json(
            """{"loaded":true,"active":"$msB","accounts":[
              {"account":{"address":"$msA","owners":["$alice","$bob"],"threshold":2,"chainIds":[5042002],"label":"R11 Team"},"name":"R11 Team","shared":true,"record_waiting":false,"local_members":["$alice"]},
              {"account":{"address":"$msB","owners":["$alice","$bob"],"threshold":2,"chainIds":[5042002],"label":"R11 Adv"},"name":"R11 Adv","shared":true,"record_waiting":false,"local_members":["$alice"]}]}""",
        ),
    )

    /** The proposals machine's view: MS-B followed (nothing waits), MS-A's waiting [needs] as read at [readAt] (`null`: kept). */
    private fun view(needs: List<Int>, readAt: Long?, followedReadAt: Long? = readAt): MsProposalsView {
        fun waiting(list: List<Int>, at: Long?) =
            """{"needs_you":${list.size},"ready":0,"waiting_payer":0,"first":${list.firstOrNull()?.let { "{\"chain_id\":5042002,\"safe_tx_hash\":\"${tx(it)}\"}" } ?: "null"},
               "needs_you_keys":[${list.joinToString { "\"${key(it)}\"" }}],"ready_keys":[],"read_at":${at ?: "null"}}"""
        return MsProposalsView.of(
            json(
                """{"followed":"$msB","items":[],"loading":false,"presence":{
                  "${msA.lowercase()}":${waiting(needs, readAt)},
                  "${msB.lowercase()}":${waiting(emptyList(), followedReadAt)}}}""",
            ),
        )
    }

    // -- r11-1: the notifier's race ------------------------------------------------------

    /**
     * r11-1 (Android, round 11): the app put away, Eve proposed #2 between
     * the leaving round's evaluation and its mailbox read landing. The round
     * recorded #2 as already said and never said it; the debug rounds after
     * said notes=0; #3 was then said as "1 needs your approval" while 2 waited.
     *
     * Now "already said" is what was said or seen, by transaction: the
     * round's evaluation of a list it did not read records nothing; the read
     * landing late is said when it lands; a cold process says what its own
     * read holds and nothing twice; and the count is the number waiting.
     */
    @Test
    fun a_proposal_posted_while_the_round_reads_is_said_once_with_the_real_count() = runBlocking {
        val store = FakeStore()
        val posted = mutableListOf<MultisigNotifier.Note>()
        val notifier = MultisigNotifier(store, { words() }) { posted += it }

        // On screen: #1 waits (seen by the person) — recorded, nothing said.
        assertTrue(notifier.evaluate(accounts, view(listOf(1), readAt = 1_000), MultisigNotifier.Look.OnScreen).isEmpty())

        // Home at 2 000; the leaving round begins at 2 100. Its evaluation
        // comes before its read: the list it holds was read at 1 000.
        val leaving = MultisigNotifier.Look.Away(since = 2_100)
        assertTrue(notifier.evaluate(accounts, view(listOf(1), readAt = 1_000), leaving).isEmpty())
        // Eve posts #2 at 2 300; the round's read lands after it: said as it lands.
        val landed = notifier.evaluate(accounts, view(listOf(1, 2), readAt = 2_200), leaving)
        assertEquals(1, landed.size)
        assertEquals(MultisigNotifier.Channel.NeedsYou, landed.single().channel)
        assertEquals("2 need your approval", landed.single().body)
        assertEquals("R11 Team", landed.single().title)

        // The process killed; a debug trigger's round in a cold one. The
        // store is all that survives. First the kept list (read by nobody
        // here), then its own read: #2 was said — nothing twice.
        val cold = MultisigNotifier(store, { words() }) { posted += it }
        val trigger = MultisigNotifier.Look.Away(since = 10_000)
        assertTrue(cold.evaluate(accounts, view(listOf(1, 2), readAt = null), trigger).isEmpty())
        assertTrue(cold.evaluate(accounts, view(listOf(1, 2), readAt = 10_050), trigger).isEmpty())

        // #3 posted: said, with the number waiting (3), not the number new.
        val third = cold.evaluate(accounts, view(listOf(1, 2, 3), readAt = 20_050), MultisigNotifier.Look.Away(since = 20_000))
        assertEquals("3 need your approval", third.single().body)

        // A count that stays the same says nothing of WHICH: #1 approved
        // elsewhere and #4 new is still three waiting — #4 is said.
        val swapped = cold.evaluate(accounts, view(listOf(2, 3, 4), readAt = 30_050), MultisigNotifier.Look.Away(since = 30_000))
        assertEquals(1, swapped.size)
        assertEquals("3 need your approval", swapped.single().body)
        assertEquals(listOf(landed.single(), third.single(), swapped.single()), posted)
    }

    /** r11-1: one note per multisig — a newer count replaces the older one, never a second banner for the same multisig. */
    @Test
    fun a_multisigs_note_is_replaced_not_stacked() = runBlocking {
        val notifier = MultisigNotifier(FakeStore(), { words() }) {}
        val first = notifier.evaluate(accounts, view(listOf(1), readAt = 200), MultisigNotifier.Look.Away(since = 100)).single()
        val second = notifier.evaluate(accounts, view(listOf(1, 2), readAt = 400), MultisigNotifier.Look.Away(since = 300)).single()
        assertEquals(first.id, second.id)
        assertEquals("ms-needs-${msA.lowercase()}", second.id)
    }

    /** r11-1: a round waits for ITS reads — every multisig's list read since it began, the followed one not loading. */
    @Test
    fun a_round_is_read_only_when_every_list_is_its_own() {
        assertFalse("MS-A's list is kept", MultisigHub.Round.read(accounts, view(listOf(1), readAt = null, followedReadAt = 600), since = 500))
        assertFalse("MS-A's read began before the round", MultisigHub.Round.read(accounts, view(listOf(1), readAt = 400, followedReadAt = 600), since = 500))
        assertFalse("the followed one is not read yet", MultisigHub.Round.read(accounts, view(listOf(1), readAt = 600, followedReadAt = 400), since = 500))
        assertTrue(MultisigHub.Round.read(accounts, view(listOf(1), readAt = 600, followedReadAt = 650), since = 500))
    }

    // -- r11-3: a multisig whose money lives in the other space ----------------------------

    /**
     * r11-3 (Android, iOS; the desktop's r10-4): an Arc-only multisig added
     * with Mainnet in front read "$0.00", "No activity yet", "Updated now".
     * Where it lives is said from the record's networks and the core's word
     * on which space each is in; the home draws the note in place of the
     * figures (`WalletScreen.otherSpace`).
     */
    @Test
    fun a_multisig_used_only_in_the_other_space_says_where_it_lives() {
        val mainnet = listOf(1, 100, 8453)
        val testnet = listOf(5042002, 11155111)
        val front = app.getvela.wallet.feature.settings.core.NetworkSpace.Mainnet
        // r13-1: the rule is the core's (`network_lives_elsewhere`); the line is drawn from its keys.
        val elsewhere = requireNotNull(app.getvela.wallet.feature.multisig.OtherSpace.livesElsewhere(listOf(5042002), here = mainnet, other = testnet, front = front))
        assertEquals(listOf(5042002), elsewhere.chain_ids)
        assertEquals(app.getvela.wallet.feature.settings.core.NetworkSpace.Testnet, elsewhere.target)
        assertEquals(
            "Used on Arc Testnet · Testnet space",
            app.getvela.wallet.feature.multisig.OtherSpace.line(words(), elsewhere, strings.t(elsewhere.heading_key)),
        )
        assertEquals("Switch to the testnet space", strings.t(elsewhere.switch_key))
        // One of its networks is here: the home as it is.
        assertNull(app.getvela.wallet.feature.multisig.OtherSpace.livesElsewhere(listOf(5042002, 100), here = mainnet, other = testnet, front = front))
        // A network in neither list (taken off the person's lists): nothing to switch to.
        assertNull(app.getvela.wallet.feature.multisig.OtherSpace.livesElsewhere(listOf(777), here = mainnet, other = testnet, front = front))
        assertNull(app.getvela.wallet.feature.multisig.OtherSpace.livesElsewhere(emptyList(), here = mainnet, other = testnet, front = front))
    }

    // -- r11-7: a dead RPC in the testnet space ------------------------------------------------

    /**
     * r11-7: the RPC dead while the app was up — the stamp aged ("Updated 3m")
     * and no line said why (the web: "Can't reach Arc Testnet right now").
     * The testnet space's hero has no figure, and still says the core's
     * unreachable list.
     */
    @Test
    fun the_testnet_hero_says_the_network_it_cannot_reach() {
        val view = app.getvela.wallet.feature.wallet.core.BalanceView(
            address = "0xabc",
            tokens = listOf(app.getvela.wallet.feature.wallet.core.BalanceToken(chain_id = 5042002, symbol = "USDC", name = "USDC", balance = "2", decimals = 18, price_usd = 1.0)),
            cached_total_usd = 2.0,
            last_refreshed_at_ms = 1.0,
            unreachable_networks = listOf(app.getvela.wallet.feature.wallet.core.UnreachableNetwork(5042002, "held", 2.0, "assets.lastSeen")),
            unreachable_key = "assets.unreachableOne",
        )
        val home = app.getvela.wallet.feature.wallet.WalletLive.home(
            app.getvela.wallet.feature.wallet.WalletFixtures.buildMobileState(app.getvela.wallet.feature.wallet.WalletScreenState.H1, strings),
            view, app.getvela.wallet.feature.wallet.core.FeedView(),
            app.getvela.wallet.feature.settings.core.CurrencyView(code = "USD", committed = true), strings, mapOf(5042002 to "Arc Testnet"),
            noValue = strings.t(app.getvela.wallet.core.i18n.I18nKeys.NetworkSpace.NO_VALUE),
        )
        assertEquals(app.getvela.wallet.feature.wallet.BalanceStateKind.NoValue, home.balance.state)
        assertEquals("Can't reach Arc Testnet right now", home.balance.status?.text)
        assertNull("never 'live' in a space with no value", home.balance.liveText)
        // Every network answered: nothing under the words.
        val fine = app.getvela.wallet.feature.wallet.WalletLive.home(
            app.getvela.wallet.feature.wallet.WalletFixtures.buildMobileState(app.getvela.wallet.feature.wallet.WalletScreenState.H1, strings),
            view.copy(unreachable_networks = emptyList(), unreachable_key = null, display_total_usd = 2.0), app.getvela.wallet.feature.wallet.core.FeedView(),
            app.getvela.wallet.feature.settings.core.CurrencyView(code = "USD", committed = true), strings, mapOf(5042002 to "Arc Testnet"),
            noValue = strings.t(app.getvela.wallet.core.i18n.I18nKeys.NetworkSpace.NO_VALUE),
        )
        assertNull(fine.balance.status)
    }

    /** r11-1: who is looking — resumed is the person; away needs a round to measure from; a sheet over the app is neither. */
    @Test
    fun who_is_looking() {
        assertEquals(MultisigNotifier.Look.OnScreen, MultisigNotifier.lookNow(onScreen = true, started = true, since = 5))
        assertEquals(MultisigNotifier.Look.Away(5), MultisigNotifier.lookNow(onScreen = false, started = false, since = 5))
        assertNull("no round in this process: nothing to measure from", MultisigNotifier.lookNow(onScreen = false, started = false, since = null))
        assertNull("paused under a system sheet: neither said nor seen", MultisigNotifier.lookNow(onScreen = false, started = true, since = 5))
    }
}
