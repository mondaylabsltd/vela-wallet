package app.getvela.wallet.feature.onboarding.core

import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONObject
import uniffi.vela_core_uniffi.registryBuildGroupProof
import uniffi.vela_core_uniffi.trustedSignerRegistryRpId

/**
 * ob-2: a wallet's registry publish that never landed, replayed from its
 * outbox record — the session's `publish_pending_upload` (the web's
 * `publishPending`, the desktop's `registry::publish` from `perform_session`).
 *
 * Every member carries the possession proof it made at creation and the
 * group key rides in the record, so **no passkey is asked**: the group key
 * closes the group over the content hash, the registry is asked to register
 * it, and the answer waits for the landing on the registry's own budget —
 * only a landed group lets the record go. A member with no proof is a record
 * from before ob-2, which only a sign-in can publish: that is a failure here
 * (the record stays), never a prompt.
 *
 * Throws on anything short of landed; [SessionExecutor] tells the core
 * `registry_landing_unconfirmed` with the reason.
 */
class PendingPublisher(
    private val registry: suspend () -> RegistryClient,
    /**
     * The wallet's own relying party, for a unit filed under no page — the
     * one the key index records, so the one the unit's keys were made under:
     * this build's platform relying party, the core's rule (§35,
     * [RelyingParty.current]) — `getvela.app` for a release build,
     * `localhost` for a test build on an emulator, the build's
     * `TEST_RP_DOMAIN` on a device. Read only when a unit needs it; a build
     * with none refuses the replay in the core's words (the record stays).
     */
    private val relyingPartyId: () -> String = {
        when (val answer = RelyingParty.current) {
            is RelyingParty.Answer.Id -> answer.rpId
            is RelyingParty.Answer.Unavailable -> throw RegistryFailure(answer.message, network = false)
        }
    },
    /** The group proof over the challenge — the core's (`registry_build_group_proof`); a seam for tests. */
    private val groupProof: (seedHex: String, rpId: String, challenge: String) -> String = ::registryBuildGroupProof,
    /** Spec 102's rule from a page to the unit's rpId — the core's; a seam for tests. */
    private val rpIdOf: (page: String?) -> String? = ::trustedSignerRegistryRpId,
) {

    suspend fun publish(publish: PendingPublish) {
        val members = publish.members.map { PublishMember.from(JSONObject(it.toString())) }
        if (members.isEmpty()) throw RegistryFailure("the pending publish has no members", network = false)
        val unproven = members.count { it.proof == null }
        if (unproven > 0) {
            // Before ob-2: only a sign-in can publish it. Kept, not prompted.
            throw RegistryFailure("$unproven member(s) carry no creation-time proof", network = false)
        }
        if (publish.group_seed_hex.isBlank() || publish.group_public_key_hex.isBlank()) {
            throw RegistryFailure("the pending publish carries no group key", network = false)
        }
        val rpId = rpIdOf(publish.page) ?: relyingPartyId()
        val client = registry()
        val challenge = client.groupChallenge(
            metadataHex = publish.metadata_hex,
            groupPublicKey = publish.group_public_key_hex,
            members = members,
            rpId = rpId,
        )
        val group = JSONObject(groupProof(publish.group_seed_hex, rpId, challenge.groupChallenge))
        val ack = client.registerGroup(
            metadataHex = publish.metadata_hex,
            groupPublicKey = publish.group_public_key_hex,
            groupProof = group.getJSONObject("proof"),
            members = members.map { ProvenMember(it, it.proof!!) },
            rpId = rpId,
        )
        when (val next = OnboardingExecutor.afterRegister(ack, answerWhenAccepted = false)) {
            OnboardingExecutor.AfterRegister.Landed -> Unit
            is OnboardingExecutor.AfterRegister.AwaitLanding -> client.awaitTask(next.taskId)
            // Never asked for (`answerWhenAccepted = false`); a record goes only once landed.
            is OnboardingExecutor.AfterRegister.Accepted -> client.awaitTask(next.taskId)
        }
        VelaLog.event("registry.landing", "replayed publish landed", "members" to members.size)
    }
}
