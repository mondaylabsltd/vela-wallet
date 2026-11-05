package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.multisig.core.HttpAnswer
import app.getvela.wallet.feature.multisig.core.MultisigOperation
import app.getvela.wallet.feature.multisig.core.MultisigPeerTransport
import app.getvela.wallet.feature.multisig.core.MultisigRpcReply
import app.getvela.wallet.feature.multisig.core.MultisigShellPorts
import app.getvela.wallet.feature.multisig.core.MultisigShellResult
import app.getvela.wallet.feature.multisig.core.MultisigWalletRefusal
import app.getvela.wallet.feature.multisig.core.PeerArrival
import app.getvela.wallet.feature.multisig.core.PeerOpened
import app.getvela.wallet.feature.multisig.core.StoreRead
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.jsonOf
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.channels.Channel
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import uniffi.vela_core_uniffi.multisigCall
import uniffi.vela_dev_fixtures.FixtureAccountRecord
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert

/**
 * A simulated world behind the multisig shell — the Kotlin twin of the core's
 * own (`rust/crates/vela-core/tests/multisig_world/mod.rs`), the subset a
 * shell test walks: the genesis service and its mailbox, chains holding
 * Safe's own code and deployed Safes' storage, a store, the person's passkeys
 * signed by the parallel space's fixture keys (the dev-fixtures library), and
 * the relay's word on a user operation. Every port the machines call is
 * answered from here, so a test drives the REAL machines (the core over
 * uniffi) end to end with no I/O — on the JVM (the host dylib) and on a
 * device (the app's own .so).
 *
 * [canonicalCode] reads Safe's v1.4.1 runtime code by vector name
 * (`rust/crates/vela-core/tests/vectors/safe-code/<name>.hex`): the core
 * hashes what `eth_getCode` answers, so it must be the real bytes.
 */
class MultisigWorld(private val canonicalCode: (String) -> String) : MultisigShellPorts {

    override var onNotice: (suspend (JsonObject) -> Unit)? = null
    override var onAskRisk: (suspend (MultisigOperation.AskRisk) -> Boolean)? = null

    /** A deployed Safe on one chain (the world's `DeployedSafe`). */
    data class Safe(
        val owners: List<String>,
        val threshold: Int,
        val nonce: Int = 0,
        val modules: List<String> = emptyList(),
        val guard: String? = null,
        val handler: String = HANDLER_141,
        val singleton: String = SAFE_L2_141,
        val approved: Set<Pair<String, String>> = emptySet(),
    )

    /** What an execution does once its user operation lands. */
    data class Landing(val chain: Int, val safe: String, val after: Safe, val safeTxHash: String, val txHash: String, val userOpHash: String)

    // -- the person -----------------------------------------------------------

    val fixtures: List<FixtureAccountRecord> by lazy { fixtureAccounts() }

    /** The fixture accounts that are this device's Vela accounts (by index). */
    val locals = CopyOnWriteArrayList<Int>()

    /** This device's Vela accounts as the machines take them (`MultisigLocalAccount`). */
    fun localAccounts(): List<Map<String, Any?>> = locals.map { i ->
        val f = fixtures[i]
        mapOf(
            "address" to f.address,
            "name" to f.name,
            "keys" to listOf(mapOf("credential_id" to f.credentialIdHex, "public_key_hex" to f.publicKeyHex)),
            "route" to "vela",
        )
    }

    fun address(index: Int): String = fixtures[index].address

    @Volatile var cancelPasskey = false
    @Volatile var riskSigns = true
    val passkeys = CopyOnWriteArrayList<Pair<String, String>>()
    val risksAsked = CopyOnWriteArrayList<List<JsonElement>>()

    // -- the store -------------------------------------------------------------

    val store = ConcurrentHashMap<String, String>()

    fun storedAccounts(): List<JsonElement> =
        store["vela.multisigAccounts"]?.let { Wire.json.parseToJsonElement(it).arr }.orEmpty()

    // -- the genesis service ------------------------------------------------------

    @Volatile var serviceDown = false
    val records = ConcurrentHashMap<String, JsonElement>()
    val imports = CopyOnWriteArrayList<JsonElement>()
    val mailbox = CopyOnWriteArrayList<JsonObject>()
    val submits = CopyOnWriteArrayList<JsonElement>()
    val httpLog = CopyOnWriteArrayList<String>()

    // -- the chains --------------------------------------------------------------

    val safes = ConcurrentHashMap<Pair<Int, String>, Safe>()
    val codes = ConcurrentHashMap<Pair<Int, String>, String>()
    val rpcLog = CopyOnWriteArrayList<Triple<Int, String, JsonElement>>()
    val receipts = ConcurrentHashMap<String, JsonElement>()

    // -- the relay ----------------------------------------------------------------

    /** Every user operation sent: (chain, from, calls). */
    val sent = CopyOnWriteArrayList<Triple<Int, String, List<JsonElement>>>()
    @Volatile var onSend: Landing? = null
    @Volatile var userOp: Pair<String, String?>? = null

    val notices = CopyOnWriteArrayList<JsonObject>()

    fun deploy(chain: Int, address: String, safe: Safe) {
        codes[chain to address.lowercase()] = "0x6080"
        safes[chain to address.lowercase()] = safe
    }

    /** A multisig genesis for these owners (the core's `multisig_genesis`), as the core writes it. */
    fun genesis(owners: List<String>, threshold: Int, name: String, salt: String): JsonObject =
        Wire.json.parseToJsonElement(
            multisigCall("genesis", jo("owners" to owners, "threshold" to threshold, "name" to name, "saltNonce" to salt).toString()),
        ) as JsonObject

    /** A stored row, as the web writes `vela.multisigAccounts`. */
    fun storedRow(g: JsonObject, chainIds: List<Int>, recorded: Boolean = true): JsonObject = jo(
        "address" to g["address"], "genesis" to g["genesis"], "owners" to g["owners"],
        "threshold" to g["threshold"], "recorded" to recorded, "chainIds" to chainIds, "addedMs" to 1,
    )

    fun safeTxHash(chain: Int, safe: String, tx: JsonElement): String =
        Wire.json.parseToJsonElement(multisigCall("safeTxHash", jo("chainId" to chain, "safe" to safe, "tx" to tx).toString())).str!!

    // -- the ports -------------------------------------------------------------------

    override suspend fun http(method: String, url: String, body: String?, timeoutMs: Long): HttpAnswer = synchronized(this) {
        httpLog += "$method $url"
        fun answer(status: Int, json: JsonElement) = HttpAnswer(status, null, json.toString())
        val path = url.removePrefix(SERVICE).takeIf { url.startsWith(SERVICE) }
            ?: throw java.io.IOException("no host for $url")
        if (serviceDown) throw java.io.IOException("connection refused")
        when {
            path == "/api/health" -> answer(
                200,
                jo("service" to "safe-genesis-registry", "ok" to true, "homeChainId" to 100, "chains" to listOf(mapOf("chainId" to 100, "registry" to true))),
            )
            path.startsWith("/api/genesis/") ->
                records[path.removePrefix("/api/genesis/").lowercase()]?.let { answer(200, it) } ?: answer(404, jo("found" to false))
            path == "/api/import" && method == "POST" -> {
                val req = Wire.json.parseToJsonElement(body ?: "{}")
                imports += req
                val safe = req["safe"].str
                val g = req["genesis"]
                if (safe != null && g != null) {
                    records[safe.lowercase()] = jo("found" to true, "pending" to false, "genesis" to g, "recordedOn" to listOf(100), "readFrom" to 100)
                }
                answer(200, jo("status" to "recorded", "recordedOn" to listOf(100)))
            }
            path == "/api/proposals" && method == "POST" -> {
                val req = Wire.json.parseToJsonElement(body ?: "{}")
                submits += req
                answer(201, take(req))
            }
            path.startsWith("/api/proposals/") -> {
                val rest = path.removePrefix("/api/proposals/")
                val safe = rest.substringBefore('?')
                val chain = rest.substringAfter('?', "").split('&').firstNotNullOfOrNull { it.removePrefix("chainId=").takeIf { c -> c != it }?.toIntOrNull() }
                val mine = mailbox.filter { it["safe"].str.equals(safe, ignoreCase = true) && (chain == null || it["chainId"].long?.toInt() == chain) }
                if (chain != null) {
                    answer(200, jo("members" to jo("deployed" to false, "owners" to emptyList<String>(), "threshold" to "1", "nonce" to "0"), "proposals" to mine))
                } else {
                    val counts = mine.groupingBy { it["chainId"].long ?: 0 }.eachCount()
                    answer(200, jo("total" to mine.size, "chains" to counts.map { (c, n) -> mapOf("chainId" to c, "count" to n) }, "proposals" to mine))
                }
            }
            path.startsWith("/api/task/") -> answer(200, jo("status" to "done"))
            else -> answer(404, jo("error" to "no route"))
        }
    }

    /** The mailbox merges an approval into its proposal (or opens one). */
    private fun take(req: JsonElement): JsonObject {
        val chain = req["chainId"].long!!.toInt()
        val safe = req["safe"].str!!
        val hash = safeTxHash(chain, safe, req["safeTx"]!!)
        val now = System.currentTimeMillis()
        val approval = JsonObject(req["approval"]!!.let { it as JsonObject } + ("createdMs" to JsonPrimitive(now)))
        val owner = approval["owner"].str!!.lowercase()
        val at = mailbox.indexOfFirst { it["chainId"].long?.toInt() == chain && it["safeTxHash"].str.equals(hash, ignoreCase = true) }
        val status: String
        if (at >= 0) {
            val p = mailbox[at]
            val list = p["approvals"].arr
            if (list.any { it["owner"].str.equals(owner, ignoreCase = true) }) {
                status = "unchanged"
            } else {
                status = "approved"
                mailbox[at] = JsonObject(p + mapOf("approvals" to JsonArray(list + approval), "updatedMs" to JsonPrimitive(now)))
            }
        } else {
            mailbox += jo(
                "chainId" to chain, "safe" to safe, "safeTxHash" to hash, "safeTx" to req["safeTx"],
                "proposer" to approval["owner"], "approvals" to listOf(approval), "createdMs" to now, "updatedMs" to now,
            )
            status = "created"
        }
        val proposal = mailbox.first { it["chainId"].long?.toInt() == chain && it["safeTxHash"].str.equals(hash, ignoreCase = true) }
        return jo("status" to status, "proposal" to proposal)
    }

    /**
     * r14: every node hangs while this is set — a read waits until it is
     * completed (the network back) or its caller gives up, as a hung node.
     */
    @Volatile var hang: kotlinx.coroutines.CompletableDeferred<Unit>? = null

    /**
     * r14 leftovers: the networks whose every node answers "slow down" (HTTP
     * 429) — said as the app's pool says it ([app.getvela.wallet.feature.multisig.core.MultisigLive]).
     */
    @Volatile var rateLimited: Set<Int> = emptySet()

    override suspend fun rpc(chainId: Int, method: String, params: JsonElement): MultisigRpcReply {
        hang?.await()
        if (chainId in rateLimited) {
            synchronized(this) { rpcLog += Triple(chainId, method, params) }
            return MultisigRpcReply.Unanswered("every node of $chainId is rate limiting")
        }
        return answer(chainId, method, params)
    }

    private fun answer(chainId: Int, method: String, params: JsonElement): MultisigRpcReply = synchronized(this) {
        rpcLog += Triple(chainId, method, params)
        fun ok(v: Any?) = MultisigRpcReply.Result(jsonOf(v))
        when (method) {
            "eth_getCode" -> ok(codeAt(chainId, params.arr[0].str!!))
            "eth_getTransactionCount" -> ok("0x0")
            "eth_getStorageAt" -> {
                val s = safes[chainId to params.arr[0].str!!.lowercase()] ?: return@synchronized ok("0x" + "0".repeat(64))
                val v = when (params.arr[1].str) {
                    GUARD_SLOT -> s.guard.orEmpty()
                    HANDLER_SLOT -> s.handler
                    SINGLETON_SLOT -> s.singleton
                    else -> ""
                }
                ok("0x" + word(v.ifEmpty { "0x0" }))
            }
            "eth_call" -> ethCall(chainId, params.arr[0])
            "eth_estimateGas" -> MultisigRpcReply.Error(-32000, "gas required exceeds allowance")
            "eth_getBlockByNumber" -> ok(mapOf("number" to "0x64", "timestamp" to "0x" + (System.currentTimeMillis() / 1000).toString(16)))
            "eth_getLogs" -> {
                val hash = params.arr[0]["topics"].arr.getOrNull(1).str.orEmpty()
                ok(receipts.filter { (_, r) -> r["logs"].arr.any { it["topics"].arr.getOrNull(1).str.equals(hash, ignoreCase = true) } }
                    .map { (tx, _) -> mapOf("transactionHash" to tx, "blockNumber" to "0x64") })
            }
            "eth_getTransactionReceipt" -> MultisigRpcReply.Result(receipts[params.arr[0].str!!.lowercase()] ?: JsonNull)
            "eth_getBalance" -> ok("0x56bc75e2d63100000") // 100 of the coin
            else -> MultisigRpcReply.Error(-32601, "$method not here")
        }
    }

    private fun codeAt(chain: Int, address: String): String {
        val a = address.lowercase()
        codes[chain to a]?.let { return it }
        return CANONICAL[a]?.let { canonical.getOrPut(a) { canonicalCode(it) } } ?: "0x"
    }

    private val canonical = ConcurrentHashMap<String, String>()

    private fun ethCall(chain: Int, call: JsonElement): MultisigRpcReply {
        val to = call["to"].str!!.lowercase()
        val data = (call["data"].str ?: call["input"].str ?: "0x").lowercase()
        fun ok(v: String) = MultisigRpcReply.Result(JsonPrimitive(v))
        if (to == GENESIS_REGISTRY.lowercase()) return MultisigRpcReply.Error(3, "execution reverted")
        // `getConfiguration(address)` on Vela's shared passkey signer: zeros — no account here is configured.
        if (to == WEBAUTHN_SIGNER.lowercase() && data.startsWith("0xc44b11f7")) return ok("0x" + "0".repeat(192))
        val s = safes[chain to to] ?: return ok("0x")
        return when (data.take(10)) {
            "0xa0e67e2b" -> ok(addresses(s.owners))
            "0xe75235b8" -> ok("0x" + uint(s.threshold.toLong()))
            "0xaffed0e0" -> ok("0x" + uint(s.nonce.toLong()))
            "0xcc2f8452" -> ok("0x" + uint(64) + word("0x0000000000000000000000000000000000000001") + uint(s.modules.size.toLong()) + s.modules.joinToString("") { word(it) })
            "0x7d832974" -> {
                val owner = "0x" + data.substring(10 + 24, 10 + 64)
                val hash = "0x" + data.substring(10 + 64, 10 + 128)
                ok("0x" + uint(if ((owner to hash) in s.approved) 1 else 0))
            }
            else -> ok("0x")
        }
    }

    override suspend fun storeRead(key: String): StoreRead = StoreRead.Value(store[key])

    override suspend fun storeWrite(key: String, value: String?): Boolean {
        if (value == null) store.remove(key) else store[key] = value
        return true
    }

    /** r14-9: the machines' clock, moved on by a test (a look grown stale). */
    @Volatile var skewMs = 0.0

    override fun nowMs(): Double = System.currentTimeMillis().toDouble() + skewMs

    private var randomSeed = 7
    override fun random(bytes: Int): ByteArray = synchronized(this) {
        randomSeed = (randomSeed + 1) and 0xff
        ByteArray(bytes) { randomSeed.toByte() }
    }

    override suspend fun passkey(op: MultisigOperation.Passkey): MultisigShellResult {
        passkeys += op.owner to op.challenge
        if (cancelPasskey) return MultisigShellResult.PasskeyCancelled
        val local = fixtures.firstOrNull { it.address.equals(op.owner, ignoreCase = true) }
            ?: return MultisigShellResult.PasskeyFailed("no key for ${op.owner} here")
        val signed = fixtureAssert(bytes(op.challenge), listOf(local.credentialIdHex), null)
        return MultisigShellResult.Asserted(signed.credentialIdHex, signed.authenticatorDataHex, signed.clientDataJsonHex, signed.signatureDerHex)
    }

    override suspend fun askRisk(op: MultisigOperation.AskRisk): Boolean {
        risksAsked += op.findings
        return onAskRisk?.invoke(op) ?: riskSigns
    }

    override suspend fun sendCalls(op: MultisigOperation.SendCalls): MultisigShellResult {
        synchronized(this) {
            onSend?.let { land(it) }
            onSend = null
            sent += Triple(op.chain_id.toInt(), op.from, op.calls)
            sentFees += op.fee
        }
        return MultisigShellResult.CallsSent("0x" + "ab".repeat(32), null)
    }

    private fun land(l: Landing) {
        deploy(l.chain, l.safe, l.after)
        receipts[l.txHash.lowercase()] = jo(
            "logs" to listOf(
                mapOf("address" to l.safe, "topics" to listOf(EXECUTION_SUCCESS, l.safeTxHash.lowercase()), "data" to "0x"),
                mapOf("address" to ENTRY_POINT, "topics" to listOf(USER_OP_EVENT, l.userOpHash), "data" to "0x"),
            ),
        )
        userOp = "included" to l.txHash
    }

    /** Executors (lowercase) whose fee look the relay refuses — they cannot pay here. */
    val quoteRefusedFor: MutableSet<String> = ConcurrentHashMap.newKeySet()

    /** r14-9: the fee the relay quotes now (the gas price moves). */
    @Volatile var quoteAmount = "0x1"

    /** What each execution's operation carried as its fee (`send_calls.fee`). */
    val sentFees = java.util.concurrent.CopyOnWriteArrayList<JsonElement>()

    override suspend fun quoteCalls(op: MultisigOperation.QuoteCalls): MultisigShellResult =
        if (op.from.lowercase() in quoteRefusedFor) MultisigShellResult.QuoteFailed("the relay quotes nothing for ${op.from}")
        else MultisigShellResult.Quoted(jo("kind" to "quoted", "fee" to jo("token" to null, "amount" to quoteAmount), "totalGas" to "0x1", "executorDeployed" to true))

    override suspend fun simulate(op: MultisigOperation.Simulate): String? = null

    override suspend fun recipientFacts(op: MultisigOperation.RecipientRisk): Pair<Boolean?, Boolean?> = false to false

    override suspend fun userOpStatus(op: MultisigOperation.UserOpStatus): Pair<String?, String?>? =
        userOp?.let { it.first to it.second } ?: ("pending" to null)

    /** PeerConnect rooms: a receive waits until a test delivers (never answered by itself). */
    val rooms = ConcurrentHashMap<String, Channel<PeerArrival>>()
    val peerSent = CopyOnWriteArrayList<Pair<String, JsonElement>>()
    private var nextRoom = 0

    override val peer: MultisigPeerTransport = object : MultisigPeerTransport {
        override suspend fun open(relay: String?, invite: String?, pairing: String?): PeerOpened {
            val n = synchronized(this@MultisigWorld) { ++nextRoom }
            rooms.getOrPut("room-$n") { Channel(Channel.UNLIMITED) }
            return PeerOpened("room-$n", if (invite == null && pairing == null) "wss://relay#invite-$n" else null, pairing ?: "record-$n")
        }
        override suspend fun send(room: String, message: JsonElement) { peerSent += room to message }
        override suspend fun receive(room: String): PeerArrival = rooms.getOrPut(room) { Channel(Channel.UNLIMITED) }.receive()
        override suspend fun close(room: String, cancel: Boolean, end: Boolean) = Unit
    }

    override suspend fun wallet(method: String, params: JsonElement): JsonElement = throw MultisigWalletRefusal(4900, "no wallet connected")

    override suspend fun notice(notice: JsonObject) {
        notices += notice
        onNotice?.invoke(notice)
    }

    companion object {
        const val SERVICE = "http://service.test"
        const val FACTORY = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67"
        const val SAFE_141 = "0x41675C099F32341bf84BFc5382aF534df5C7461a"
        const val SAFE_L2_141 = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762"
        const val HANDLER_141 = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"
        const val TO_L2_SETUP_141 = "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54"
        const val GENESIS_REGISTRY = "0xF412388bF908E510066d3Be5c704bd9A327f945d"
        const val WEBAUTHN_SIGNER = "0x94a4F6affBd8975951142c3999aEAB7ecee555c2"
        const val ENTRY_POINT = "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
        const val EXECUTION_SUCCESS = "0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e"
        const val USER_OP_EVENT = "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f"
        const val GUARD_SLOT = "0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8"
        const val HANDLER_SLOT = "0x6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d5"
        const val SINGLETON_SLOT = "0x0"

        /** Safe's v1.4.1 code at its own addresses: what every chain here holds (vector names). */
        val CANONICAL = mapOf(
            FACTORY.lowercase() to "safe-proxy-factory-1.4.1",
            SAFE_141.lowercase() to "safe-1.4.1",
            SAFE_L2_141.lowercase() to "safe-l2-1.4.1",
            HANDLER_141.lowercase() to "compatibility-fallback-handler-1.4.1",
            TO_L2_SETUP_141.lowercase() to "safe-to-l2-setup-1.4.1",
        )

        private fun word(address: String) = address.removePrefix("0x").lowercase().padStart(64, '0')
        private fun uint(n: Long) = n.toString(16).padStart(64, '0')
        private fun addresses(list: List<String>) = "0x" + uint(32) + uint(list.size.toLong()) + list.joinToString("") { word(it) }

        fun bytes(hex: String): ByteArray {
            val h = hex.removePrefix("0x")
            return ByteArray(h.length / 2) { h.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
        }
    }
}
