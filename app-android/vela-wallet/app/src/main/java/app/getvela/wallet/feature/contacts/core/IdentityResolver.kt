package app.getvela.wallet.feature.contacts.core

import app.getvela.wallet.core.data.KeyValueStore
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.keccak256

/**
 * A name for an address — the waterfall behind a counterparty's label (spec
 * 043 T048, the arm the 042 merge left marked for this spec).
 *
 * The contacts machine's `ResolveIdentity` and the send machine's
 * `resolve_identity` ask the same question about the same address, so they
 * ask it here. The desktop's `executor/identity.rs` is the twin; the web's
 * `services/recipient-identity.ts` the original.
 *
 * ## The order is the order
 *
 * 1. **The person's own accounts** — on disk, no network. A wallet that
 *    misses its own account labels it a stranger.
 * 2. **Cache**, positive entries only, 24 hours.
 * 3. **The passkey index** — a Vela user, by wallet reference.
 * 4. **Name services**, asked together, answered in priority order:
 *    `.bnb`, `.arb`, `.g`, Basename, ENS — each name forward-verified
 *    before it counts.
 *
 * ## Only positive results are cached
 *
 * The core says so (`ContactShellResult::IdentityResolved`, invariant ⑦): a
 * name registered a minute after somebody looked would otherwise be
 * invisible for a day. A miss costs a lookup; a cached miss costs the truth.
 *
 * ## A reverse record is a claim, not a name (spec 081, FR-010)
 *
 * `addr.reverse` is written by the address itself, so whoever controls an
 * address controls the string this class used to hand the UI: fund a fresh
 * address, name it after the contact somebody is about to pay, and the wallet
 * drew that name beside the payment. Every name a name service returns is now
 * put to `vela_core::app::name_verify`, which resolves it FORWARD and compares;
 * only a verified name is shown or cached. The rule is the core's, written once
 * for four shells — what is here is the transport.
 */
class IdentityResolver(
    private val store: KeyValueStore,
    /** `(address, name)` for every account this device signs for. */
    private val ownAccounts: suspend () -> List<Pair<String, String>> = { emptyList() },
    private val registryName: suspend (String) -> String? = { null },
    /** `eth_call` on one chain: the `result` hex, or `null` for "no answer" / `0x`. */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    /**
     * `verifiedNameStep` from the core. A seam so the JVM tests can script the
     * verdict, exactly as `RegistryNameLookup` holds one for its own walk.
     */
    private val verifiedNameStep: (
        chainId: Int,
        registry: String,
        address: String,
        name: String,
        answersJson: String,
    ) -> String = { chainId, registry, address, name, answers ->
        uniffi.vela_core_uniffi.verifiedNameStep(chainId.toUInt(), registry, address, name, answers)
    },
) {
    data class Identity(val name: String, val source: String)

    /** An ENS-compatible registry: `registry.resolver(node)` then `resolver.name(node)`. */
    data class NameService(
        /** The words the UI shows. The shell owns these, per `ContactIdentity`. */
        val label: String,
        val chainId: Int,
        val registry: String,
        /**
         * ENSIP-19 chains derive the reverse node from a registrar call
         * instead of `namehash("<addr>.addr.reverse")`.
         */
        val reverseRegistrar: String? = null,
    )

    /** The whole waterfall. `null` is a real answer: nobody knows this address. */
    suspend fun resolve(address: String): Identity? {
        if (!askable(address)) return null
        val wanted = address.lowercase()
        // 1. The person's own accounts, before anything reaches the network.
        ownAccounts().firstOrNull { (addr, name) -> addr.lowercase() == wanted && name.isNotBlank() }
            ?.let { (_, name) -> return Identity(name, "self") }
        // 2. Cache — positives only, so a miss simply is not here.
        cached(wanted)?.let { return it }
        // 3. The passkey index.
        runCatching { registryName(address) }.getOrNull()?.takeIf { it.isNotBlank() }?.let {
            return Identity(it, "passkey").also { identity -> remember(wanted, identity) }
        }
        // 4. Name services — asked together, the first by priority wins.
        val answers = coroutineScope {
            NAME_SERVICES.map { service -> async { runCatching { reverseResolve(wanted, service) }.getOrNull() } }.awaitAll()
        }
        NAME_SERVICES.forEachIndexed { index, service ->
            answers[index]?.let { name -> return Identity(name, service.label).also { remember(wanted, it) } }
        }
        return null
    }

    /**
     * One service's answer for one address, forward-verified, or `null`. A
     * name this returns has been proven to resolve back to `address`.
     */
    internal suspend fun reverseResolve(address: String, service: NameService): String? {
        val stripped = address.removePrefix("0x").lowercase()
        val reverseNode = when (val registrar = service.reverseRegistrar) {
            null -> namehash("$stripped.addr.reverse")
            else -> {
                val answer = ethCall(service.chainId, registrar, "0x$SEL_NODE${stripped.padStart(64, '0')}") ?: return null
                // A node is one word. Anything shorter is not one.
                if (answer.removePrefix("0x").length < 64) return null
                answer
            }
        }
        val node = reverseNode.removePrefix("0x")
        val resolverWord = ethCall(service.chainId, service.registry, "0x$SEL_RESOLVER$node")?.removePrefix("0x") ?: return null
        // The last 20 bytes of the word. A zero resolver means "no record",
        // which is the common answer and not an error.
        if (resolverWord.length < 40) return null
        val resolver = resolverWord.takeLast(40)
        if (resolver.all { it == '0' }) return null
        val nameWord = ethCall(service.chainId, "0x$resolver", "0x$SEL_NAME$node") ?: return null
        // What the address CLAIMS to be called. Nothing may draw it yet.
        val claimed = decodeName(nameWord) ?: return null
        return forwardVerified(address, service, claimed)
    }

    /**
     * The claimed name, but only if resolving it forward lands back on
     * `address`.
     *
     * Which calls are made, in what order, what an unanswered one means and the
     * comparison itself are all `name_verify`'s; this drives the transcript.
     * `null` covers both "this name is somebody else's" and "nobody answered" —
     * failing open on the second would let whoever poisons a record choose the
     * moment.
     */
    internal suspend fun forwardVerified(address: String, service: NameService, claimed: String): String? {
        val answers = JSONArray()
        repeat(MAX_VERIFY_ROUNDS) {
            val next = runCatching {
                JSONObject(verifiedNameStep(service.chainId, service.registry, address, claimed, answers.toString()))
            }.getOrNull() ?: return null
            if (next.optString("type") != "ask") {
                return if (next.optString("forward_state") == "verified") {
                    // The core hands back the name it PROVED, normalised.
                    // Drawing that and not the reverse record's own spelling is
                    // what keeps what is shown and what was checked together.
                    next.optString("name").ifBlank { null }
                } else {
                    null
                }
            }
            val requests = next.optJSONArray("requests") ?: return null
            for (i in 0 until requests.length()) {
                val request = requests.optJSONObject(i) ?: return null
                val body = runCatching {
                    ethCall(request.optInt("chain_id"), request.optString("to"), request.optString("data"))
                }.getOrNull()
                answers.put(
                    JSONObject()
                        .put("id", request.optString("id"))
                        // A call that did not answer is `failed`, never an empty
                        // body: the core's verdicts turn on the difference.
                        .put("outcome", if (body == null) "failed" else "ok")
                        .put("body", body ?: JSONObject.NULL),
                )
            }
        }
        return null
    }

    private suspend fun cached(lower: String): Identity? {
        val raw = store.read(CACHE_KEY) ?: return null
        val entry = runCatching { JSONObject(raw).optJSONObject(lower) }.getOrNull() ?: return null
        val at = entry.optDouble("at", Double.NaN)
        if (at.isNaN() || now() - at > CACHE_TTL_MS) return null
        val name = entry.optString("name").ifBlank { return null }
        return Identity(name, entry.optString("source").ifBlank { "ens" })
    }

    private suspend fun remember(lower: String, identity: Identity) {
        runCatching {
            val map = store.read(CACHE_KEY)?.let { runCatching { JSONObject(it) }.getOrNull() } ?: JSONObject()
            map.put(lower, JSONObject().put("name", identity.name).put("source", identity.source).put("at", now()))
            store.write(CACHE_KEY, map.toString())
        }
    }

    companion object {
        /**
         * `vela.recipientIdentity.v2` — `{ "0xlowercase": { name, source, at } }`,
         * one document (the desktop's shape).
         *
         * The `.v2` is the forward-verification rule arriving (FR-010):
         * everything the old key holds was cached under no rule at all, so a
         * poisoned name would have kept its place for a day after the fix
         * shipped. A new key retires the lot at once.
         */
        const val CACHE_KEY = "vela.recipientIdentity.v2"
        const val CACHE_TTL_MS = 24.0 * 60 * 60 * 1000

        /** Verification is three `eth_call`s at most; this only stops a spin. */
        const val MAX_VERIFY_ROUNDS = 8

        /** Priority order. Adding a service is a row here, as long as it follows the ENS registry pattern. */
        val NAME_SERVICES: List<NameService> = listOf(
            NameService(".bnb", 56, "0x08CEd32a7f3eeC915Ba84415e9C07a7286977956"),
            NameService(".arb", 42161, "0x4a067EE58e73ac5E4a43722E008DFdf65B2bF348"),
            NameService(".g", 1625, "0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17"),
            NameService("Basename", 8453, "0xb94704422c2a1e396835a571837aa5ae53285a95", reverseRegistrar = "0x79ea96012eea67a83431f1701b3dff7e37f9e282"),
            NameService("ENS", 1, "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"),
        )

        /** `resolver(bytes32)`, `name(bytes32)`, and ENSIP-19's `node(address)`. */
        const val SEL_RESOLVER = "0178b8bf"
        const val SEL_NAME = "691f3431"
        const val SEL_NODE = "bffbe61c"

        private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

        /**
         * Is this a real address worth asking about? The zero address is a
         * mint/burn counterparty, not a recipient: it has no identity and
         * asking 404s the index.
         */
        fun askable(address: String): Boolean =
            ADDRESS.matches(address) && !address.drop(2).all { it == '0' }

        /** EIP-137 namehash: `namehash("") = 0x00…00`, then fold the labels right to left. */
        fun namehash(name: String): String {
            var node = ByteArray(32)
            if (name.isNotEmpty()) {
                name.split('.').asReversed().forEach { label ->
                    val labelHash = keccak256(label.toByteArray(Charsets.UTF_8))
                    node = keccak256(node + labelHash)
                }
            }
            return "0x" + node.joinToString("") { "%02x".format(it) }
        }

        /**
         * An ABI `string` return, bounded and validated. A name is drawn next
         * to somebody's money, so an over-long or non-UTF-8 answer is refused
         * rather than truncated into something plausible-looking.
         */
        fun decodeName(hex: String): String? {
            val data = hex.removePrefix("0x")
            if (data.length < 128) return null
            val offset = data.take(64).toBigIntegerOrNull(16)?.toInt()?.times(2) ?: return null
            if (offset < 0 || offset + 64 > data.length) return null
            val length = data.substring(offset, offset + 64).toBigIntegerOrNull(16)?.toInt()?.times(2) ?: return null
            // The web's cap, in bytes.
            if (length <= 0 || length > 1024) return null
            val start = offset + 64
            if (start + length > data.length) return null
            val body = data.substring(start, start + length)
            val bytes = ByteArray(body.length / 2) { i -> body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte() ?: return null }
            val decoded = runCatching { String(bytes, Charsets.UTF_8) }.getOrNull()?.trim() ?: return null
            return decoded.ifEmpty { null }
        }
    }
}
