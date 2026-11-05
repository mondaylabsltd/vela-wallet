package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.JsonShell
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.json.JSONObject

/**
 * The Android shell for the core's multisig machines
 * (`vela_core::app::multisig_*`, over uniffi: `MultisigAccountsCore`,
 * `MultisigProposalsCore`, `MultisigMembersCore`, `MultisigOutsidePageCore`,
 * `MultisigOutsideSessionCore`). Every [MultisigOperation] any of the five
 * asks is answered here by exactly ONE [MultisigShellResult] — the web's
 * `shell/handler.ts`, line for line, and iOS's `MultisigExecutor.swift`.
 *
 * This file draws nothing and decides nothing. It performs I/O and reports
 * what happened in the vocabulary's own words: an HTTP answer as it came, a
 * JSON-RPC reply as the node gave it, a store key as it was. What to ask,
 * when, how often and what an answer means is the core's. The product's own
 * pipelines — the passkey and its trusted page, the risk question, the send
 * pipeline, simulation, recipient facts, the relay's word on a user
 * operation, the PeerConnect transport and where a notice goes — come in as
 * [MultisigPorts], so a test answers them from a simulated world and the app
 * from [MultisigLive].
 *
 * The contract the effect loop holds every shell to: [answer] never throws
 * (bar cancellation). A port that throws is answered with that operation's
 * own failure ([failure], the web's `operationFailure`).
 */
class MultisigExecutor(private val ports: MultisigPorts) {

    /** The `perform` a [app.getvela.wallet.core.crux.CoreDriver] takes. */
    val perform: suspend (JSONObject) -> String =
        JsonShell.perform(MultisigOperation.serializer(), MultisigShellResult.serializer()) { answer(it) }

    /** The driver's answer for an exception that escaped [perform]. */
    val escapedFailure: (JSONObject, Throwable) -> String =
        JsonShell.escapedFailure(
            MultisigOperation.serializer(),
            MultisigShellResult.serializer(),
            fallback = MultisigShellResult.Ack,
        ) { failure(it, "the shell failed") }

    /** Perform one operation: its one answer. */
    suspend fun answer(op: MultisigOperation): MultisigShellResult = try {
        run(op)
    } catch (cancelled: CancellationException) {
        throw cancelled
    } catch (error: Throwable) {
        failure(op, describe(error))
    }

    private suspend fun run(op: MultisigOperation): MultisigShellResult = when (op) {
        is MultisigOperation.Http -> try {
            val answer = ports.http(op.method, op.url, op.body, op.timeout_ms)
            MultisigShellResult.HttpAnswered(answer.status, answer.retryAfter, answer.body)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (error: Throwable) {
            MultisigShellResult.HttpUnreachable(describe(error))
        }

        is MultisigOperation.Rpc -> {
            val bound = op.bound_ms
            val reply = if (bound != null) {
                withTimeoutOrNull(bound.coerceAtLeast(0)) { ports.rpc(op.chain_id.toInt(), op.method, op.params) }
                    ?: MultisigRpcReply.Unanswered("no answer in $bound ms")
            } else {
                ports.rpc(op.chain_id.toInt(), op.method, op.params)
            }
            when (reply) {
                is MultisigRpcReply.Result -> MultisigShellResult.RpcResult(reply.value)
                is MultisigRpcReply.Error -> MultisigShellResult.RpcError(reply.code, reply.message, reply.data)
                is MultisigRpcReply.Unanswered -> MultisigShellResult.RpcUnanswered(reply.detail)
            }
        }

        is MultisigOperation.LoadStore -> {
            val entries = linkedMapOf<String, String?>()
            var failed: String? = null
            for (key in op.keys) {
                when (val read = ports.storeRead(key)) {
                    is StoreRead.Value -> entries[key] = read.value
                    StoreRead.Unreadable -> {
                        failed = "the store could not read $key"
                        break
                    }
                }
            }
            failed?.let { MultisigShellResult.StoreFailed(it) } ?: MultisigShellResult.StoreLoaded(entries)
        }

        is MultisigOperation.WriteStore ->
            if (ports.storeWrite(op.key, op.value)) {
                MultisigShellResult.StoreWritten
            } else {
                MultisigShellResult.StoreFailed("the store did not take ${op.key}")
            }

        // A wait the machine stopped waiting for is cancelled with its job;
        // the loop drops its answer either way.
        is MultisigOperation.Wait -> {
            delay(op.ms.coerceAtLeast(0))
            MultisigShellResult.Waited
        }

        MultisigOperation.Now -> MultisigShellResult.Now(ports.nowMs())

        is MultisigOperation.Random ->
            MultisigShellResult.Random("0x" + ports.random(op.bytes.coerceAtLeast(0)).joinToString("") { "%02x".format(it) })

        is MultisigOperation.Passkey -> ports.passkey(op)
        is MultisigOperation.AskRisk -> MultisigShellResult.RiskAnswered(ports.askRisk(op))
        // r14-9: an execution goes on only at the fee the person pressed on.
        is MultisigOperation.SendCalls -> ShownFee.refusal(op) ?: ports.sendCalls(op)
        is MultisigOperation.QuoteCalls -> ports.quoteCalls(op)
        is MultisigOperation.Simulate -> MultisigShellResult.Simulated(ports.simulate(op))

        is MultisigOperation.RecipientRisk -> {
            val facts = ports.recipientFacts(op)
            MultisigShellResult.RecipientFacts(facts.first, facts.second)
        }

        is MultisigOperation.UserOpStatus -> {
            val answer = ports.userOpStatus(op)
            MultisigShellResult.UserOpStatus(answer?.first, answer?.second)
        }

        is MultisigOperation.PeerOpen -> try {
            val opened = ports.peer.open(op.relay, op.invite, op.pairing)
            MultisigShellResult.PeerOpened(opened.room, opened.invite, opened.pairing)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (error: Throwable) {
            MultisigShellResult.PeerClosed(describe(error))
        }

        is MultisigOperation.PeerSend -> {
            ports.peer.send(op.room, op.message)
            MultisigShellResult.Ack
        }

        is MultisigOperation.PeerReceive -> when (val arrival = ports.peer.receive(op.room)) {
            is PeerArrival.Message -> MultisigShellResult.PeerMessage(arrival.message)
            is PeerArrival.Closed -> MultisigShellResult.PeerClosed(arrival.reason)
        }

        is MultisigOperation.PeerClose -> {
            ports.peer.close(op.room, op.cancel, op.end)
            MultisigShellResult.Ack
        }

        is MultisigOperation.WalletRequest -> try {
            MultisigShellResult.WalletAnswered(ports.wallet(op.method, op.params))
        } catch (refusal: MultisigWalletRefusal) {
            MultisigShellResult.WalletRefused(refusal.code, refusal.message ?: "refused")
        }

        is MultisigOperation.Notice -> {
            ports.notice(op.notice)
            MultisigShellResult.Ack
        }
    }

    companion object {
        /**
         * The failure each operation answers when its port could not — the
         * result the machine waiting on it reads as "it did not happen",
         * never a hang (the web's `operationFailure`).
         */
        fun failure(op: MultisigOperation, detail: String): MultisigShellResult = when (op) {
            is MultisigOperation.Http -> MultisigShellResult.HttpUnreachable(detail)
            is MultisigOperation.Rpc -> MultisigShellResult.RpcUnanswered(detail)
            is MultisigOperation.LoadStore, is MultisigOperation.WriteStore -> MultisigShellResult.StoreFailed(detail)
            is MultisigOperation.Wait -> MultisigShellResult.Waited
            MultisigOperation.Now -> MultisigShellResult.Now(System.currentTimeMillis().toDouble())
            is MultisigOperation.Random -> MultisigShellResult.Random("0x")
            is MultisigOperation.Passkey -> MultisigShellResult.PasskeyFailed(detail)
            is MultisigOperation.AskRisk -> MultisigShellResult.RiskAnswered(false)
            is MultisigOperation.SendCalls -> MultisigShellResult.CallsFailed(detail, maybe_sent = false)
            is MultisigOperation.QuoteCalls -> MultisigShellResult.QuoteFailed(detail)
            is MultisigOperation.Simulate -> MultisigShellResult.Simulated(null)
            is MultisigOperation.RecipientRisk -> MultisigShellResult.RecipientFacts(null, null)
            is MultisigOperation.UserOpStatus -> MultisigShellResult.UserOpStatus(null, null)
            is MultisigOperation.PeerOpen, is MultisigOperation.PeerReceive -> MultisigShellResult.PeerClosed(detail)
            is MultisigOperation.PeerSend, is MultisigOperation.PeerClose, is MultisigOperation.Notice -> MultisigShellResult.Ack
            is MultisigOperation.WalletRequest -> MultisigShellResult.WalletRefused(null, detail)
        }

        fun describe(error: Throwable): String =
            (error as? MultisigWalletRefusal)?.message ?: "${error::class.java.simpleName}: ${error.message ?: ""}".trimEnd(' ', ':')
    }
}

/** A JSON-RPC reply through the wallet's own pool: its `result`, the node's `error`, or nobody answered. */
sealed interface MultisigRpcReply {
    data class Result(val value: JsonElement) : MultisigRpcReply
    data class Error(val code: Long?, val message: String, val data: JsonElement? = null) : MultisigRpcReply
    data class Unanswered(val detail: String) : MultisigRpcReply
}

/** A store key as read: its value (`null`: never written), or a store that could not be read. */
sealed interface StoreRead {
    data class Value(val value: String?) : StoreRead
    data object Unreadable : StoreRead
}

/** One HTTP answer, as it came back. */
data class HttpAnswer(val status: Int, val retryAfter: String?, val body: String)

/** A room opened: its handle, and the invite / pairing record. */
data class PeerOpened(val room: String, val invite: String?, val pairing: String?)

sealed interface PeerArrival {
    data class Message(val message: JsonElement) : PeerArrival
    data class Closed(val reason: String) : PeerArrival
}

/** The PeerConnect rooms a machine opens (`peer_*`), as the web's `PeerTransport`. */
interface MultisigPeerTransport {
    /** Create (no invite or pairing), join (`invite`) or reopen (`pairing`) a room. Throws when it cannot. */
    suspend fun open(relay: String?, invite: String?, pairing: String?): PeerOpened
    suspend fun send(room: String, message: JsonElement)
    /** The next message, or the room's close (its reason). */
    suspend fun receive(room: String): PeerArrival
    suspend fun close(room: String, cancel: Boolean, end: Boolean)
}

/** An EIP-1193 refusal (`{ code, message }`). */
class MultisigWalletRefusal(val code: Long?, message: String) : Exception(message)

/**
 * The product pipelines a multisig operation reaches (the web's
 * `MultisigShellDeps`). Each answers in the vocabulary; a throw is answered
 * with the operation's failure.
 */
interface MultisigPorts {
    suspend fun http(method: String, url: String, body: String?, timeoutMs: Long): HttpAnswer
    suspend fun rpc(chainId: Int, method: String, params: JsonElement): MultisigRpcReply
    suspend fun storeRead(key: String): StoreRead
    suspend fun storeWrite(key: String, value: String?): Boolean
    fun nowMs(): Double
    fun random(bytes: Int): ByteArray
    /** The member's passkey, in Vela or on its trusted page. */
    suspend fun passkey(op: MultisigOperation.Passkey): MultisigShellResult
    /** The high-risk question (§3.5): `false` when nobody can be asked. */
    suspend fun askRisk(op: MultisigOperation.AskRisk): Boolean
    /** The send pipeline: `calls_sent` / `calls_failed` / `calls_cancelled`. */
    suspend fun sendCalls(op: MultisigOperation.SendCalls): MultisigShellResult
    /** The fee pipeline: `quoted` / `quote_failed`. */
    suspend fun quoteCalls(op: MultisigOperation.QuoteCalls): MultisigShellResult
    /** The signing sheet's simulation envelope, or `null`. */
    suspend fun simulate(op: MultisigOperation.Simulate): String?
    /** What the send flow knows of a recipient: (is_contract, first_time). */
    suspend fun recipientFacts(op: MultisigOperation.RecipientRisk): Pair<Boolean?, Boolean?>
    /** The relay's word on a user operation (status, tx hash), or `null`. */
    suspend fun userOpStatus(op: MultisigOperation.UserOpStatus): Pair<String?, String?>?
    val peer: MultisigPeerTransport
    /** One EIP-1193 request to an injected wallet. Android has none: a refusal. */
    suspend fun wallet(method: String, params: JsonElement): JsonElement
    /** Route a notice (another machine, the activity, a screen). */
    suspend fun notice(notice: JsonObject)
}

/**
 * The ports a hub runs its machines over: [MultisigPorts], plus the two
 * places the hub itself answers — where a notice goes (its router) and who
 * asks the high-risk question (its sheet).
 */
interface MultisigShellPorts : MultisigPorts {
    var onNotice: (suspend (JsonObject) -> Unit)?
    var onAskRisk: (suspend (MultisigOperation.AskRisk) -> Boolean)?
}
