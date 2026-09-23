package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject

/**
 * Which passkeys control a wallet — the Android transport for
 * `vela_core::wallet_keys` (spec 062).
 *
 * A person offered "back up your keys" is owed the sight of them first. The
 * walk reads the wallet's founding set from the registry CONTRACT (Gnosis, then
 * the Ethereum backup; never our index), names each key and the vault holding
 * it, and falls back to what this device's account record remembers when no
 * chain answers. Every rule is the core's; this carries the `eth_call`s.
 */
class WalletKeys(
    /** The RAW `result` hex, or `null` when that chain did not answer. */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    private val step: (address: String, deviceKeysJson: String, answersJson: String) -> String =
        { address, device, answers -> uniffi.vela_core_uniffi.walletKeysStep(address, device, answers) },
) {
    enum class Source {
        /** The registry answered: every field is filled. */
        Registry,

        /** No chain answered: the device's own memory, and no sync badges. */
        Device,

        /** A chain answered and holds no record for this address. Nothing was unreachable. */
        NotRegistered,
    }

    /** One key as the account record holds it, in founding order. */
    /**
     * Spec 075: [signerOrigin] is the Trusted Signer page this key lives behind,
     * empty when it lives on an authenticator this device can reach itself.
     * Without it the core cannot tell a key minted on a page from the built-in
     * passkey — a page runs the ceremony in a browser and so reports
     * `platform` (the device pass of 2026-09-22).
     */
    data class DeviceKey(
        val publicKeyHex: String,
        val name: String,
        val transports: String,
        val signerOrigin: String = "",
    )

    data class Row(
        /** In the create flow's shape — what `PasskeyProviderMark` draws from. */
        val key: CreateKeyRow,
        /** `null` when only the device answered: nobody can vouch for a badge. */
        val synced: Boolean?,
        val publicKeyHex: String,
        /** base64url, as the registry explorer prints it; empty from the device. */
        val credentialId: String = "",
        /** Spec 075: the Trusted Signer page this key lives behind; empty when none. */
        val signerOrigin: String = "",
        /** The registry's 20-byte attestation summary, `0x`-hex; empty from the device. */
        val attestationHex: String = "",
        /** The authenticator verified the person at registration; `null` = nobody can vouch. */
        val userVerified: Boolean? = null,
    )

    data class Result(val source: Source, val rows: List<Row>)

    suspend fun read(address: String, device: List<DeviceKey>): Result {
        val deviceJson = JSONArray().apply {
            device.forEach {
                put(
                    JSONObject()
                        .put("public_key_hex", it.publicKeyHex)
                        .put("name", it.name)
                        .put("transports", it.transports)
                        .put("signer_origin", it.signerOrigin),
                )
            }
        }.toString()
        val answers = JSONArray()
        repeat(MAX_ROUNDS) {
            val next = runCatching { JSONObject(step(address, deviceJson, answers.toString())) }.getOrNull() ?: return Result(Source.Device, emptyList())
            if (next.optString("type") != "ask") return parse(next)
            val requests = next.optJSONArray("requests") ?: return Result(Source.Device, emptyList())
            coroutineScope {
                (0 until requests.length()).map { i ->
                    val request = requests.getJSONObject(i)
                    async {
                        val result = if (request.optString("type") == "eth_call") {
                            ethCall(request.optInt("chain_id"), request.optString("to"), request.optString("data"))
                        } else {
                            null
                        }
                        JSONObject()
                            .put("id", request.optString("id"))
                            .put("outcome", if (result != null) "ok" else "failed")
                            .put("body", result ?: JSONObject.NULL)
                    }
                }.awaitAll()
            }.forEach(answers::put)
        }
        // Never settled: what the device alone says, asked with no address so it can ask nobody.
        return runCatching { parse(JSONObject(step("", deviceJson, "[]"))) }.getOrDefault(Result(Source.Device, emptyList()))
    }

    private fun parse(done: JSONObject): Result {
        val source = when (done.optString("source")) {
            "registry" -> Source.Registry
            "not_registered" -> Source.NotRegistered
            else -> Source.Device
        }
        val keys = done.optJSONArray("keys") ?: JSONArray()
        val rows = (0 until keys.length()).map { i ->
            val key = keys.getJSONObject(i)
            val synced = if (key.isNull("synced")) null else key.optBoolean("synced")
            Row(
                key = CreateKeyRow(
                    name = key.optString("name"),
                    authenticatorAttachment = key.optString("authenticator_attachment"),
                    transports = key.optString("transports"),
                    confirmed = true,
                    synced = synced ?: true,
                    aaguid = key.optString("aaguid"),
                    providerName = key.optString("provider_name"),
                    method = KeyMethod.entries.firstOrNull { it.wire == key.optString("method") } ?: KeyMethod.Platform,
                ),
                synced = synced,
                publicKeyHex = key.optString("public_key_hex"),
                credentialId = key.optString("credential_id"),
                signerOrigin = key.optString("signer_origin"),
                attestationHex = key.optString("attestation_hex"),
                userVerified = if (key.isNull("user_verified")) null else key.optBoolean("user_verified"),
            )
        }
        return Result(source, rows)
    }

    private companion object {
        /** A handful of rounds; this only stops a contract bug from spinning. */
        const val MAX_ROUNDS = 24
    }
}
