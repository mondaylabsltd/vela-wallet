package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.NetworkSwitchModel
import app.getvela.wallet.feature.scan.Eip681
import app.getvela.wallet.feature.wallet.core.PaymentRequestEvent
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import java.io.File
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.PaymentRequestCore

/**
 * Spec 090: the receive code's "include network" switch.
 *
 * What the code says is the core's (`payment_request`), so the cases drive the
 * real machine, the way `WalletController` does, and check that the sheet, the
 * saved card and Vela's own scanner all agree with it.
 */
class ReceiveNetworkSwitchTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private val me = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    private fun qrFixture() =
        (FlowFixtures.build(FlowState.R2, strings).sheet as FlowSheet.ReceiveQr).model

    private fun cardFixture() =
        (FlowFixtures.build(FlowState.R4, strings).base as FlowBase.Share).model

    /** The real machine, one event at a time. */
    private class Machine {
        private val core = PaymentRequestCore()
        fun send(event: PaymentRequestEvent): PaymentRequestView {
            val out = core.dispatch(Wire.json.encodeToString(PaymentRequestEvent.serializer(), event))
            val view = Wire.json.parseToJsonElement(out).jsonObject["view"] as JsonObject
            return Wire.json.decodeFromJsonElement(PaymentRequestView.serializer(), view)
        }
    }

    /** Started for [me], showing Gnosis — what a tapped Gnosis row does. */
    private fun onGnosis(machine: Machine): PaymentRequestView {
        machine.send(PaymentRequestEvent.Start(account = me, recipient = me, base_url = "https://getvela.app/pay"))
        return machine.send(PaymentRequestEvent.AssetPicked(chain_id = 100, token_address = null, symbol = "XDAI", decimals = 18, network_name = "Gnosis"))
    }

    @Test
    fun `the gallery draws the switch off`() {
        assertEquals(
            NetworkSwitchModel(strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK), isOn = false),
            qrFixture().network,
        )
    }

    @Test
    fun `off is the bare address, on names the network on screen, in the saved card and to the scanner`() {
        val machine = Machine()
        val off = onGnosis(machine)
        val offQr = FlowLive.receiveQr(qrFixture(), me, "Me", off, strings = strings)
        assertEquals(me, offQr.code)
        assertEquals(NetworkSwitchModel(strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK), isOn = false, hint = null), offQr.network)

        val on = machine.send(PaymentRequestEvent.IncludeNetworkChanged(true))
        val uri = "ethereum:$me@100"
        assertEquals(uri, on.qr_value)
        assertEquals("people paste addresses", me, on.copy_payload)
        val onQr = FlowLive.receiveQr(qrFixture(), me, "Me", on, strings = strings)
        assertEquals(uri, onQr.code)
        assertEquals(
            NetworkSwitchModel(
                strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK),
                isOn = true,
                hint = strings.t(I18nKeys.Flows.RECEIVE_INCLUDE_NETWORK_HINT),
            ),
            onQr.network,
        )

        val card = FlowLive.shareCard(cardFixture(), me, "Me", on.asset.network_name, strings, on.asset.chain_id, "XDAI", code = on.qr_value)
        assertEquals(uri, card.code)

        val scanned = Eip681.parse(on.qr_value)!!
        assertEquals(me, scanned.recipient)
        assertEquals(100L, scanned.chainId)
        assertNull(scanned.tokenAddress)
        assertNull(scanned.amountBaseUnits)
        assertTrue(scanned.isNative)
    }

    @Test
    fun `before the machine answers, the code is the address and there is no switch`() {
        val qr = FlowLive.receiveQr(qrFixture(), me, "Me", PaymentRequestView(), strings = strings)
        assertEquals(me, qr.code)
        assertNull(qr.network)
        val card = FlowLive.shareCard(cardFixture(), me, "Me", "Gnosis", strings, 100, "XDAI")
        assertEquals(me, card.code)
    }
}
