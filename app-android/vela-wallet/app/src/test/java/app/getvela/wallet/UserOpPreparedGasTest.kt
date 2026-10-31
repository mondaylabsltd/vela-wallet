package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.PreparedGas
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.userOpApplyEstimate
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpSafeOpHash
import uniffi.vela_core_uniffi.userOpWithCalls

/**
 * core-perf-1 on Android: the submit signs the gas its fee was priced on
 * (`SubmitUserOp.prepared`) instead of asking the relay to simulate again and
 * measuring the inner calls again — when, and only when, the quote measured
 * the account as it stands now. The nonce is read fresh either way.
 *
 * What the passkey is asked to sign (the SafeOp hash) is checked against the
 * operation the core assembles with exactly those limits; the ceremony then
 * ends unsigned, so nothing here needs a real signature.
 */
class UserOpPreparedGasTest {
    private val port = FakeRelayPort()
    private val measured = AtomicInteger()
    private var challenge: ByteArray? = null

    private val call = UserOpCall(to = TOKEN, value = "0", data = TRANSFER)
    private val prepared = PreparedGas(
        calls = listOf(FeeCall(TOKEN, "0", TRANSFER)),
        deployed = true,
        verification_gas_limit = "123456",
        call_gas_limit = "345678",
        pre_verification_gas = "56789",
        settlement_gas = null,
    )

    init {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_blockNumber") { FakeRelayPort.body("0x10") }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
    }

    @Test
    fun `the quote's measured gas is signed, without a second simulation or measurement`() = runBlocking {
        submit(prepared)
        assertFalse("no second simulation", port.calls.any { it.endsWith("eth_estimateUserOperationGas") })
        assertEquals("no second measurement of the inner calls", 0, measured.get())
        assertTrue("the nonce is read fresh", port.calls.any { it.endsWith("eth_call") })
        assertArrayEquals("the passkey signs the op with exactly the quote's limits", expectedChallenge(prepared), challenge)
    }

    @Test
    fun `gas measured for another deployment state is not used`() = runBlocking {
        submit(prepared.copy(deployed = false))
        assertTrue("measured again", port.calls.any { it.endsWith("eth_estimateUserOperationGas") })
        assertEquals("the inner call measured again", 1, measured.get())
        assertFalse(expectedChallenge(prepared).contentEquals(challenge))
    }

    @Test
    fun `without a measurement the submit measures as it always did`() = runBlocking {
        submit(null)
        assertTrue(port.calls.any { it.endsWith("eth_estimateUserOperationGas") })
        assertEquals(1, measured.get())
        assertNotNull(challenge)
    }

    @Test
    fun `the estimate and the submit carry the measurement on the wire`() {
        val gas = """{"calls":[{"to":"$TOKEN","value":"0","data":"$TRANSFER"}],"deployed":true,
            "verification_gas_limit":"123456","call_gas_limit":"345678","pre_verification_gas":"56789","settlement_gas":null}"""
        val estimate = Wire.json.decodeFromString(
            FeeEstimateView.serializer(),
            """{"chain_id":100,"total_wei":"1","max_fee_per_gas":"1","network_fee_per_gas":"1","relayer_fee_per_gas":"0",
               "bundler_gas_price":"1","in_band_gas_basis":"1","total_gas":"1","deployed":true,"tier":"standard","quoted":true,
               "fee_asset":{"type":"native"},"prepared":$gas}""",
        )
        assertEquals(prepared, estimate.prepared)
        // Handed back to the core whole (`SendEvent.FeeUpdated`).
        assertEquals(prepared, Wire.json.decodeFromString(FeeEstimateView.serializer(), Wire.json.encodeToString(FeeEstimateView.serializer(), estimate)).prepared)
        val submit = Wire.json.decodeFromString(
            SendOperation.serializer(),
            """{"type":"submit_user_op","chain_id":100,"account":"$SAFE","public_key_hex":"$KEY","calls":[],"prepared":$gas}""",
        ) as SendOperation.SubmitUserOp
        assertEquals(prepared, submit.prepared)
        val bare = Wire.json.decodeFromString(
            SendOperation.serializer(),
            """{"type":"submit_user_op","chain_id":100,"account":"$SAFE","public_key_hex":"$KEY","calls":[]}""",
        ) as SendOperation.SubmitUserOp
        assertNull(bare.prepared)
    }

    // -- the rig ------------------------------------------------------------------

    private fun expectedChallenge(gas: PreparedGas): ByteArray {
        val floors = userOpFloors(100u, true, 2u)
        var draft = userOpDraft(SAFE, "0x7", true, listOf(KEY), listOf(call), UserOpFeeMode.InBand(null, "1", SAFE), floors)
        draft = userOpApplyEstimate(draft, gas.verification_gas_limit, gas.call_gas_limit, gas.pre_verification_gas, gas.settlement_gas, floors)
        draft = userOpWithCalls(draft, listOf(call), UserOpFeeMode.InBand(null, FEE, RECIPIENT))
        return userOpSafeOpHash(draft, 100u)
    }

    private suspend fun submit(gas: PreparedGas?) {
        val accounts = AccountStore(FakeStore())
        accounts.saveAccount(
            JSONObject().put("id", CREDENTIAL).put("address", SAFE).put("public_key_hex", KEY).put(
                "keys",
                JSONArray().put(JSONObject().put("credential_id", CREDENTIAL).put("public_key_hex", KEY).put("name", "Key 1").put("transports", "internal")),
            ),
        )
        val spine = UserOpSpine(
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(accounts),
            signer = {
                object : UserOpSigner {
                    override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                        this@UserOpPreparedGasTest.challenge = challenge
                        throw PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
                    }
                }
            },
            measureCall = { _, _, _, _, _ -> measured.incrementAndGet(); "0x30d40" },
        )
        try {
            spine.submit(100, SAFE, listOf(call), null, UserOpSpine.Quoted(FEE, RECIPIENT), prepared = gas)
            fail("the ceremony signed nothing; nothing may be sent")
        } catch (refused: UserOpSpine.Refused) {
            assertEquals(UserOpSpine.Failure.PasskeyCancelled, refused.failure)
        }
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val TOKEN = "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
        const val FEE = "1000000000000000"
        const val CREDENTIAL = "aabbcc"
        val KEY = "04" + "11".repeat(64)
        // A contract call, not a plain transfer: its inner gas is measured.
        val TRANSFER = "0xdeadbeef" + "0".repeat(63) + "1"
    }
}
