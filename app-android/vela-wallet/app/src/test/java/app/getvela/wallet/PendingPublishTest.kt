package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.PendingPublish
import app.getvela.wallet.feature.onboarding.core.PendingPublisher
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.onboarding.core.SessionExecutor
import app.getvela.wallet.feature.onboarding.core.SessionOperation
import app.getvela.wallet.feature.onboarding.core.SessionShellResult
import java.util.Collections
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ob-2 on Android: a wallet saved BEFORE its registry publish carries the whole
 * publish in its outbox record, and the session — app-resident — replays it
 * (`publish_pending_upload`) with no passkey, removing the record only once the
 * group has landed. Before native parity A1 the executor threw on the
 * operation, and such a wallet never finished registering.
 */
class PendingPublishTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    private val account = """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        "public_key_hex":"04ab","created_at_iso":"2026-10-04T10:00:00.000Z",
        "keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}"""

    private val proof = JSONObject()
        .put("authenticatorData", "00").put("clientDataJSON", "00")
        .put("challengeIndex", 0).put("typeIndex", 0).put("r", "01").put("s", "02")

    private fun member(withProof: Boolean) = JSONObject()
        .put("credential_id", "cred-1")
        .put("public_key_hex", "04ab")
        .put("attestation_hex", "a0")
        .put("authenticator_attachment", "platform")
        .put("transports", "internal")
        .put("proof", if (withProof) proof else JSONObject.NULL)

    private fun publish(withProof: Boolean = true) = JSONObject()
        .put("metadata_hex", "0x01")
        .put("members", JSONArray().put(member(withProof)))
        .put("group_seed_hex", "aa".repeat(32))
        .put("group_public_key_hex", "04bb")

    /** An outbox record with no task id and a replayable publish (ob-2). */
    private fun record() = JSONObject()
        .put("id", "cred-1")
        .put("name", "Ann")
        .put("public_key_hex", "04ab")
        .put("attestation_object_hex", "a0")
        .put("created_at_iso", "2026-10-04T10:00:00.000Z")
        .put("authenticator_attachment", "platform")
        .put("transports", "internal")
        .put("members", JSONArray())
        .put("publish", publish())

    private fun seeded() = FakeStore(
        mapOf(
            "vela.accounts" to "[$account]",
            "vela.activeAccountIndex" to "0",
            "vela.pendingUploads" to JSONArray().put(record()).toString(),
        ),
    )

    private fun pendingPublish(withProof: Boolean = true): PendingPublish =
        Wire.json.decodeFromString(PendingPublish.serializer(), publish(withProof).toString())

    @Test
    fun `the session replays a pending publish and the record goes once it landed`() = runBlocking<Unit> {
        val store = seeded()
        val replayed = Collections.synchronizedList(mutableListOf<PendingPublish>())
        val executor = SessionExecutor(AccountStore(store), publisher = { replayed += it })
        val session = SessionController(AccountStore(store), scope, executor)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading && it.hasWallet } }
        withTimeout(10_000) { while (store.values["vela.pendingUploads"].orEmpty().contains("cred-1")) delay(50) }
        assertEquals(1, replayed.size)
        assertEquals("0x01", replayed.single().metadata_hex)
        assertEquals(1, replayed.single().members.size)
        withTimeout(10_000) { session.view.first { it.unsynced.isEmpty() } }
    }

    @Test
    fun `a replay that did not land keeps the record, names the wallet, and keeps retrying`() = runBlocking<Unit> {
        val store = seeded()
        val executor = SessionExecutor(AccountStore(store), publisher = { error("registry unreachable") })
        val session = SessionController(AccountStore(store), scope, executor)
        session.boot()
        val view = withTimeout(10_000) { session.view.first { !it.loading && it.unsynced.isNotEmpty() } }
        assertEquals("Ann", view.unsynced.single().name)
        assertTrue("the core can finish it on its own", view.unsynced.single().retrying)
        delay(300)
        assertTrue(store.values["vela.pendingUploads"].orEmpty().contains("cred-1"))
        // Asking again is harmless: the core runs one round at a time.
        session.syncPending()
    }

    @Test
    fun `the executor answers registry_landing_unconfirmed with the reason`() = runBlocking<Unit> {
        val executor = SessionExecutor(AccountStore(FakeStore()), publisher = { throw IllegalStateException("429") })
        assertEquals(
            SessionShellResult.RegistryLandingUnconfirmed("429"),
            executor.perform(SessionOperation.PublishPendingUpload(pendingPublish())),
        )
        val landed = SessionExecutor(AccountStore(FakeStore()), publisher = { })
        assertEquals(SessionShellResult.RegistryLanded, landed.perform(SessionOperation.PublishPendingUpload(pendingPublish())))
    }

    @Test
    fun `a member with no creation-time proof is never prompted for — the replay refuses`() = runBlocking<Unit> {
        // A record from before ob-2: only a sign-in can publish it.
        val publisher = PendingPublisher(registry = { error("the registry must not be asked") })
        val thrown = runCatching { publisher.publish(pendingPublish(withProof = false)) }.exceptionOrNull()
        assertTrue(thrown?.message.orEmpty(), thrown?.message.orEmpty().contains("no creation-time proof"))
    }

    @Test
    fun `an escaped failure keeps the record, whatever the operation`() {
        val answer = JSONObject(
            SessionExecutor.escapedFailure(
                JSONObject(Wire.json.encodeToString(SessionOperation.serializer(), SessionOperation.PublishPendingUpload(pendingPublish()))),
                RuntimeException("boom"),
            ),
        )
        assertEquals("registry_landing_unconfirmed", answer.getString("type"))
        assertEquals("boom", answer.getString("message"))
    }
}
