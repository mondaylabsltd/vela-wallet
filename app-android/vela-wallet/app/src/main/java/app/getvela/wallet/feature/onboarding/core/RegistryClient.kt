package app.getvela.wallet.feature.onboarding.core

import java.io.IOException
import app.getvela.wallet.core.diagnostics.VelaLog
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/**
 * The public-key registry, over HTTP.
 *
 * Nothing here decides anything. The one judgement it makes is the `network` bit
 * on a failure — whether the request reached the server at all — because that is
 * the single fact only a shell can know, and the core needs it to tell "the
 * service said no" from "the service was not there".
 *
 * `HttpURLConnection` rather than a client library: the six calls below are the
 * whole surface, and adding an HTTP dependency to carry them would be more
 * machinery than the thing it carries.
 */
class RegistryClient(
    baseUrl: String = DEFAULT_REGISTRY_URL,
    /** The contract itself, for the two reads sign-in needs when the index is gone (spec 062). `null` = no fallback. */
    private val chain: RegistryChainReader? = null,
) {

    /**
     * Which chain listed the last key's units; `null` = the index did.
     *
     * Unit ids are per DEPLOYMENT (unit 10 on Gnosis is unit 0 on Ethereum), so
     * a key's units are asked of whoever listed them. The index mirrors Gnosis,
     * so an index listing may be continued on Gnosis; a chain listing is
     * continued on that chain and nowhere else.
     */
    private var unitSource: Int? = null

    var baseUrl: String = normalize(baseUrl)
        set(value) {
            field = normalize(value)
        }

    /**
     * MEMBER-mode challenge: one founding passkey confirming AT CREATION. Binds
     * only (groupPublicKey, own attestation), so it exists before the rest of
     * the set does — which is what makes the interleaved create-then-confirm
     * flow work.
     */
    suspend fun memberChallenge(
        groupPublicKey: String,
        publicKey: String,
        attestation: String,
        rpId: String,
    ): String {
        val body = JSONObject()
            .put("rpId", rpId)
            .put("groupPublicKey", groupPublicKey)
            .put("publicKey", publicKey)
        if (attestation.isNotEmpty()) body.put("attestation", attestation)
        return post("/api/challenge", body, READ_TIMEOUT_MS, "Challenge").getString("challenge")
    }

    /** GROUP-mode challenge: closing the group at publish. */
    suspend fun groupChallenge(
        metadataHex: String,
        groupPublicKey: String,
        members: List<PublishMember>,
        rpId: String,
    ): GroupChallenge {
        val body = JSONObject()
            .put("rpId", rpId)
            .put("groupPublicKey", groupPublicKey)
            .put(
                "members",
                JSONArray().apply {
                    members.forEach { member ->
                        put(
                            JSONObject().put("publicKey", member.publicKeyHex).apply {
                                if (member.attestationHex.isNotEmpty()) {
                                    put("attestation", member.attestationHex)
                                }
                            },
                        )
                    }
                },
            )
        if (metadataHex.isNotEmpty()) body.put("metadata", metadataHex)

        val answer = post("/api/challenge", body, READ_TIMEOUT_MS, "Challenge")
        val perMember = answer.optJSONArray("members").objects().associate { entry ->
            entry.getString("publicKey").lowercase() to entry.getString("challenge")
        }
        return GroupChallenge(
            groupChallenge = answer.getJSONObject("groupChallenge").getString("challenge"),
            memberChallenges = perMember,
        )
    }

    /**
     * Register the closed group.
     *
     * `members` is built here into the REGISTRY's camelCase shape. The core's
     * wire type is snake_case because it is generated from Rust; sending
     * `public_key_hex` where the server reads `publicKey` earns a
     * `members[0]: publicKey is required` — after the person has already minted
     * and confirmed every key. The two vocabularies meet in this one function.
     */
    suspend fun registerGroup(
        metadataHex: String,
        groupPublicKey: String,
        groupProof: JSONObject,
        members: List<ProvenMember>,
        rpId: String,
    ): RegisterAck {
        val body = JSONObject()
            .put("rpId", rpId)
            .put("groupPublicKey", groupPublicKey)
            .put("groupProof", groupProof)
            .put(
                "members",
                JSONArray().apply {
                    members.forEach { member ->
                        put(
                            JSONObject()
                                .put("publicKey", member.member.publicKeyHex)
                                .put("credentialId", member.member.credentialIdHex)
                                .put("proof", member.proof)
                                .apply {
                                    if (member.member.attestationHex.isNotEmpty()) {
                                        put("attestation", member.member.attestationHex)
                                    }
                                    if (member.member.authenticatorAttachment.isNotEmpty()) {
                                        put(
                                            "authenticatorAttachment",
                                            member.member.authenticatorAttachment,
                                        )
                                    }
                                    if (member.member.transports.isNotEmpty()) {
                                        put("transports", member.member.transports)
                                    }
                                },
                        )
                    }
                },
            )
        if (metadataHex.isNotEmpty()) body.put("metadata", metadataHex)

        val answer = post("/api/register", body, WRITE_TIMEOUT_MS, "Register")
        return RegisterAck(
            id = answer.nullableString("id"),
            status = answer.optString("status"),
        )
    }

    /**
     * Poll until terminal.
     *
     * A transient read failure is retried until the budget runs out: the task is
     * already accepted, so giving up on one bad read would report a failure that
     * did not happen — and the group may well be landing on-chain meanwhile.
     */
    suspend fun awaitTask(id: String) {
        var waited = 0L
        var lastError: String? = null
        while (waited < POLL_TIMEOUT_MS) {
            try {
                val task = get("/api/task/${encode(id)}", READ_TIMEOUT_MS, "Task status")
                when (task.optString("status")) {
                    "done" -> return
                    "failed" -> throw RegistryFailure(
                        "Register failed: ${task.nullableString("error") ?: "unknown"}",
                        network = false,
                    )
                }
            } catch (failure: RegistryFailure) {
                if (!failure.network) throw failure
                lastError = failure.message
            }
            delay(POLL_INTERVAL_MS)
            waited += POLL_INTERVAL_MS
        }
        throw RegistryFailure(
            "Register timed out after ${POLL_TIMEOUT_MS / 1000}s" +
                (lastError?.let { ": $it" } ?: ""),
            network = true,
        )
    }

    /** `/api/query?publicKey=` — is this key registered, and which groups does it found? */
    suspend fun queryByPublicKey(publicKeyHex: String): KeyStatus {
        val profile = try {
            get("/api/query?publicKey=${encode(publicKeyHex)}", READ_TIMEOUT_MS, "Query").also { unitSource = null }
        } catch (gone: RegistryFailure) {
            if (!gone.indexIsGone) throw gone
            // Nobody answered on-chain either: the original failure is the honest one.
            val (chainId, body) = chain?.keyProfile(publicKeyHex) ?: throw gone
            VelaLog.event("registry", "Query", "source" to "chain", "chain" to chainId)
            unitSource = chainId
            body
        }
        val ids = profile.optJSONObject("groups")?.optJSONArray("unitIds")
        val unitIds = buildList {
            for (i in 0 until (ids?.length() ?: 0)) {
                val value = ids!!.optLong(i, -1L)
                // The core speaks u32 unit ids because the wire is JSON. An id
                // past 2^32 would truncate into a DIFFERENT group, so this fails
                // the query instead of quietly fetching the wrong founding set.
                if (value < 0 || value >= U32_CEILING) {
                    throw RegistryFailure(
                        "Query failed: unit id out of u32 range in $ids",
                        network = false,
                    )
                }
                add(value)
            }
        }
        return KeyStatus(registered = !profile.isNull("entry"), unitIds = unitIds)
    }

    private suspend fun readUnit(unitId: Long): JSONObject {
        unitSource?.let { source ->
            return chain?.unit(source, unitId)
                ?: throw RegistryFailure("Query failed: chain $source did not answer for unit $unitId", network = true)
        }
        return try {
            get(
                "/api/query?unitId=${encode(unitId.toString())}&pageSize=$MAX_UNIT_MEMBERS&order=asc",
                READ_TIMEOUT_MS,
                "Query",
            )
        } catch (gone: RegistryFailure) {
            if (!gone.indexIsGone) throw gone
            // The index listed this unit and then went away. Its ids are Gnosis's.
            chain?.unit(RegistryChainReader.HOME_CHAIN, unitId) ?: throw gone
        }
    }

    /**
     * `/api/query?unitId=` — the group's frozen metadata and ALL its founding
     * members in ascending order, which IS the canonical founding order the Safe
     * address derivation pins.
     *
     * Both guards refuse rather than degrade: a group larger than a wallet's cap
     * is not ours, and a partial page would rebuild the address from a SUBSET of
     * the founding set — a different, wrong, fundable address.
     */
    suspend fun queryUnit(unitId: Long): UnitDetail {
        val detail = readUnit(unitId)
        val members = detail.optJSONObject("members")
        val total = members?.optInt("total") ?: 0
        val items = members?.optJSONArray("items").objects()
        if (total > MAX_UNIT_MEMBERS) {
            throw RegistryFailure(
                "Query failed: unit $unitId has $total members (cap $MAX_UNIT_MEMBERS)",
                network = false,
            )
        }
        if (items.size != total) {
            throw RegistryFailure(
                "Query failed: unit $unitId page holds ${items.size} of $total members",
                network = false,
            )
        }
        return UnitDetail(
            metadataHex = detail.getJSONObject("unit").getString("metadata"),
            members = items.map { item ->
                UnitMember(
                    credentialIdHex = item.optString("credentialId"),
                    publicKeyHex = item.optString("publicKey"),
                    authenticatorAttachment = item.optString("authenticatorAttachment"),
                    transports = item.optString("transports"),
                )
            },
        )
    }

    /** One health probe. Never throws: the core asked a yes/no question. */
    suspend fun probeHealth(): Boolean = try {
        val health = get("/api/health?_t=${System.currentTimeMillis()}", READ_TIMEOUT_MS, "Health")
        health.optString("service") in SERVICE_IDENTITIES && health.optString("status") == "ok"
    } catch (_: RegistryFailure) {
        false
    }

    /**
     * One raw GET, for a caller that reads the body itself: the status and the
     * text, or `null` when the request never arrived.
     *
     * This is the transport half of "the name behind a wallet ADDRESS". The v2
     * index cannot be asked that directly — `?walletRef=` answers 400, which is
     * what `nameForAddress` here asked for two specs — so the name is reached
     * through the chain, and the walk is the core's
     * (`feature/contacts/core/RegistryNameLookup`, issue 191). Best-effort by
     * construction: nothing here throws.
     */
    suspend fun rawGet(path: String): Pair<Int, String>? = withContext(Dispatchers.IO) {
        val connection = runCatching { URL(baseUrl + path).openConnection() as HttpURLConnection }.getOrNull()
            ?: return@withContext null
        try {
            connection.connectTimeout = READ_TIMEOUT_MS
            connection.readTimeout = READ_TIMEOUT_MS
            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            status to (stream?.bufferedReader(Charsets.UTF_8)?.use { it.readText() } ?: "")
        } catch (_: IOException) {
            null
        } finally {
            connection.disconnect()
        }
    }

    /**
     * The v1 index's display name for a credential — the only place a v1-era
     * wallet's name survives. Best-effort and read-only: a lost name degrades
     * the label, never the flow.
     */
    suspend fun legacyName(credentialIdHex: String): String? = try {
        get("/api/query?credentialId=${encode(credentialIdHex)}", READ_TIMEOUT_MS, "Legacy name")
            .nullableString("name")
            ?.trim()
            ?.takeIf { it.isNotEmpty() }
    } catch (_: RegistryFailure) {
        null
    }

    // -- transport -----------------------------------------------------------

    private suspend fun get(path: String, timeoutMs: Int, label: String): JSONObject =
        request(path, null, timeoutMs, label)

    private suspend fun post(
        path: String,
        body: JSONObject,
        timeoutMs: Int,
        label: String,
    ): JSONObject = request(path, body, timeoutMs, label)

    private suspend fun request(
        path: String,
        body: JSONObject?,
        timeoutMs: Int,
        label: String,
    ): JSONObject = withContext(Dispatchers.IO) {
        val startedAt = System.currentTimeMillis()
        val connection = try {
            (URL(baseUrl + path).openConnection() as HttpURLConnection)
        } catch (error: Exception) {
            VelaLog.failure("registry", label, error, "path" to path, "stage" to "connect")
            throw RegistryFailure("$label failed: ${error.message}", network = true)
        }
        try {
            connection.connectTimeout = timeoutMs
            connection.readTimeout = timeoutMs
            if (body != null) {
                connection.requestMethod = "POST"
                connection.doOutput = true
                connection.setRequestProperty("content-type", "application/json")
                connection.outputStream.use { it.write(body.toString().toByteArray(Charsets.UTF_8)) }
            }

            // Reading the status code is what forces the request to complete, so
            // it is also the line that separates the two failure worlds: a
            // throw here is a request that never arrived, while any code at all
            // means the server answered.
            val status = try {
                connection.responseCode
            } catch (error: IOException) {
                VelaLog.failure("registry", label, error, "path" to path, "stage" to "read")
                throw RegistryFailure("$label failed: ${error.message}", network = true)
            }
            VelaLog.event(
                "registry",
                label,
                "path" to path,
                "status" to status,
                "ms" to System.currentTimeMillis() - startedAt,
            )
            if (status !in 200..299) {
                throw RegistryFailure("$label failed: $status", network = false)
            }

            val text = connection.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            try {
                JSONObject(text)
            } catch (error: Exception) {
                // The server answered with something that is not our protocol.
                // That is an answer, not an outage: `network = false` keeps the
                // core from offering "check your connection" for a broken proxy.
                throw RegistryFailure("$label failed: malformed response", network = false)
            }
        } finally {
            connection.disconnect()
        }
    }

    private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8")

    companion object {
        /** The v2 registry. Overridable so a self-hosted stack is a setting, not a fork. */
        const val DEFAULT_REGISTRY_URL = "https://p256-index-v2.getvela.app"

        /** A 20-byte hex address, and nothing else, reaches the index. */
        private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

        /**
         * The health identities this endpoint accepts — the legacy index and the
         * v2 registry, so a wallet can point at either during the migration.
         */
        private val SERVICE_IDENTITIES = setOf(
            "webauthn-p256-publickey-registry",
            "webauthn-p256-publickey-index",
        )

        /** A vela wallet's founding set is capped at 7 keys; a larger group is
         *  not ours and must never be reconstructed into an account. */
        private const val MAX_UNIT_MEMBERS = 7

        private const val READ_TIMEOUT_MS = 15_000
        private const val WRITE_TIMEOUT_MS = 30_000
        private const val POLL_TIMEOUT_MS = 120_000L
        private const val POLL_INTERVAL_MS = 2_000L
        private const val U32_CEILING = 4_294_967_296L

        fun normalize(url: String): String =
            url.trim().replace("\r", "").replace("\n", "").trimEnd('/').ifEmpty { DEFAULT_REGISTRY_URL }
    }
}

/**
 * A registry call that did not produce an answer.
 *
 * [network] means the request never reached the server — a transport failure or
 * a timeout, as distinct from a refusal. Only a shell can tell those apart,
 * which is why this single bit of classification is delegated to it; everything
 * else about an index failure is the core's to interpret.
 */
class RegistryFailure(message: String, val network: Boolean) : Exception(message) {
    /** Unreachable, or failing (5xx): not an answer. A 4xx is the index saying no. */
    val indexIsGone: Boolean
        get() = network || Regex(" failed: 5\\d\\d$").containsMatchIn(message.orEmpty())
}

data class GroupChallenge(
    val groupChallenge: String,
    /** Keyed by lowercase public key — the server echoes the case it was sent. */
    val memberChallenges: Map<String, String>,
)

data class RegisterAck(val id: String?, val status: String)

data class KeyStatus(val registered: Boolean, val unitIds: List<Long>)

data class UnitDetail(val metadataHex: String, val members: List<UnitMember>)

data class UnitMember(
    val credentialIdHex: String,
    val publicKeyHex: String,
    val authenticatorAttachment: String,
    val transports: String,
)

/** One member of a publish, as the core hands it over. */
data class PublishMember(
    val credentialIdHex: String,
    val publicKeyHex: String,
    val attestationHex: String,
    val authenticatorAttachment: String,
    val transports: String,
    /** The proof collected AT CREATION. Absent on the login re-publish, whose
     *  executor signs the member live. */
    val proof: JSONObject?,
) {
    companion object {
        fun from(json: JSONObject): PublishMember = PublishMember(
            credentialIdHex = json.optString("credential_id"),
            publicKeyHex = json.optString("public_key_hex"),
            attestationHex = json.optString("attestation_hex"),
            authenticatorAttachment = json.optString("authenticator_attachment"),
            transports = json.optString("transports"),
            proof = if (json.isNull("proof")) null else json.optJSONObject("proof"),
        )
    }
}

/** A member whose possession proof is in hand. */
data class ProvenMember(val member: PublishMember, val proof: JSONObject)
