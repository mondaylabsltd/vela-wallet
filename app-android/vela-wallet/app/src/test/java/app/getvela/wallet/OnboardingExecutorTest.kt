package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.OnboardingExecutor
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.RegistryFailure
import app.getvela.wallet.feature.onboarding.core.SessionExecutor
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The failure contract, and the exhaustiveness the compiler cannot give us.
 *
 * The bridge is JSON, so `OnboardingExecutor`'s eighteen-way branch is a `when`
 * over strings rather than the desktop's `match` over an enum — the compiler
 * will not notice an operation nobody handled. **This file is that check.** An
 * operation with no failure variant leaves the core waiting forever on an effect
 * nobody answers, which presents as a spinner that never stops, with no error
 * anywhere.
 */
class OnboardingExecutorTest {

    private fun operation(type: String) = JSONObject().put("type", type)

    @Test
    fun everyOperationOwnsAFailureVariant() {
        assertEquals(18, OnboardingExecutor.OPERATIONS.size)
        OnboardingExecutor.OPERATIONS.forEach { type ->
            val answer = OnboardingExecutor.failureFor(operation(type), RuntimeException("boom"))
            assertNotNull("no failure variant for `$type`", answer.optString("type"))
            assertTrue("empty failure variant for `$type`", answer.optString("type").isNotEmpty())
        }
    }

    @Test
    fun anUnknownOperationIsLoudRatherThanSilent() {
        val thrown = runCatching {
            OnboardingExecutor.failureFor(operation("teleport_wallet"), RuntimeException())
        }.exceptionOrNull()
        assertNotNull("an unknown operation must throw, not answer", thrown)
    }

    /**
     * The session machine's operations, all of them — a count, because the
     * failure map below answers by NAME and a new operation that nobody taught
     * it would fall through to `accounts_unavailable` and sign the device out.
     *
     * Eight since 2026-09-23: `remove_account`, one wallet leaving a device
     * that keeps the others.
     */
    @Test
    fun sessionOperationsAreAllEight() {
        assertEquals(8, SessionExecutor.OPERATIONS.size)
    }

    /**
     * `network` is the one bit of classification only a shell can supply: a
     * request that never arrived is not the same as one the server refused. The
     * core branches on it — an unreachable index offers a different endpoint, a
     * 4xx does not.
     */
    @Test
    fun onlyATransportFailureIsNetwork() {
        val refused = OnboardingExecutor.failureFor(
            operation("registry_query_by_public_key"),
            RegistryFailure("Query failed: 404", network = false),
        )
        assertEquals("index_failed", refused.getString("type"))
        assertFalse(refused.getBoolean("network"))

        val unreachable = OnboardingExecutor.failureFor(
            operation("registry_publish"),
            RegistryFailure("Register failed: connection refused", network = true),
        )
        assertTrue(unreachable.getBoolean("network"))

        // An exception that is not a registry failure at all cannot claim the
        // server answered — it never got that far.
        val unknown = OnboardingExecutor.failureFor(
            operation("registry_query_unit"),
            IllegalStateException("bad state"),
        )
        assertTrue(unknown.getBoolean("network"))
    }

    /**
     * `sign_member_proof` is the mixed one: the ceremony and the challenge fetch
     * can each fail, and the core branches differently on the two. Classifying
     * by the OPERATION rather than by what threw would send a network outage to
     * the passkey sheet.
     */
    @Test
    fun memberProofClassifiesByWhatThrew() {
        val fromRegistry = OnboardingExecutor.failureFor(
            operation("sign_member_proof"),
            RegistryFailure("Challenge failed: 503", network = false),
        )
        assertEquals("index_failed", fromRegistry.getString("type"))

        val fromCeremony = OnboardingExecutor.failureFor(
            operation("sign_member_proof"),
            PasskeyFailure(
                app.getvela.wallet.feature.onboarding.core.FailureKind.Cancelled,
                "cancelled",
            ),
        )
        assertEquals("passkey_failed", fromCeremony.getString("type"))
        assertEquals("cancelled", fromCeremony.getString("kind"))
    }

    /**
     * A cancellation carries no message; everything else carries the platform's
     * own words. Prettifying them would lose the only detail a bug report has.
     */
    @Test
    fun onlyClassifiedFailuresDropTheirMessage() {
        val cancelled = OnboardingExecutor.failureFor(
            operation("register_passkey"),
            PasskeyFailure(
                app.getvela.wallet.feature.onboarding.core.FailureKind.Cancelled,
                "User cancelled the operation",
            ),
        )
        assertTrue(cancelled.isNull("message"))

        val other = OnboardingExecutor.failureFor(
            operation("register_passkey"),
            PasskeyFailure(
                app.getvela.wallet.feature.onboarding.core.FailureKind.Other,
                "provider exploded",
            ),
        )
        assertEquals("provider exploded", other.getString("message"))
    }

    /** Three operations degrade rather than fail; the core must never see an error. */
    @Test
    fun bestEffortOperationsDegradeQuietly() {
        val name = OnboardingExecutor.failureFor(operation("lookup_legacy_name"), RuntimeException())
        assertEquals("legacy_name", name.getString("type"))
        assertTrue(name.isNull("name"))

        val health = OnboardingExecutor.failureFor(operation("probe_index_health"), RuntimeException())
        assertFalse(health.getBoolean("ok"))

        // A dismissed dialog is a refusal, not an error.
        val prompt = OnboardingExecutor.failureFor(operation("prompt"), RuntimeException())
        assertFalse(prompt.getBoolean("accepted"))
    }

    /**
     * `check_passkey_support` never fails outward — the contract says report
     * `supported: false`. A thrown support check would abort a create before the
     * form is even on screen.
     */
    @Test
    fun supportCheckFailsAsUnsupported() {
        val answer = OnboardingExecutor.failureFor(
            operation("check_passkey_support"),
            RuntimeException(),
        )
        assertEquals("passkey_support", answer.getString("type"))
        assertFalse(answer.getBoolean("supported"))
    }

    /** The last-resort net still produces parseable JSON for every operation. */
    @Test
    fun escapedFailuresAreStillAnswers() {
        OnboardingExecutor.OPERATIONS.forEach { type ->
            val json = JSONObject(OnboardingExecutor.escapedFailure(operation(type), Error("fatal")))
            assertTrue(json.optString("type").isNotEmpty())
        }
        // Even for an operation the mapping does not know — the core has to be
        // unblocked whatever happened.
        val unknown = JSONObject(
            OnboardingExecutor.escapedFailure(operation("teleport_wallet"), Error("fatal")),
        )
        assertEquals("storage_failed", unknown.getString("type"))
    }

    /** UTC, always: a local offset in `created_at_iso` means something else abroad. */
    @Test
    fun timestampsAreUtcIso() {
        assertTrue(OnboardingExecutor.nowIso().endsWith("Z"))
        assertTrue(OnboardingExecutor.nowIso().matches(Regex("""\d{4}-\d{2}-\d{2}T[\d:.]+Z""")))
    }

    /** A pasted URL with a stray newline or slash must not become a broken host. */
    @Test
    fun registryUrlIsNormalized() {
        assertEquals("https://example.test", RegistryClient.normalize("  https://example.test/ \n"))
        assertEquals(RegistryClient.DEFAULT_REGISTRY_URL, RegistryClient.normalize("   "))
    }
}
