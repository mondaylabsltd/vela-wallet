package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.platform.LaunchIntent
import app.getvela.wallet.feature.multisig.LookAsk
import app.getvela.wallet.feature.multisig.MultisigReach
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MultisigOperation
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.MultisigShellResult
import app.getvela.wallet.feature.multisig.core.ShownFee
import app.getvela.wallet.feature.multisig.pricedFee
import java.io.File
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Round 14's Android findings, at the shell's pure seams (docs/multisig-machines.md §20). */
class R14AndroidFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val interleave = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(interleave.lowercase() to "Interleave"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { if (it == 5042002) "USDC" else null },
    )

    @After fun clean() = ShownFee.forget()

    // -- r14-6: the fee line recovers by itself, and offers Check again ------------

    @Test
    fun `a look that could not price it is asked again slowly after the first few, never a priced one`() {
        for (kind in listOf(null, "unpriced", "unchecked")) {
            assertTrue(LookAsk.timerAsks(kind, 0, 0))
            assertFalse("not again at once past the first few", LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, LookAsk.ASK_EVERY_MS))
            assertTrue("again once the slow pace is up", LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, LookAsk.SLOW_EVERY_MS))
        }
        for (kind in listOf("priced", "fails", "refused", "spent")) {
            assertFalse(kind, LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, 10 * LookAsk.SLOW_EVERY_MS))
        }
        // 3.5 minutes of "Couldn't check whether it would run" (r14-6) is asked
        // again many times — the line comes back on its own once the network does.
        var asks = 0
        var last = 0L
        var clock = 0L
        while (clock < 210_000) {
            clock += LookAsk.ASK_EVERY_MS
            if (LookAsk.timerAsks("unchecked", asks, clock - last)) { asks += 1; last = clock }
        }
        assertTrue("asked $asks times", asks >= LookAsk.AUTO_LIMIT + 8)
    }

    @Test
    fun `Check again is offered under a fee that was not worked out or checked`() {
        assertTrue(LookAsk.offersCheckAgain("unchecked"))
        assertTrue(LookAsk.offersCheckAgain("unpriced"))
        assertFalse(LookAsk.offersCheckAgain("priced"))
        assertFalse(LookAsk.offersCheckAgain(null))
    }

    // -- r14-9: the fee signed is the fee shown -----------------------------------

    private fun json(text: String) = Wire.json.parseToJsonElement(text) as JsonObject

    private fun execution(amount: String) = MultisigOperation.SendCalls(
        chain_id = 5042002,
        from = interleave,
        calls = emptyList(),
        fee = json("""{"amount":"$amount","recipient":"0x1111111111111111111111111111111111111111"}"""),
        purpose = json("""{"kind":"execute","multisig":"0xE1F2","threshold":2,"tx":{}}"""),
    )

    @Test
    fun `the fee pressed on is the one a priced look says`() {
        assertEquals("98172000000000000", pricedFee(json("""{"kind":"priced","fee":{"amount":"98172000000000000"}}""")))
        assertNull(pricedFee(json("""{"kind":"unchecked"}""")))
        assertNull(pricedFee(null))
    }

    @Test
    fun `an execution whose fee moved since the press signs nothing and says the new fee`() {
        // ≈0.098172 on screen when pressed; the machine took a new look (the
        // old one past its 15 s) and would sign 0.0972.
        ShownFee.pressed(5042002, interleave, "98172000000000000")
        val stopped = ShownFee.refusal(execution("97200000000000000"))
        assertTrue(stopped is MultisigShellResult.CallsFailed)
        stopped as MultisigShellResult.CallsFailed
        assertFalse("nothing left the device", stopped.maybe_sent)
        val said = words.failureText(stopped.detail)
        assertEquals("That didn't go through: Network fee ≈ 0.0972 USDC, paid by Interleave", said)
        // Pressed on once: the next press records its own figure.
        assertNull(ShownFee.refusal(execution("97200000000000000")))
    }

    @Test
    fun `the same fee goes on, in either spelling`() {
        ShownFee.pressed(5042002, interleave.lowercase(), "97200000000000000")
        assertNull(ShownFee.refusal(execution("0x" + java.math.BigInteger("97200000000000000").toString(16))))
    }

    @Test
    fun `no fee said, or not an execution, is not held to one`() {
        assertNull(ShownFee.refusal(execution("1")))
        ShownFee.pressed(5042002, interleave, "5")
        val record = execution("6").copy(purpose = json("""{"kind":"record","multisig":"0xE1F2"}"""))
        assertNull(ShownFee.refusal(record))
        ShownFee.pressed(5042002, interleave, null)
        assertNull(ShownFee.refusal(execution("6")))
    }

    @Test
    fun `the fee moved failure reads back as the shell wrote it`() {
        val detail = MultisigShellFailure.FeeMoved("1", 5042002, interleave).detail
        assertEquals("fee_moved", (MultisigShellFailure.read(detail)?.get("kind") as JsonPrimitive).content)
    }

    // -- r14-2: the launch intent, acted on once ----------------------------------

    @Test
    fun `an activity's starting intent is acted on only on its first creation`() {
        assertTrue(LaunchIntent.firstDelivery(noSavedState = true, flags = 0))
        assertFalse("recreated: a restored task or a configuration change", LaunchIntent.firstDelivery(noSavedState = false, flags = 0))
        assertFalse("reopened from Recents", LaunchIntent.firstDelivery(noSavedState = true, flags = LaunchIntent.LAUNCHED_FROM_HISTORY))
        assertEquals(android.content.Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY, LaunchIntent.LAUNCHED_FROM_HISTORY)
    }

    // -- r14-2 / r14-6: asked again when a network answers again ------------------

    @Test
    fun `a network the pool lets go of, or the device back online, is a reason to read again`() {
        assertEquals(setOf(5042002), MultisigReach.back(setOf(5042002, 1), setOf(1), wasOnline = true, online = true))
        assertNull("nothing came back", MultisigReach.back(setOf(5042002), setOf(5042002), wasOnline = true, online = true))
        assertNull("a network newly out of reach is no reason", MultisigReach.back(emptySet(), setOf(5042002), wasOnline = true, online = true))
        assertEquals("the device back: every network", emptySet<Int>(), MultisigReach.back(setOf(5042002), setOf(5042002), wasOnline = false, online = true))
        assertNull("offline: nothing to ask", MultisigReach.back(setOf(5042002), emptySet(), wasOnline = true, online = false))
    }
}
