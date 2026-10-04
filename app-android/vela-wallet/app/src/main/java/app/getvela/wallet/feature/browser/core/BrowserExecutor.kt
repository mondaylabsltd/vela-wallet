package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject

/**
 * The `dapp_browser` machine's arms (spec 070; the desktop's and iOS's are
 * the same list).
 *
 * ## What this file must never decide
 *
 * Whether an origin is connected, whether a frame may ask, what a method is,
 * which chain a site is on, which page an answer belongs to and which error
 * code a refusal carries. All of those are the core's, and every one is a
 * security rule — the reason this file used to have two siblings
 * (`DappRpc.kt`, `RequestRouter.kt`) and now has none. This performs: the
 * grant and chain store, strings into a named tab, a read through the
 * person's own pool, a receipt from the relay, the "connected to" row, and
 * the hand-off to the signing sheet.
 */
class BrowserExecutor(
    private val store: KeyValueStore,
    private val ports: Ports,
    /** The shell's clock, ms since the epoch: the core owns none, and times each request's row with this (spec 099). */
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {
    /** What a read through the pool came back with (spec 099 FR-009). */
    sealed class ReadAnswer {
        /** The JSON-RPC body — `{"result":…}` or the endpoint's own `{"error":…}`, which the core passes on. */
        data class Body(val json: JSONObject) : ReadAnswer()

        /** Nothing answered; [rateLimited] when the pool gave up because every endpoint said to slow down. */
        data class NoAnswer(val rateLimited: Boolean = false) : ReadAnswer()
    }

    interface Ports {
        /** Post [messageJson] into [tab]'s page. No such tab: drop it — never another tab. */
        fun deliver(tab: String, messageJson: String)

        /** A node (or bundler) read through the pool: its body, or why there is none. */
        suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): ReadAnswer

        /** The transaction hash a user operation landed in; `null` while it has not (or nobody answered). */
        suspend fun userOpTxHash(chainId: Int, userOpHash: String): String?

        /** Open the signing sheet for this request; its answer comes back as `signing_answered`. */
        fun forwardToSigning(operation: DbrOperation.ForwardToSigning)

        /** Close the sheet for this request: its page is gone and already answered. */
        fun cancelSigning(tab: String, id: String)

        /** The feed's "connected to <site>" row (`type: "connect"`). */
        fun saveConnectionRecord(row: JSONObject)
    }

    suspend fun perform(operation: DbrOperation): DbrShellResult = when (operation) {
        DbrOperation.ListSites -> DbrShellResult.SitesListed(listSites())
        is DbrOperation.WriteGrant -> {
            store.write(grantKey(operation.grant.origin), Wire.json.encodeToString(DpermGrant.serializer(), operation.grant))
            DbrShellResult.Ack
        }
        is DbrOperation.RemoveGrant -> {
            store.remove(grantKey(operation.origin))
            DbrShellResult.Ack
        }
        is DbrOperation.WriteSiteChain -> {
            store.write(chainKey(operation.origin), operation.chain_id.toString())
            DbrShellResult.Ack
        }
        is DbrOperation.Deliver -> {
            ports.deliver(operation.tab, operation.message_json)
            DbrShellResult.Ack
        }
        is DbrOperation.Read -> read(operation)
        is DbrOperation.ResolveUserOp -> {
            val txHash = ports.userOpTxHash(operation.chain_id, operation.user_op_hash)
            DbrShellResult.UserOpResolved(txHash, now_ms = now())
        }
        is DbrOperation.ForwardToSigning -> {
            ports.forwardToSigning(operation)
            DbrShellResult.Ack
        }
        is DbrOperation.CancelSigning -> {
            ports.cancelSigning(operation.tab, operation.id)
            DbrShellResult.Ack
        }
        is DbrOperation.SaveConnectionRecord -> {
            ports.saveConnectionRecord(connectionRow(operation.address, operation.chain_id, operation.origin, System.currentTimeMillis()))
            DbrShellResult.Ack
        }
        is DbrOperation.Log -> {
            VelaLog.event("browser", operation.line)
            DbrShellResult.Ack
        }
    }

    /** What an operation answers when performing it threw: the page is still settled, by the core. */
    fun neutralAnswer(operation: DbrOperation): DbrShellResult = when (operation) {
        DbrOperation.ListSites -> DbrShellResult.SitesListed(emptyList())
        is DbrOperation.Read -> DbrShellResult.ReadAnswered(null, now_ms = now())
        is DbrOperation.ResolveUserOp -> DbrShellResult.UserOpResolved(null, now_ms = now())
        else -> DbrShellResult.Ack
    }

    /**
     * One read, answered by the core's deadline (spec 099 FR-008) whatever
     * the endpoints are doing: the pool's answer races the deadline, and a
     * read that loses is answered `timed_out` at once — its late body, if one
     * ever comes, is dropped. The race does not wait for the I/O to notice it
     * was cancelled: a port that never checks for cancellation still costs
     * the page nothing past the deadline.
     */
    private suspend fun read(operation: DbrOperation.Read): DbrShellResult.ReadAnswered {
        val params = runCatching { JSONArray(operation.params_json) }.getOrElse { JSONArray() }
        val answer = CompletableDeferred<ReadAnswer>()
        // Not a child of this coroutine: its end is not waited for when the deadline wins.
        val work = CoroutineScope(currentCoroutineContext().minusKey(Job) + SupervisorJob()).launch {
            answer.complete(
                runCatching { ports.read(operation.chain_id, operation.method, params, operation.bundler) }
                    .getOrElse { error ->
                        if (error is kotlinx.coroutines.CancellationException) throw error
                        VelaLog.failure("browser.read", "read failed", error, "method" to operation.method)
                        ReadAnswer.NoAnswer()
                    },
            )
        }
        val answered = try {
            if (operation.deadline_ms > 0) {
                withTimeoutOrNull(operation.deadline_ms.toLong().coerceAtLeast(1L)) { answer.await() }
            } else {
                answer.await()
            }
        } finally {
            // The deadline won (or the caller went): the pool's call is stopped
            // where it can be, and whatever it brings back later goes nowhere.
            if (!answer.isCompleted) work.cancel()
        }
        return when (answered) {
            null -> {
                VelaLog.event("browser.read", "deadline passed: answered timed out", "method" to operation.method, "chain" to operation.chain_id)
                DbrShellResult.ReadAnswered(null, now_ms = now(), failure = DbrReadFailure.TimedOut)
            }
            is ReadAnswer.Body -> DbrShellResult.ReadAnswered(answered.json.toString(), now_ms = now())
            is ReadAnswer.NoAnswer -> DbrShellResult.ReadAnswered(
                null,
                now_ms = now(),
                failure = if (answered.rateLimited) DbrReadFailure.RateLimited else DbrReadFailure.NoEndpoint,
            )
        }
    }

    /**
     * Every stored grant and chain. An unreadable grant is no grant: guessing
     * one from a half-written record would connect a site nobody connected.
     */
    private suspend fun listSites(): List<DbrStoredSite> {
        val sites = LinkedHashMap<String, DbrStoredSite>()
        for (key in store.allKeys()) {
            when {
                key.startsWith(GRANT_PREFIX) -> {
                    val origin = key.removePrefix(GRANT_PREFIX)
                    val grant = store.read(key)?.takeIf { it.isNotBlank() }?.let { raw ->
                        runCatching { Wire.json.decodeFromString(DpermGrant.serializer(), raw) }.getOrNull()
                    } ?: continue
                    sites[origin] = (sites[origin] ?: DbrStoredSite(origin)).copy(grant = grant)
                }
                key.startsWith(CHAIN_PREFIX) -> {
                    val origin = key.removePrefix(CHAIN_PREFIX)
                    val chain = store.read(key)?.trim()?.toIntOrNull()?.takeIf { it > 0 } ?: continue
                    sites[origin] = (sites[origin] ?: DbrStoredSite(origin)).copy(chain_id = chain)
                }
            }
        }
        return sites.values.toList()
    }

    companion object {
        const val GRANT_PREFIX = "vela.perm."
        const val CHAIN_PREFIX = "vela.chain."

        /** `vela.perm.<origin>` — every client's key; one document per origin. */
        fun grantKey(origin: String): String = GRANT_PREFIX + origin

        /** `vela.chain.<origin>` — the extension's key for the chain a site is on. */
        fun chainKey(origin: String): String = CHAIN_PREFIX + origin

        // main's `resultJson`/`errorJson`/`eventJson`/`rejectMessage` are NOT
        // revived here: they speak `DpermPageEvent` and `DpermRejectReason`,
        // the pre-070 core's vocabulary, and 070 moved all four decisions into
        // `dapp_browser`. The thing they carried that matters — `error.kind`,
        // the machine-readable half of a refusal (spec 081) — survives: it is
        // on the core's own wire (`SignWire`'s `self_call_blocked` among the
        // rest), so every shell reads one spelling of it.

        /** The "Connected to <app>" row, field for field the other clients' (`buildConnectionRecord`). */
        fun connectionRow(address: String, chainId: Int, origin: String, nowMs: Long): JSONObject = JSONObject()
            .put("id", "dapp-$nowMs-connect")
            .put("userOpHash", "")
            .put("txHash", "")
            .put("from", address)
            .put("to", "")
            .put("value", "0")
            .put("symbol", "")
            .put("decimals", 0)
            .put("chainId", chainId)
            .put("timestamp", nowMs / 1000)
            .put("status", "confirmed")
            .put("type", "connect")
            .put("dappOrigin", origin)
    }
}
