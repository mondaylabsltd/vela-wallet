package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The receive screens.
 *
 * **This is the one screen in the app where a stale value is unrecoverable.**
 * Every other fixture that leaks shows somebody the wrong information; a
 * fixture address on a receive QR sends their money to a stranger, and no part
 * of the system can undo it. Every case here is about that.
 */
class FlowLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "public/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private val mine = "0x7687AbCdEf0123456789abcdefABCDEF0123D141"

    private fun qrFixture() =
        (FlowFixtures.build(FlowState.R2, strings).sheet as FlowSheet.ReceiveQr).model

    private fun listFixture() =
        (FlowFixtures.build(FlowState.R1, strings).base as FlowBase.Receive).model

    private fun row(chainId: Long, name: String, symbol: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
    )

    @Test
    fun `the QR card shows this person's address, not the drawn one`() {
        val drawn = qrFixture()
        // The fixture's address is somebody else's, and it is a real-looking
        // one — which is exactly why this cannot be left to a glance.
        assertTrue(drawn.account.lines.first.startsWith("0x"))

        val live = FlowLive.receiveQr(drawn, mine, "Me", PaymentRequestView())

        assertEquals(mine, live.account.lines.first + live.account.lines.second)
        assertEquals("Me", live.account.name)
    }

    /**
     * The identicon is drawn FROM the address.
     *
     * A stale seed beside a live address is a mismatch nobody can interpret —
     * and the face is precisely what people use to check at a glance that they
     * are looking at their own wallet.
     */
    @Test
    fun `the face is drawn from the address on screen`() {
        val live = FlowLive.receiveQr(qrFixture(), mine, "Me", PaymentRequestView())

        assertEquals(mine, live.account.identiconSeed)
    }

    /**
     * **No session, no address.**
     *
     * Not "keep the fixture until we have one". A blank card is a screen that
     * cannot be acted on; a fixture card is a screen that can.
     */
    @Test
    fun `an empty session shows no address at all`() {
        val live = FlowLive.receiveQr(qrFixture(), "", "", PaymentRequestView())

        assertEquals("" to "", live.account.lines)
        assertEquals(WalletFixtures.ADDRESS_FULL != "", true) // the fixture HAS one
        assertFalse(
            "a blank session must not fall back to the drawn address",
            live.account.lines.first.startsWith("0x"),
        )
    }

    /** The two lines are the whole address, split — never a shortened one. */
    @Test
    fun `the card carries the full address across its two lines`() {
        val live = FlowLive.receiveQr(qrFixture(), mine, "Me", PaymentRequestView())

        val rejoined = live.account.lines.first + live.account.lines.second
        assertEquals(42, rejoined.length)
        assertFalse("a copy button must not put an ellipsis on the clipboard", rejoined.contains("…"))
    }

    // -- the warning gate -----------------------------------------------------

    @Test
    fun `the QR stays covered until the gate has both loaded and been accepted`() {
        // While loading, a first visit would otherwise flash the code before
        // the warning about which networks this address is safe on.
        assertFalse(FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = true)))
        assertFalse(
            FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = false, acknowledged = false)),
        )
        assertTrue(
            FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = false, acknowledged = true)),
        )
    }

    // -- the network list -----------------------------------------------------

    @Test
    fun `the network list is this device's, with this address on every row`() {
        val view = NetView(
            loaded = true,
            networks = listOf(row(1, "Ethereum", "ETH"), row(137, "Polygon", "POL")),
        )

        val live = FlowLive.receiveNetworks(listFixture(), view, mine, WalletLive::badge)

        assertEquals(listOf("Ethereum", "Polygon"), live.rows.map { it.name })
        assertEquals(listOf("ETH", "POL"), live.rows.map { it.code })
        // One address across every network — the thing the subtitle claims.
        assertEquals(1, live.rows.map { it.addressDisplay }.distinct().size)
        assertTrue(live.rows[0].addressDisplay.startsWith("0x7687"))
        assertTrue(live.rows[0].addressDisplay.endsWith("D141"))
    }

    @Test
    fun `the subtitle counts the networks this device has`() {
        val view = NetView(loaded = true, networks = listOf(row(1, "Ethereum", "ETH")))

        val live = FlowLive.receiveNetworks(listFixture(), view, mine, WalletLive::badge)

        // The fixture says "across all 8 networks" beside a live list; a count
        // that disagrees with the rows under it is the 040 bug in a new place.
        assertTrue("the count follows the list", live.subtitle.contains("1"))
        assertFalse(live.subtitle.contains("8"))
    }

    @Test
    fun `an unloaded network list leaves the screen alone`() {
        val drawn = listFixture()

        val live = FlowLive.receiveNetworks(drawn, NetView(), mine, WalletLive::badge)

        // Before storage answers there is nothing truer to show; blanking for a
        // frame is a flicker a person reads as a bug.
        assertEquals(drawn.rows, live.rows)
    }
}
