package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RefusalReason
import app.getvela.wallet.feature.send.core.SendEstimateFailure
import app.getvela.wallet.feature.send.core.SendSubmitFailure
import app.getvela.wallet.feature.send.core.SendTxErrorKey
import app.getvela.wallet.feature.send.core.UserOpSpine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.RelayRejection
import uniffi.vela_core_uniffi.feeFailureReasonKey
import uniffi.vela_core_uniffi.feeRequoteDelayMs
import uniffi.vela_core_uniffi.classifyRelayRejection
import uniffi.vela_core_uniffi.sendEstimateFailureBodyKey

/**
 * Round 21 (§35) on Android's wire: the words the core added cross as the core
 * writes them, and each lands where its rule says.
 *
 * - review 1-6: `settlement_gas_too_high` — a fee failure (and its send
 *   mirror) worded `componentsUi.gas.reasonSettlementHigh`/`…Stopped` and
 *   asked again on the relay schedule, like `gas_quote_too_high`;
 * - every SafeOp expires: `expired` — Send's submit failure and error key, the
 *   tracker's refusal, and the spine's failure the multisig names honestly.
 */
class R21DoorsWireTest {

    @Test
    fun `a relay's settlementGas past the cap decodes as its own fee failure`() {
        assertEquals(FeeFailure.SettlementGasTooHigh, Wire.json.decodeFromString(FeeFailure.WireSerializer, "\"settlement_gas_too_high\""))
        assertEquals("settlement_gas_too_high", FeeFailure.SettlementGasTooHigh.wire)
        assertTrue(FeeFailure.SettlementGasTooHigh in FeeFailure.PLAIN)
        val view = Wire.json.decodeFromString(FeeView.serializer(), """{"failed":"settlement_gas_too_high"}""")
        assertEquals(FeeFailure.SettlementGasTooHigh, view.failed)
        // The send side's mirror, both ways.
        val send = Wire.json.decodeFromString(SendEstimateFailure.serializer(), "\"settlement_gas_too_high\"")
        assertEquals(SendEstimateFailure.Fee(FeeFailure.SettlementGasTooHigh), send)
        assertEquals("\"settlement_gas_too_high\"", Wire.json.encodeToString(SendEstimateFailure.serializer(), send))
        // The core's words and schedule for it.
        assertEquals(I18nKeys.Flows.FEE_REASON_SETTLEMENT_HIGH, feeFailureReasonKey(FeeFailure.SettlementGasTooHigh.wire))
        assertEquals(I18nKeys.Flows.FEE_REASON_SETTLEMENT_HIGH_STOPPED, sendEstimateFailureBodyKey(send.wire))
        assertNotNull("asked again on the relay schedule", feeRequoteDelayMs(FeeFailure.SettlementGasTooHigh.wire, 1u))
        assertEquals(feeRequoteDelayMs(FeeFailure.GasQuoteTooHigh.wire, 1u), feeRequoteDelayMs(FeeFailure.SettlementGasTooHigh.wire, 1u))
    }

    @Test
    fun `an operation that expired crosses as the core writes it`() {
        assertEquals("""{"type":"expired"}""", Wire.json.encodeToString(SendSubmitFailure.serializer(), SendSubmitFailure.Expired))
        assertEquals(SendSubmitFailure.Expired, Wire.json.decodeFromString(SendSubmitFailure.serializer(), """{"type":"expired"}"""))
        assertEquals(SendTxErrorKey.Expired, Wire.json.decodeFromString(SendTxErrorKey.serializer(), "\"expired\""))
        assertEquals(RefusalReason.Expired, Wire.json.decodeFromString(RefusalReason.serializer(), "\"expired\""))
    }

    @Test
    fun `the EntryPoint's AA22 is the core's Expired, and a multisig execution says it honestly`() {
        val rejection = classifyRelayRejection("AA22 expired or not due")
        assertEquals(RelayRejection.Expired, rejection)
        val failure = MultisigShellFailure.of(UserOpSpine.Failure.Expired)
        assertTrue(failure.toString(), failure is MultisigShellFailure.Unknown)
        assertTrue(failure.toString(), failure.toString().contains("expired before it was submitted"))
    }
}
