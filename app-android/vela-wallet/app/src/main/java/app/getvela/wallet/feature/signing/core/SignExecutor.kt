package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.UserOpSpine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.delay
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.UserOpCall

/**
 * The `sign_request` machine's seven arms (spec 044 T029; the desktop's
 * `executor/sign_request.rs`). Seven sentences, and the shell decides none
 * of them: single-flight, "a rejected pipeline may not submit", the
 * record-then-respond order and the account sequencing live in the core;
 * this answers the transport, writes the record, asks the relay, and runs
 * the ceremony — through [UserOpSpine], the SAME pipeline a person's own
 * transfer runs, deliberately.
 *
 * It reports twice: the accepted user-op hash mid-flight (`OpSubmitted`, so
 * the durable record precedes anything the dApp could poll) and the final
 * outcome. The page is answered with the user-op hash at once (the web's
 * non-blocking rule after spec 028's finding: blocking on the receipt made
 * `eth_sendTransaction` time out); `eth_getTransactionReceipt` for that
 * hash is translated by the router once the receipt exists.
 */
class SignExecutor(
    private val spine: UserOpSpine,
    private val relay: RelayClient,
    private val feed: FeedExecutor,
    private val ports: Ports,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    /** How long the final answer waits for the receipt (the desktop's `await_receipt`); then the op hash answers. */
    private val receiptWaitMs: Long = 120_000L,
    private val receiptPollMs: Long = 3_000L,
) {
    /** User-op hashes whose pending record has been written — the response never precedes the record. */
    private val persisted = MutableStateFlow<Set<String>>(emptySet())

    interface Ports {
        /** The answer, to the transport (tab) that owns the request. */
        fun respond(transportId: String, id: String, json: JSONObject)

        /** The relay accepted: the core must hear this BEFORE the submit resolves. */
        fun opSubmitted(id: String, userOpHash: String)

        fun signingStarted()

        /** The feed re-reads its store. */
        fun recordsPersisted()

        /** This record is on disk: the tracker may be handed its hash. */
        fun recordPersisted(recordId: String)

        /** The session's active account switch, verified by the caller. `true` when it happened. */
        suspend fun switchAccount(index: Int): Boolean

        /** The chain's native symbol for the record row. */
        fun nativeSymbol(chainId: Int): String
    }

    suspend fun perform(operation: SignOperation): SignShellResult = when (operation) {
        is SignOperation.SendResponse -> {
            ports.respond(operation.transport_id, operation.id, responseJson(operation.id, operation.payload))
            SignShellResult.Responded
        }
        // `null` means "proceed to submit" — including when a check itself
        // fails: the core's doc is explicit that a timed-out or errored
        // pre-check is not a refusal, and the submit's own underfunded answer
        // is the authority (the desktop answers the same).
        is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
        // Denied with no reason rather than invented: sponsorship is a relay
        // feature this shell does not reach, and `Funded` would be a claim
        // that somebody else paid.
        is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(SignSponsorship.Denied(null))
        is SignOperation.SignAndSubmit -> SignShellResult.Submit(signAndSubmit(operation), now())
        is SignOperation.PersistRecord -> {
            feed.writeRecords(listOf(recordRow(operation.record, ports.nativeSymbol(operation.record.chain_id))))
            persisted.value = persisted.value + operation.record.user_op_hash.lowercase()
            ports.recordsPersisted()
            ports.recordPersisted(operation.record.record_id)
            SignShellResult.RecordPersisted
        }
        is SignOperation.UpdateRecord -> {
            when (val close = operation.close) {
                is SignRecordClose.Confirmed -> feed.patchRecords(listOf(operation.record_id), "confirmed", close.tx_hash)
                SignRecordClose.Failed -> feed.patchRecords(listOf(operation.record_id), "failed", null)
            }
            ports.recordsPersisted()
            SignShellResult.RecordUpdated
        }
        // Best effort, and the core is told either way: it sequences "switch
        // first, then the approval surface may act" off this acknowledgement,
        // so withholding it would strand the grant.
        is SignOperation.SwitchActiveAccount -> {
            ports.switchAccount(operation.index)
            SignShellResult.AccountSwitched
        }
    }

    fun neutralAnswer(operation: SignOperation): SignShellResult = when (operation) {
        is SignOperation.SendResponse -> SignShellResult.Responded
        is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
        is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(SignSponsorship.Denied(null))
        is SignOperation.SignAndSubmit -> SignShellResult.Submit(SignSubmitOutcome.Failed("Signing failed"), now())
        is SignOperation.PersistRecord -> SignShellResult.RecordPersisted
        is SignOperation.UpdateRecord -> SignShellResult.RecordUpdated
        is SignOperation.SwitchActiveAccount -> SignShellResult.AccountSwitched
    }

    private suspend fun signAndSubmit(op: SignOperation.SignAndSubmit): SignSubmitOutcome {
        if (op.method == "personal_sign" || op.method == "eth_sign" || op.method.contains("signTypedData")) return signMessage(op)
        val calls = callsOf(op.method, op.params_json)
            ?: return SignSubmitOutcome.Failed("${op.method} carried no transaction this wallet could read")
        return try {
            val hash = spine.submit(
                chainId = op.chain_id,
                account = op.address,
                calls = calls,
                gasFeeToken = op.gas_fee_token,
                quotedFee = op.quoted_fee?.let { UserOpSpine.Quoted(it.amount, it.recipient, it.tier) },
                signingStarted = { ports.signingStarted() },
            )
            ports.opSubmitted(op.id, hash)
            // §4: the durable record precedes anything the dApp could poll —
            // the core persists it on `OpSubmitted`; the answer waits for it.
            withTimeoutOrNull(5_000) { persisted.first { hash.lowercase() in it } }
            // The desktop's `await_receipt`: a dApp's `eth_sendTransaction`
            // resolves to a TX hash; the op hash only when the receipt is late
            // (the tracker patches the row when it lands).
            SignSubmitOutcome.Succeeded(awaitReceipt(op.chain_id, hash) ?: hash)
        } catch (refused: UserOpSpine.Refused) {
            VelaLog.event("sign.submit", "refused", "why" to refused.failure.toString().take(120))
            when (val failure = refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled -> SignSubmitOutcome.PasskeyCancelled
                UserOpSpine.Failure.BundlerUnderfunded -> SignSubmitOutcome.Underfunded("The relay's gas account is underfunded", null)
                UserOpSpine.Failure.RelayerUnavailable -> SignSubmitOutcome.Failed("The gas relayer is unavailable right now")
                is UserOpSpine.Failure.Other -> SignSubmitOutcome.Failed(failure.message ?: "Signing failed")
            }
        }
    }

    /** A message: hashed the way the page's verifier hashes it, signed once, answered as the EIP-1271 envelope. */
    private suspend fun signMessage(op: SignOperation.SignAndSubmit): SignSubmitOutcome {
        val original = messageHash(op.method, op.params_json)
            ?: return SignSubmitOutcome.Failed("${op.method} carried nothing this wallet could sign")
        return try {
            SignSubmitOutcome.Succeeded(spine.signMessage(op.chain_id, op.address, original, signingStarted = { ports.signingStarted() }))
        } catch (refused: UserOpSpine.Refused) {
            when (val failure = refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled -> SignSubmitOutcome.PasskeyCancelled
                is UserOpSpine.Failure.Other -> SignSubmitOutcome.Failed(failure.message ?: "Signing failed")
                else -> SignSubmitOutcome.Failed("Signing failed")
            }
        }
    }

    private suspend fun awaitReceipt(chainId: Int, userOpHash: String): String? {
        val deadline = System.currentTimeMillis() + receiptWaitMs
        while (System.currentTimeMillis() < deadline) {
            when (val answer = relay.userOpReceipt(chainId, userOpHash)) {
                is RelayClient.ReceiptAnswer.Resolved -> return answer.txHash
                else -> delay(receiptPollMs)
            }
        }
        return null
    }

    companion object {
        /**
         * What the page's verifier will hash: `personal_sign` is the
         * EIP-191 prefix over the bytes (hex or text, the web's rule); typed
         * data is its EIP-712 digest, computed by the core.
         */
        fun messageHash(method: String, paramsJson: String): ByteArray? {
            val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return null
            // Spec 046 US2: `eth_sign` is `[address, data]` — the same EIP-191
            // envelope over `data` (EIP-1474's rule), the params swapped. The
            // sheet has already shown it as the danger it is (ClearSignMethod::EthSign).
            return if (method == "personal_sign" || method == "eth_sign") {
                val payload = params.optString(if (method == "eth_sign") 1 else 0).ifBlank { return null }
                val bytes = if (isHexPayload(payload)) SendExecutor.unhex(payload) else payload.toByteArray(Charsets.UTF_8)
                val prefix = "\u0019Ethereum Signed Message:\n${bytes.size}".toByteArray(Charsets.UTF_8)
                uniffi.vela_core_uniffi.keccak256(prefix + bytes)
            } else {
                val typed = params.opt(1)?.let { if (it is String) it else it.toString() } ?: return null
                runCatching { uniffi.vela_core_uniffi.hashTypedData(typed) }.getOrNull()
            }
        }

        private fun isHexPayload(payload: String): Boolean =
            payload.startsWith("0x") && payload.length % 2 == 0 && payload.drop(2).all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }

        /** The page's answer in the wire's shape; the core chose `ok`/`err` and the code. */
        fun responseJson(id: String, payload: SignResponsePayload): JSONObject = when (payload) {
            is SignResponsePayload.Ok -> BrowserExecutor.resultJson(id, payload.result)
            is SignResponsePayload.Err -> BrowserExecutor.errorJson(id, payload.code, payload.message ?: defaultMessage(payload.kind))
        }

        fun defaultMessage(kind: SignErrorKind): String = when (kind) {
            SignErrorKind.UserRejected -> "User rejected the request"
            SignErrorKind.WalletSwitchedChains -> "The wallet switched chains"
            SignErrorKind.UnsupportedChain -> "Unsupported chain"
            SignErrorKind.UnauthorizedAccount -> "Unauthorized account"
            SignErrorKind.InvalidParams -> "Invalid params"
            SignErrorKind.UnsupportedCapability -> "Unsupported capability"
            SignErrorKind.UnlimitedApproval -> "Unlimited approvals are disabled"
            SignErrorKind.FundingCancelled -> "Funding cancelled"
            SignErrorKind.SubmitFailed -> "The transaction could not be submitted"
            SignErrorKind.StaleFeeQuote -> "The fee quote expired"
        }

        /**
         * The calls a request carries (the desktop's `calls_of`): one for
         * `eth_sendTransaction`, many for `wallet_sendCalls` — and an empty
         * batch is not a batch. Hex value on the wire, decimal to the core.
         */
        fun callsOf(method: String, paramsJson: String): List<UserOpCall>? {
            val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return null
            val first = params.optJSONObject(0) ?: return null
            return when (method) {
                "wallet_sendCalls" -> {
                    val raw = first.optJSONArray("calls") ?: return null
                    val calls = (0 until raw.length()).mapNotNull { raw.optJSONObject(it)?.let(::call) }
                    calls.takeIf { it.isNotEmpty() }
                }
                else -> call(first)?.let { listOf(it) }
            }
        }

        private fun call(raw: JSONObject): UserOpCall? {
            val to = raw.optString("to").ifBlank { return null }
            val valueHex = raw.optString("value").ifBlank { "0x0" }
            val value = valueHex.removePrefix("0x").ifEmpty { "0" }.toBigIntegerOrNull(16) ?: return null
            return UserOpCall(to = to, value = value.toString(), data = raw.optString("data").ifBlank { "0x" })
        }

        /** The feed's row for a dApp's request (the desktop's `persist_record`, field for field). */
        fun recordRow(record: SignRecord, nativeSymbol: String): JSONObject {
            val first = runCatching { JSONArray(record.params_json).optJSONObject(0) }.getOrNull()
            val (kind, to, value, symbol, decimals) = when (record.kind) {
                SignRecordKind.DappTx -> Row("dapp_tx", first?.optString("to").orEmpty(), first?.optString("value")?.ifBlank { null } ?: "0x0", nativeSymbol, 18)
                // A signature moves nothing, and a row that claimed a value and
                // a symbol would show up in the feed as money.
                SignRecordKind.SignTypedData -> Row("sign_typed_data", "", "0", "", 0)
                SignRecordKind.SignMessage -> Row("sign_message", "", "0", "", 0)
            }
            val clipped = record.params_json.length > 4096
            return JSONObject()
                .put("id", record.record_id)
                .put("userOpHash", record.user_op_hash)
                .put("txHash", record.result)
                .put("from", record.from)
                .put("to", to)
                .put("value", value)
                .put("symbol", symbol)
                .put("decimals", decimals)
                .put("chainId", record.chain_id)
                .put("timestamp", (record.now_ms / 1000).toLong())
                .put("status", if (record.status == SignRecordStatus.Confirmed) "confirmed" else "pending")
                .put("type", kind)
                .put("dappOrigin", record.dapp_origin)
                .put("signedRequest", if (clipped) record.params_json.take(4096) else record.params_json)
                .put("requestTruncated", clipped)
                .apply { record.intent?.let { put("intent", it) } }
        }

        private data class Row(val kind: String, val to: String, val value: String, val symbol: String, val decimals: Int)
    }
}
