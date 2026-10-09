package app.getvela.wallet.feature.onboarding.core

import android.os.SystemClock
import app.getvela.wallet.core.diagnostics.VelaLog
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.trustedSignerCeremonyRequest
import uniffi.vela_core_uniffi.registryBuildGroupProof
import uniffi.vela_core_uniffi.registryBuildMemberProof
import uniffi.vela_core_uniffi.registryGroupPublicKeyFromSeed
import uniffi.vela_core_uniffi.toHex

/**
 * The only place onboarding touches the outside world.
 *
 * Each of the eighteen `ShellOperation`s the core declares maps to exactly one
 * call — a passkey ceremony, a storage read or write, a registry request, a
 * timer, a prompt. There is no branching on business meaning here: **if this
 * file ever grows an `if` that decides what happens next, that decision belongs
 * in the Rust machine instead.**
 *
 * ## Failure contract
 *
 * Nothing propagates outward. Every operation answers with the result variant it
 * owes, including for its failures, which is what lets the core own
 * classification instead of the shell pattern-matching error strings.
 *
 * The mapping lives in [failureFor] rather than inline in each arm, for the same
 * reason the web executor splits it out: the classification for several
 * operations depends on WHAT THREW, not on which operation it was —
 * `sign_member_proof` can fail as a ceremony or as a registry call, and the core
 * branches differently on the two.
 *
 * ## Exhaustiveness
 *
 * The bridge is JSON, so the compiler cannot check this `when` the way it checks
 * the desktop's `match`. `OnboardingExecutorTest` enumerates all eighteen
 * operation names against [OPERATIONS] instead, and an operation this file does
 * not handle throws rather than silently answering nothing — a silent answer
 * would leave the core waiting forever with a spinner on screen.
 */
class OnboardingExecutor(
    private val passkey: PasskeyExecutor,
    private val registry: RegistryClient,
    private val store: AccountStore,
    private val deps: Deps,
    /**
     * Spec 102 R3: the signing page, when this surface can open one. A
     * ceremony whose operation names a `page` runs THERE instead of on the
     * platform's sheet — only that page can mint or use a custom domain's
     * keys. `null` on surfaces with no way to open a page (previews, the
     * gallery).
     */
    private val trustedSigner: TrustedSignerCeremonies? = null,
    /**
     * The corpus, for the few sentences a ceremony that cannot reach its page
     * is told in (`componentsUi.signing.*`). English-free by default: the
     * key itself, which a test can read.
     */
    private val words: (key: String) -> String = { it },
) {
    /** The two operations whose outside world is the user interface itself. */
    interface Deps {
        /**
         * Show a notice or ask a question. `confirmable` selects a two-button
         * dialog whose answer is a business decision the core acts on; a
         * dismissal is `false`.
         */
        suspend fun prompt(kind: PromptKind, confirmable: Boolean): Boolean

        /** Hand the wallet to the session machine and leave onboarding. */
        suspend fun complete(mode: JSONObject)

        /**
         * The wallet's name, as the signing page shows it ("create a key for
         * 〈name〉"). Empty when the flow has none yet.
         */
        fun walletName(): String = ""
    }

    /** Perform one operation and return the result JSON the core is waiting for. */
    suspend fun perform(operation: JSONObject): String {
        // One line in, one line out, for every operation the core asks for. A
        // create that "does not work" is always a specific step that did not
        // answer, and this is the record of which (spec 019, USB-key
        // diagnosis 2026-08-26). Debug builds only; see VelaLog.
        val type = operation.optString("type")
        val started = SystemClock.elapsedRealtime()
        VelaLog.event(
            "core.operation",
            type,
            "cred" to VelaLog.shortId(operation.optString("credential_id").ifEmpty { null }),
            "transports" to operation.optString("transports").ifEmpty { null },
            "method" to operation.optString("method").ifEmpty { null },
            "purpose" to operation.optString("purpose").ifEmpty { null },
        )
        return try {
            val answer = run(operation)
            VelaLog.event(
                "core.result",
                answer.optString("type"),
                "for" to type,
                "ms" to SystemClock.elapsedRealtime() - started,
            )
            answer.toString()
        } catch (error: Throwable) {
            if (error is kotlinx.coroutines.CancellationException) throw error
            val answer = failureFor(operation, error)
            VelaLog.failure(
                "core.failed",
                type,
                error,
                "answered" to answer.optString("type"),
                "kind" to answer.opt("kind"),
                "ms" to SystemClock.elapsedRealtime() - started,
            )
            answer.toString()
        }
    }

    private suspend fun run(operation: JSONObject): JSONObject =
        when (val type = operation.getString("type")) {
            "check_passkey_support" ->
                result("passkey_support") { put("supported", passkey.supported()) }

            "register_passkey" -> {
                val method = KeyMethod.of(operation.optString("method", KeyMethod.Platform.wire))
                // Spec 102 R3: a custom-domain wallet's key is minted on its
                // page (the op names it), and the core hands back the machine's
                // own `Registration` — which carries `signer_origin`, so the
                // key remembers where it lives. `method` rides along as the
                // place the page hints the browser to.
                val wire = if (pageOf(operation) != null) {
                    registered(onPage(operation))
                } else {
                    val registration = passkey.register(
                        name = operation.optString("name"),
                        excludeCredentialIds = operation.optJSONArray("exclude_credential_ids").strings(),
                        method = method,
                    )
                    JSONObject()
                        .put("credential_id", registration.credentialIdHex)
                        .put("attestation_object_hex", registration.attestationObjectHex)
                        .put("client_data_json_hex", registration.clientDataJsonHex)
                        .put("authenticator_attachment", registration.authenticatorAttachment)
                        .put("transports", registration.transports)
                }
                result("passkey_registered") {
                    put("registration", wire)
                    put("now_iso", nowIso())
                }
            }

            "sign_proof" -> {
                val wire = operation.optString("method")
                // The place the person signed in with. Recovery's second
                // signature over caBLE goes back to the same phone; one made
                // on a custom domain's page goes back to that page (R3).
                val method = if (wire.isEmpty()) KeyMethod.Platform else KeyMethod.of(wire)
                val proof = if (pageOf(operation) != null) {
                    asserted(onPage(operation))
                } else {
                    passkey.assert(
                        challenge = challengeFor(operation.optString("purpose")),
                        credentialIdHex = operation.optString("credential_id"),
                        transports = operation.optString("transports"),
                        method = method,
                    ).toWire()
                }
                result("proof_signed") {
                    put("assertion", proof)
                    put("now_iso", nowIso())
                }
            }

            "generate_group_key" -> {
                // The one-time software group key — the only randomness in the
                // flow that is not a challenge, and it stays in the shell. The
                // core only echoes it into the final publish.
                val seedHex = toHex(passkey.random(GROUP_SEED_BYTES), false)
                result("group_key_generated") {
                    put("seed_hex", seedHex)
                    put("group_public_key_hex", registryGroupPublicKeyFromSeed(seedHex))
                }
            }

            "sign_member_proof" -> {
                // Creation-time membership confirmation: fetch the member-mode
                // challenge (it binds only groupPublicKey + own attestation, so
                // it exists before the rest of the set does), sign against
                // exactly this credential, assemble the proof in the core. The
                // publish later replays it without another prompt.
                // The rpId is the KEY's, not this app's. A key minted on a
                // custom domain's page belongs to that domain — a browser lets
                // a page mint passkeys for nothing else — so asking the
                // registry under `getvela.app` for a key that lives on
                // `localhost` returns a challenge the page could not have
                // derived, and the wallet then refuses its own key's proof.
                // Spec 102: the op's `page` is that domain's page; the rule
                // that folds it to an rpId is the core's.
                val keyRpId = uniffi.vela_core_uniffi.trustedSignerRegistryRpId(pageOf(operation))
                    ?: passkey.relyingPartyId
                val challenge = registry.memberChallenge(
                    groupPublicKey = operation.optString("group_public_key_hex"),
                    publicKey = operation.optString("public_key_hex"),
                    attestation = operation.optString("attestation_hex"),
                    rpId = keyRpId,
                )
                val memberWire = operation.optString("method")
                // The place that minted this key confirms it — a key created
                // on the phone over caBLE is confirmed on that phone, one
                // created on a custom domain's page on that page.
                val memberMethod =
                    if (memberWire.isEmpty()) KeyMethod.Platform else KeyMethod.of(memberWire)
                val bytes = uniffi.vela_core_uniffi.fromHex(stripHex(challenge))
                val assertion = if (pageOf(operation) != null) {
                    // The page fetches its own challenge from the registry for
                    // the inputs it displays; the core refuses the answer
                    // unless it equals the one the WALLET fetched here.
                    assertionOf(asserted(onPage(operation, expectedMemberChallenge = bytes)))
                } else {
                    passkey.assert(
                        challenge = bytes,
                        credentialIdHex = operation.optString("credential_id"),
                        transports = operation.optString("transports"),
                        method = memberMethod,
                    )
                }
                result("member_proof_signed") {
                    put("proof", JSONObject(memberProof(assertion)))
                }
            }

            "lookup_legacy_name" -> result("legacy_name") {
                put("name", registry.legacyName(operation.optString("credential_id")) ?: JSONObject.NULL)
            }

            "authenticate_passkey" -> {
                val wire = operation.optString("method")
                val method = if (wire.isEmpty()) KeyMethod.Platform else KeyMethod.of(wire)
                // Spec 102 R3: signing in on a custom domain's page — the page
                // asks the browser for a key on the place `method` names and
                // signs a challenge IT derived (`vela-signin-…`), so a sign-in
                // can never be a transaction hash in disguise.
                val proof = if (pageOf(operation) != null) {
                    asserted(onPage(operation))
                } else {
                    passkey.assert(passkey.random(CHALLENGE_BYTES), null, method = method).toWire()
                }
                result("passkey_authenticated") {
                    put("assertion", proof)
                    put("now_iso", nowIso())
                }
            }

            "load_accounts" -> result("accounts_loaded") { put("accounts", store.loadAccounts()) }

            "save_account" -> {
                store.saveAccount(operation.getJSONObject("account"))
                result("account_saved") {}
            }

            "save_pending_upload" -> {
                store.savePendingUpload(operation.getJSONObject("record"))
                result("pending_upload_saved") {}
            }

            "remove_pending_upload" -> {
                store.removePendingUpload(operation.optString("credential_id"))
                result("pending_upload_removed") {}
            }

            "registry_publish" -> publishedResult(publish(operation))

            "registry_query_by_public_key" -> {
                val status = registry.queryByPublicKey(operation.optString("public_key_hex"))
                result("registry_key_status") {
                    put("registered", status.registered)
                    put("unit_ids", JSONArray().apply { status.unitIds.forEach { put(it) } })
                }
            }

            "registry_query_unit" -> {
                val unit = registry.queryUnit(operation.optLong("unit_id"))
                result("registry_unit") {
                    put("metadata_hex", unit.metadataHex)
                    put(
                        "members",
                        JSONArray().apply {
                            unit.members.forEach { member ->
                                put(
                                    JSONObject()
                                        .put("credential_id", member.credentialIdHex)
                                        .put("public_key_hex", member.publicKeyHex)
                                        .put(
                                            "authenticator_attachment",
                                            member.authenticatorAttachment,
                                        )
                                        .put("transports", member.transports),
                                )
                            }
                        },
                    )
                }
            }

            "probe_index_health" -> result("index_health") { put("ok", registry.probeHealth()) }

            // `wait` is the core's only clock. `delay` is cancellable, so an
            // abandoned timer stops rather than firing into a machine that moved
            // on — and the driver drops its answer either way.
            "wait" -> {
                delay(operation.optLong("ms"))
                result("waited") {}
            }

            "prompt" -> result("prompt_answered") {
                put(
                    "accepted",
                    deps.prompt(
                        PromptKind.from(operation.getJSONObject("kind")),
                        operation.optBoolean("confirmable"),
                    ),
                )
            }

            "complete_onboarding" -> {
                deps.complete(operation.getJSONObject("mode"))
                result("onboarding_completed") {}
            }

            else -> error("unhandled shell operation: $type")
        }

    /**
     * Possession-proven publish of the founding key set as one registry group.
     *
     * With a group seed in hand the members already carry creation-time proofs
     * and **no prompt is raised**; that is the whole point of the interleaved
     * create-then-confirm flow. The empty-seed path is the login re-publish: a
     * fresh group key, and one live assertion per member that has no proof.
     *
     * Returns the registry task when the core asked to be answered on
     * acceptance (`answer_when_accepted` — issue #409, a one-key create) and the
     * group has not landed yet; `null` once it has landed.
     */
    private suspend fun publish(operation: JSONObject): String? {
        val members = operation.optJSONArray("members").objects().map(PublishMember::from)
        if (members.isEmpty()) {
            throw RegistryFailure("registry publish needs at least one member", network = false)
        }
        // The route a member with no replayable proof signs its live possession
        // proof over — recovery's third signature. A caBLE-recovered wallet
        // signs it on the same phone; a create replays and never reaches here.
        val wire = operation.optString("method")
        val method = if (wire.isEmpty()) KeyMethod.Platform else KeyMethod.of(wire)

        var seedHex = operation.optString("group_seed_hex")
        var groupPublicKey = operation.optString("group_public_key_hex")
        if (seedHex.isEmpty() || groupPublicKey.isEmpty()) {
            seedHex = toHex(passkey.random(GROUP_SEED_BYTES), false)
            groupPublicKey = registryGroupPublicKeyFromSeed(seedHex)
        }

        val metadataHex = operation.optString("metadata_hex")
        // One relying party for the whole unit (ruling, 2026-09-23): the
        // contract stores a single `rpId` per unit. Spec 102: a set is minted
        // for ONE domain from its first key on, so the unit's rpId is the
        // op's page's domain (a custom-domain wallet) or the wallet's own —
        // the core folds it (`registry_rp_id`), no member carries a page.
        val page = pageOf(operation)
        val unitRpId = uniffi.vela_core_uniffi.trustedSignerRegistryRpId(page) ?: passkey.relyingPartyId
        // Issue #409: the publish is the create's last step, and these three
        // lines split it — the challenge, the write's acceptance, and the
        // landing (the on-chain receipt the core waits for before "Wallet
        // created"). Debug builds only, like every VelaLog line.
        val publishStarted = SystemClock.elapsedRealtime()
        val challenge = registry.groupChallenge(
            metadataHex = metadataHex,
            groupPublicKey = groupPublicKey,
            members = members,
            rpId = unitRpId,
        )
        VelaLog.event(
            "registry.publish",
            "challenge",
            "members" to members.size,
            "ms" to SystemClock.elapsedRealtime() - publishStarted,
        )

        val proven = members.map { member ->
            val existing = member.proof
            if (existing != null) {
                ProvenMember(member, existing)
            } else {
                val memberChallenge = challenge.memberChallenges[member.publicKeyHex.lowercase()]
                    ?: throw RegistryFailure(
                        "registry challenge is missing member ${member.publicKeyHex}",
                        network = false,
                    )
                val bytes = uniffi.vela_core_uniffi.fromHex(stripHex(memberChallenge))
                // Spec 102 R3: a custom-domain wallet proves its members on
                // its page (the op's `page`) — the keys are behind it and no
                // provider on this device holds them. The page is told the
                // place (`method`) and the key's transports, so the browser
                // goes straight to it.
                val assertion = if (page != null) {
                    assertionOf(
                        asserted(
                            onPage(
                                JSONObject()
                                    .put("type", "sign_member_proof")
                                    .put("credential_id", member.credentialIdHex)
                                    .put("public_key_hex", member.publicKeyHex)
                                    .put("attestation_hex", member.attestationHex)
                                    .put("transports", member.transports)
                                    .put("method", method.wire)
                                    .put("group_public_key_hex", groupPublicKey)
                                    .put("page", page),
                                expectedMemberChallenge = bytes,
                            ),
                        ),
                    )
                } else {
                    passkey.assert(
                        challenge = bytes,
                        credentialIdHex = member.credentialIdHex,
                        method = method,
                    )
                }
                ProvenMember(member, JSONObject(memberProof(assertion)))
            }
        }

        // The group key silently closes over the content hash — under the
        // unit's own relying party, the one the challenge was taken under and
        // the one the contract will store. Three places, one value: the
        // group proof's authenticator data, the challenge, and the write.
        val group = JSONObject(
            registryBuildGroupProof(seedHex, unitRpId, challenge.groupChallenge),
        )

        val ack = registry.registerGroup(
            metadataHex = metadataHex,
            groupPublicKey = groupPublicKey,
            groupProof = group.getJSONObject("proof"),
            members = proven,
            rpId = unitRpId,
        )

        val acceptedAt = SystemClock.elapsedRealtime()
        VelaLog.event(
            "registry.publish",
            "register",
            "ack" to ack.status.ifEmpty { "-" },
            "sinceStartMs" to acceptedAt - publishStarted,
        )

        when (val next = afterRegister(ack, operation.optBoolean("answer_when_accepted"))) {
            AfterRegister.Landed -> return null
            // Issue #409: a one-key create is answered here, at the 202. The
            // landing is the session's to confirm from now on — its own
            // `registry.landing` lines say when it does.
            is AfterRegister.Accepted -> {
                VelaLog.event(
                    "registry.publish",
                    "accepted",
                    "task" to next.taskId,
                    "totalMs" to acceptedAt - publishStarted,
                )
                return next.taskId
            }
            is AfterRegister.AwaitLanding -> {
                registry.awaitTask(next.taskId)
                val landedAt = SystemClock.elapsedRealtime()
                VelaLog.event(
                    "registry.publish",
                    "landed",
                    "waitedMs" to landedAt - acceptedAt,
                    "totalMs" to landedAt - publishStarted,
                )
                return null
            }
        }
    }

    /** What the register's answer leaves to do. */
    sealed interface AfterRegister {
        /** The group has a receipt on-chain (or the identical one already had). */
        data object Landed : AfterRegister

        /** Answer the core now with this task (issue #409). */
        data class Accepted(val taskId: String) : AfterRegister

        /** Poll this task until the group has landed, then answer. */
        data class AwaitLanding(val taskId: String) : AfterRegister
    }

    // -- spec 102 R3: a ceremony on a custom domain's page ---------------------

    /**
     * Run this operation's ceremony on its page instead of on the platform's
     * sheet — the page the core named in the op (`page`), which it names only
     * for a wallet on a custom domain (R3: a `getvela.app` ceremony runs in the
     * app, where the challenge is random or derived and there is nothing to
     * preview).
     *
     * The request is the CORE's (`trustedSignerCeremonyRequest` over the
     * operation's own wire JSON), so nothing here decides what the page is
     * asked or how a challenge is derived; the answer comes back already
     * judged. [expectedMemberChallenge] is the registry challenge this
     * executor fetched, which a member proof's answer must match exactly.
     */
    private suspend fun onPage(
        operation: JSONObject,
        expectedMemberChallenge: ByteArray? = null,
    ): TrustedSignerCeremonyOutcome {
        val page = pageOf(operation) ?: throw PasskeyFailure(FailureKind.Other, words(SIGNER_DOWN))
        val signer = trustedSigner ?: throw PasskeyFailure(FailureKind.NotSupported, words(SIGNER_DOWN))
        val operationJson = operation.toString()
        val id = toHex(passkey.random(CEREMONY_ID_BYTES), false)
        // A member proof needs the registry's deployment, because the page
        // computes the challenge itself and the published page reaches no
        // network to look it up (076). Asked for only when it is needed, so no
        // other ceremony waits on a round trip.
        val deployment = if (operation.optString("type") == "sign_member_proof") {
            registry.deployment()
        } else {
            null
        }
        val request = trustedSignerCeremonyRequest(
            operationJson,
            id,
            deps.walletName(),
            registry.baseUrl,
            deployment,
        ) ?: throw PasskeyFailure(FailureKind.Other, words(SIGNER_REFUSED))
        return signer.run(
            requestJson = request,
            operationJson = operationJson,
            expectedMemberChallenge = expectedMemberChallenge,
            page = page,
        )
    }

    /**
     * The core's own `Registration` / `Assertion`, reported as the result the
     * platform ceremony would have given. A refusal never reaches here — the
     * channel throws the [PasskeyFailure] the executor's failure contract
     * already answers every ceremony's failure in.
     */
    private fun registered(outcome: TrustedSignerCeremonyOutcome): JSONObject = when (outcome) {
        is TrustedSignerCeremonyOutcome.Registered -> wire(outcome.registrationJson)
        else -> throw PasskeyFailure(FailureKind.Other, words(SIGNER_MISMATCH))
    }

    private fun asserted(outcome: TrustedSignerCeremonyOutcome): JSONObject = when (outcome) {
        is TrustedSignerCeremonyOutcome.Asserted -> wire(outcome.assertionJson)
        else -> throw PasskeyFailure(FailureKind.Other, words(SIGNER_MISMATCH))
    }

    /**
     * A verdict's JSON, read. The bridge substitutes an empty string if the
     * core could not serialise its own value, and an unreadable answer must
     * reach the machine as a described ceremony failure rather than as a
     * `JSONException` nobody on this path catches.
     */
    private fun wire(json: String): JSONObject = runCatching { JSONObject(json) }.getOrElse {
        throw PasskeyFailure(FailureKind.Other, words(SIGNER_MISMATCH))
    }

    /** The machine's `Assertion` wire, back as the shell's own value. */
    private fun assertionOf(wire: JSONObject): Assertion = Assertion(
        credentialIdHex = wire.optString("credential_id"),
        signatureDerHex = wire.optString("signature_der_hex"),
        authenticatorDataHex = wire.optString("authenticator_data_hex"),
        clientDataJsonHex = wire.optString("client_data_json_hex"),
        userIdHex = wire.optString("user_id_hex").ifEmpty { null },
        authenticatorAttachment = wire.optString("authenticator_attachment"),
    )

    private fun memberProof(assertion: Assertion): String = registryBuildMemberProof(
        assertion.authenticatorDataHex,
        assertion.clientDataJsonHex,
        assertion.signatureDerHex,
    )

    /**
     * The challenge a proof purpose signs over.
     *
     * The label strings are preserved verbatim from the other clients — they are
     * part of the wire, not decoration. The two recovery purposes share a label
     * on purpose: what must differ between the two signatures is the challenge
     * BYTES, and the millisecond tail supplies that. The invariant is not
     * trusted to the shell either way — `recover_public_key_from_assertions`
     * returns nothing unless the two assertions pin down exactly one key, so a
     * repeated challenge fails closed in the core.
     */
    private fun challengeFor(purpose: String): ByteArray {
        val label = if (purpose == "verify") "vela-verify-" else "vela-recover-"
        return (label + System.currentTimeMillis()).toByteArray(Charsets.UTF_8)
    }

    companion object {
        /**
         * The one new rule of issue #409, apart from the HTTP so a JVM test can
         * pin it: when the core asked to be answered on acceptance, the 202's
         * task id IS the answer; otherwise the landing is waited for, as it
         * always was. `done` up front means the identical group was already
         * on-chain — idempotent by content hash, and just as landed as a fresh
         * one.
         */
        fun afterRegister(ack: RegisterAck, answerWhenAccepted: Boolean): AfterRegister {
            if (ack.status == "done") return AfterRegister.Landed
            val id = ack.id
                ?: throw RegistryFailure("register was accepted without a task id", network = false)
            return if (answerWhenAccepted) AfterRegister.Accepted(id) else AfterRegister.AwaitLanding(id)
        }

        /** The publish's answer: accepted with its task, or published. */
        fun publishedResult(taskId: String?): JSONObject =
            if (taskId != null) {
                JSONObject().put("type", "registry_accepted").put("task_id", taskId)
            } else {
                JSONObject().put("type", "registry_published")
            }

        /** Every operation this executor is required to handle (contract §1). */
        val OPERATIONS = listOf(
            "check_passkey_support",
            "register_passkey",
            "sign_proof",
            "generate_group_key",
            "sign_member_proof",
            "lookup_legacy_name",
            "authenticate_passkey",
            "load_accounts",
            "save_account",
            "save_pending_upload",
            "remove_pending_upload",
            "registry_publish",
            "registry_query_by_public_key",
            "registry_query_unit",
            "probe_index_health",
            "wait",
            "prompt",
            "complete_onboarding",
        )

        private const val GROUP_SEED_BYTES = 32
        private const val CHALLENGE_BYTES = 32

        /** Spec 075: the request id the page echoes back. */
        private const val CEREMONY_ID_BYTES = 8

        /**
         * Spec 102 R3: the page a ceremony runs on — the op's `page`, which the
         * core sets only for a custom-domain wallet. `null`: in the app.
         */
        fun pageOf(operation: JSONObject): String? = operation.nullableString("page")

        /** The corpus lines a ceremony that could not finish on its page is told in. */
        const val SIGNER_DOWN = "componentsUi.signing.signerDown"
        const val SIGNER_REFUSED = "componentsUi.signing.trustedSignerRefused"
        const val SIGNER_MISMATCH = "componentsUi.signing.trustedSignerMismatch"

        /**
         * The result variant an operation owes when its execution threw.
         *
         * This is the whole failure contract: every rejection lands here, and the
         * core sees a described outcome rather than an exception. An operation
         * missing from this map would leave the core waiting forever, so the
         * fallthrough is deliberate and loud.
         */
        fun failureFor(operation: JSONObject, error: Throwable): JSONObject =
            when (val type = operation.optString("type")) {
                "check_passkey_support" ->
                    result("passkey_support") { put("supported", false) }

                "register_passkey", "sign_proof", "authenticate_passkey" -> passkeyFailure(error)

                // Mixed: the ceremony and the challenge fetch can each fail, and
                // the core branches differently on the two. Classify by what
                // actually threw rather than by which operation it was.
                "sign_member_proof" ->
                    if (error is RegistryFailure) indexFailure(error) else passkeyFailure(error)

                "generate_group_key",
                "load_accounts",
                "save_account",
                "save_pending_upload",
                "remove_pending_upload",
                -> result("storage_failed") { put("message", describe(error)) }

                "registry_publish", "registry_query_by_public_key", "registry_query_unit" ->
                    indexFailure(error)

                // Best-effort and read-only: a lost name degrades the label,
                // never the flow.
                "lookup_legacy_name" -> result("legacy_name") { put("name", JSONObject.NULL) }

                "probe_index_health" -> result("index_health") { put("ok", false) }

                "wait" -> result("waited") {}

                // A dismissed dialog is a refusal, not an error.
                "prompt" -> result("prompt_answered") { put("accepted", false) }

                // The hand-over already happened as far as the core is concerned;
                // a failure here is the app's to survive, not the machine's.
                "complete_onboarding" -> result("onboarding_completed") {}

                else -> error("no failure variant for operation: $type")
            }

        /**
         * The net for an exception that escaped [perform] itself — a shell bug,
         * not an expected failure. It still has to answer, because a core left
         * waiting on an unanswered effect shows a spinner that never stops.
         */
        fun escapedFailure(operation: JSONObject, error: Throwable): String =
            runCatching { failureFor(operation, error).toString() }
                .getOrElse { result("storage_failed") { put("message", describe(error)) }.toString() }

        private fun passkeyFailure(error: Throwable): JSONObject {
            val failure = error as? PasskeyFailure
                ?: PasskeyFailure(FailureKind.Other, describe(error))
            return result("passkey_failed") {
                put("kind", failure.kind.wire)
                // A classified failure's copy comes from the classification; only
                // `other` and `not_supported` carry the platform's own words,
                // because those go into the bug report and must not be
                // prettified.
                put(
                    "message",
                    when (failure.kind) {
                        FailureKind.Cancelled -> JSONObject.NULL
                        else -> failure.message ?: JSONObject.NULL
                    },
                )
            }
        }

        private fun indexFailure(error: Throwable): JSONObject = result("index_failed") {
            put("message", describe(error))
            // The one bit of classification only a shell can supply: a request
            // that never arrived is not the same as one the server refused.
            put("network", (error as? RegistryFailure)?.network ?: true)
        }

        private fun describe(error: Throwable): String =
            error.message ?: error::class.simpleName ?: "unknown failure"

        private inline fun result(type: String, fill: JSONObject.() -> Unit): JSONObject =
            JSONObject().put("type", type).apply(fill)

        /** UTC, always: a stored `created_at_iso` carrying a local offset means
         *  something different the moment the phone changes time zone. */
        fun nowIso(): String = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.ROOT)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }
            .format(Date())

        private fun stripHex(value: String): String = value.removePrefix("0x")
    }
}

private fun Assertion.toWire(): JSONObject = JSONObject()
    .put("credential_id", credentialIdHex)
    .put("signature_der_hex", signatureDerHex)
    .put("authenticator_data_hex", authenticatorDataHex)
    .put("client_data_json_hex", clientDataJsonHex)
    .put("user_id_hex", userIdHex ?: JSONObject.NULL)
    .put("authenticator_attachment", authenticatorAttachment)

private fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map { optString(it) }

/**
 * Spec 102 R3: a ceremony on a custom domain's signing page, as onboarding
 * sees it — one passkey ceremony on [page], and its verdict.
 *
 * A port rather than the channel itself, so the onboarding executor keeps
 * knowing nothing about Custom Tabs, the integrity check or callbacks. A
 * refusal is a [PasskeyFailure], which is the vocabulary
 * [OnboardingExecutor.failureFor] already classifies every ceremony's failure
 * in: a closed page is `Cancelled`, anything else carries the sentence to show.
 */
fun interface TrustedSignerCeremonies {
    suspend fun run(
        requestJson: String,
        operationJson: String,
        expectedMemberChallenge: ByteArray?,
        page: String,
    ): uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
}
