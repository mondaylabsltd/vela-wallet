package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.onboarding.core.assertionRequest
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.send.core.ceremonyFailure
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.passkeySignTimeoutMs
import uniffi.vela_core_uniffi.userOpNonceUsedDetail

/**
 * send-11 on the phone: the passkey prompt runs to the core's deadline, and a
 * prompt that signed nothing is "closed" or "timed out" by the core's rule
 * (`passkey_ceremony_end`) — Credential Manager says both as one cancel; only
 * the clock tells them apart. Also the dApp half of send-15: a nonce another
 * transaction spent first is a refusal with the core's fixed detail.
 */
class PasskeyDeadlineTest {
    private val deadline = passkeySignTimeoutMs().toLong()

    @Test
    fun `a cancel is the person's, a cancel at the deadline is the clock's, and a failure stays the passkey's`() {
        val cancelled = PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
        assertEquals(UserOpSpine.Failure.PasskeyCancelled, ceremonyFailure(cancelled, 3_000))
        assertEquals(UserOpSpine.Failure.PasskeyTimedOut, ceremonyFailure(cancelled, deadline))
        assertEquals(UserOpSpine.Failure.PasskeyTimedOut, ceremonyFailure(cancelled, deadline + 5_000))
        // A little before the figure still counts (the core's slack), well before does not.
        assertEquals(UserOpSpine.Failure.PasskeyTimedOut, ceremonyFailure(cancelled, deadline - 1_000))
        assertEquals(UserOpSpine.Failure.PasskeyCancelled, ceremonyFailure(cancelled, deadline - 30_000))
        // A failure that is not a cancel is never a timeout, whenever it came.
        val unsupported = PasskeyFailure(FailureKind.NotSupported, "no provider")
        assertEquals(UserOpSpine.Failure.Signer(FailureKind.NotSupported, "no provider"), ceremonyFailure(unsupported, deadline))
    }

    /**
     * Real-device pass 2 (D5): Google unreachable, the create sheet spun 241 s
     * and handed the person back to the key list with no reason. The wait ends
     * AT the core's deadline even when the platform call never answers its
     * cancellation, and its end is said as the deadline's — the core names it
     * "The passkey prompt timed out. Try again.", never the person's cancel.
     */
    @Test
    fun `a create that never answers ends at the deadline, said as timed out`() {
        val started = System.nanoTime()
        val ended = runCatching {
            runBlocking {
                app.getvela.wallet.feature.onboarding.core.withinPasskeyDeadline(300) {
                    // A provider stuck on a network it cannot reach, deaf to cancellation.
                    kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) {
                        kotlinx.coroutines.delay(60_000)
                    }
                }
            }
        }.exceptionOrNull()
        val tookMs = (System.nanoTime() - started) / 1_000_000
        assertTrue("ended at the deadline, not when the call gave up: $tookMs ms", tookMs < 5_000)
        val failure = ended as? app.getvela.wallet.feature.onboarding.core.PasskeyDeadlinePassed
            ?: return fail("not the deadline's end: $ended")
        assertEquals(FailureKind.Cancelled, failure.kind)
        val words = uniffi.vela_core_uniffi.passkeyErrorWords("android", failure.domain.orEmpty(), failure.code, failure.message.orEmpty())
        assertEquals("onboarding.common.passkeyError.timedOut", words.key)
        // An answer before the deadline is the answer.
        assertEquals(7, runBlocking { app.getvela.wallet.feature.onboarding.core.withinPasskeyDeadline(5_000) { 7 } })
    }

    @Test
    fun `the deadline is two minutes, the core's, and the prompt is asked for it`() {
        assertEquals(120_000L, deadline)
        val asked = assertionRequest(ByteArray(32), "getvela.app", "aabb", "internal", timeoutMs = deadline)
        assertEquals(deadline, asked.getLong("timeout"))
        // A sign-in keeps the platform's own.
        assertFalse(assertionRequest(ByteArray(32), "getvela.app", null, "").has("timeout"))
    }

    @Test
    fun `a prompt the platform closes at the deadline is a timeout, one closed early a cancel`() = runBlocking<Unit> {
        // The clock reads the prompt's time up: raised at 0, answered at [answeredAt].
        for ((answeredAt, expected) in listOf(
            deadline to UserOpSpine.Failure.PasskeyTimedOut,
            4_000L to UserOpSpine.Failure.PasskeyCancelled,
        )) {
            val ticks = ArrayDeque(listOf(0L, answeredAt))
            val spine = spine(clock = { ticks.removeFirstOrNull() ?: answeredAt }) {
                throw PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
            }
            assertEquals("answered at $answeredAt", expected, refusal(spine))
        }
    }

    @Test
    fun `a prompt nobody answers is stopped at the deadline and said as timed out`() = runBlocking<Unit> {
        var stopped = false
        // The deadline shortened for the test; the clock says it ran out.
        val ticks = ArrayDeque(listOf(0L, deadline))
        val spine = spine(deadlineMs = 50, clock = { ticks.removeFirstOrNull() ?: deadline }) {
            try {
                awaitCancellation()
            } finally {
                stopped = true
            }
        }
        assertEquals(UserOpSpine.Failure.PasskeyTimedOut, refusal(spine))
        assertTrue("the prompt's wait was cancelled, never left open", stopped)
    }

    @Test
    fun `the sheet hears a timeout as a timeout, and a spent nonce as a refusal in the core's words`() {
        assertEquals(SignSubmitOutcome.PasskeyTimedOut, SignExecutor.outcomeOf(UserOpSpine.Failure.PasskeyTimedOut))
        assertEquals(SignSubmitOutcome.PasskeyCancelled, SignExecutor.outcomeOf(UserOpSpine.Failure.PasskeyCancelled))
        val spent = SignExecutor.outcomeOf(UserOpSpine.Failure.NonceUsed) as SignSubmitOutcome.Failed
        assertEquals(userOpNonceUsedDetail(), spent.message)
        assertTrue("a refusal of this operation: no Retry", spent.refused)
        assertTrue(spent.message.contains("went first"))
    }

    // -- the rig ------------------------------------------------------------------

    private fun spine(deadlineMs: Long = deadline, clock: () -> Long, sign: suspend () -> Assertion): UserOpSpine {
        val accounts = AccountStore(FakeStore())
        runBlocking {
            accounts.saveAccount(
                JSONObject().put("id", CREDENTIAL).put("address", SAFE).put("public_key_hex", KEY).put("created_at_iso", "2026-01-01T00:00:00Z").put(
                    "keys",
                    JSONArray().put(JSONObject().put("credential_id", CREDENTIAL).put("public_key_hex", KEY).put("name", "Key 1").put("transports", "internal")),
                ),
            )
        }
        return UserOpSpine(
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(accounts),
            signer = {
                object : UserOpSigner {
                    override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion = sign()
                }
            },
            signDeadlineMs = deadlineMs,
            clock = clock,
        )
    }

    private suspend fun refusal(spine: UserOpSpine): UserOpSpine.Failure {
        try {
            spine.signMessage(100, SAFE, ByteArray(32))
        } catch (refused: UserOpSpine.Refused) {
            return refused.failure
        }
        fail("the ceremony signed nothing; nothing may be signed")
        error("unreachable")
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val CREDENTIAL = "aabbcc"
        val KEY = "04" + "11".repeat(64)
    }
}
