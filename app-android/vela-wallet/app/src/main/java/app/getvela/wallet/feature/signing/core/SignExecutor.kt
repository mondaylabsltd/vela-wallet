package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.TrustedSignerIntent
import app.getvela.wallet.feature.send.core.UserOpSpine
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.delay
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.dappReceiptWaitMs
import uniffi.vela_core_uniffi.userOpNotSentDetail

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
    /**
     * How long the final answer waits for the receipt; then the op hash
     * answers. `null` (the app): the core's `dappReceiptWaitMs` — what is left
     * of the 120 s answer window since the approve, never under 10 s (spec 082
     * RA12). Tests pin a number.
     */
    private val receiptWaitMs: Long? = null,
    private val receiptPollMs: Long = 3_000L,
    /** The asking site's origin — what the Trusted Signer names as the requester (spec 071). */
    private val origin: () -> String = { "" },
    /** Spec 079: whether [origin] was read from the in-app browser (a page), not the wallet's own request. */
    private val originSeenByBrowser: () -> Boolean = { false },
) {
    /** User-op hashes whose pending record has been written — the response never precedes the record. */
    private val persisted = MutableStateFlow<Set<String>>(emptySet())

    interface Ports {
        /**
         * The answer, to the transport (tab) that owns the request. The core's
         * typed payload: a page's words and wire shape are `dapp_browser`'s
         * (spec 070), not this executor's.
         */
        fun respond(transportId: String, id: String, payload: SignResponsePayload)

        /**
         * The relay was handed the op: the core must hear this BEFORE the
         * submit resolves. [submitted] says whether the reply was lost (spec
         * 082 RA3) — the hash is then the local one — and the head before the
         * first POST; both go into `OpSubmitted` and on with the record.
         */
        fun opSubmitted(id: String, submitted: UserOpSpine.Submitted)

        /** Spec 082 RA9: the passkey (or the Trusted Signer's page) is up for request [id]. */
        fun ceremonyStarted(id: String) {}

        /** …and it returned a signature. */
        fun ceremonyDone(id: String) {}

        /**
         * Spec 082 RB2: the page that asked is gone (its tab closed or moved
         * on). Asked before the passkey and again between the passkey and the
         * relay POST: nothing is signed or sent for nobody.
         */
        fun askerGone(): Boolean = false

        /** The feed re-reads its store. */
        fun recordsPersisted()

        /** This record is on disk: the tracker may be handed its hash. */
        fun recordPersisted(recordId: String)

        /**
         * Make [address] the session's active account. `true` only when the
         * session's active address IS [address] afterwards — the machine's
         * row index is not the session's, so the switch is by address.
         */
        suspend fun switchAccount(address: String): Boolean

        /**
         * The machine's signer row at [index] (what `AccountsChanged` gave it);
         * `null` = no such row. [SigningController] answers it — it owns the
         * rows; the default refuses every switch.
         */
        fun signerAt(index: Int): String? = null

        /** The open request's `signer_address`; `null` = the request names none. [SigningController] answers it. */
        fun intendedSigner(): String? = null

        /** The chain's native symbol for the record row. */
        fun nativeSymbol(chainId: Int): String
    }

    suspend fun perform(operation: SignOperation): SignShellResult = when (operation) {
        is SignOperation.SendResponse -> {
            ports.respond(operation.transport_id, operation.id, operation.payload)
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
        // The core sequences "switch first, then the approval surface may act"
        // off this acknowledgement — so it is given only for the RIGHT
        // account (the web's `sign-resident` rule). The target row must be
        // the request's signer, checked BEFORE anything moves; a bad index or
        // a different account leaves the person's active account alone and
        // is never acknowledged: the approval surface stays shut, nothing is
        // signed.
        is SignOperation.SwitchActiveAccount -> {
            val intended = ports.intendedSigner()
            val target = ports.signerAt(operation.index)
            if (target == null || (intended != null && !target.equals(intended, ignoreCase = true))) {
                refuseSwitch(operation.index, intended, target)
            }
            if (!ports.switchAccount(target)) refuseSwitch(operation.index, intended, target)
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

    /** A switch that would sign as the wrong account: logged, and deliberately never answered. */
    private suspend fun refuseSwitch(index: Int, intended: String?, target: String?): Nothing {
        VelaLog.event(
            "sign.switch",
            "refused: the approval surface stays shut; nothing is signed",
            "index" to index,
            "intended" to (intended ?: "nothing"),
            "found" to (target ?: "nothing"),
        )
        awaitCancellation()
    }

    /** Thrown from the ceremony's edges when [Ports.askerGone]: unwinds the spine before the passkey or the POST. */
    private class AskerGone : RuntimeException("the asking page is gone")

    private fun stillAsked(op: SignOperation.SignAndSubmit) {
        if (!ports.askerGone()) return
        VelaLog.event("sign.submit", "asker gone: nothing signed or sent", "method" to op.method)
        throw AskerGone()
    }

    private suspend fun signAndSubmit(op: SignOperation.SignAndSubmit): SignSubmitOutcome {
        if (ports.askerGone()) return SignSubmitOutcome.AskerGone
        if (op.method == "personal_sign" || op.method == "eth_sign" || op.method.contains("signTypedData")) return signMessage(op)
        val calls = callsOf(op.method, op.params_json)
            ?: return SignSubmitOutcome.Failed("${op.method} carried no transaction this wallet could read")
        // The answer window runs from the approve (RA12); on this client the
        // pre-check and sponsorship answer at once, so the approve is now.
        val approvedAt = System.currentTimeMillis()
        return try {
            val submitted = spine.submit(
                chainId = op.chain_id,
                account = op.address,
                calls = calls,
                gasFeeToken = op.gas_fee_token,
                quotedFee = op.quoted_fee?.let { UserOpSpine.Quoted(it.amount, it.recipient, it.tier) },
                signingStarted = { stillAsked(op); ports.ceremonyStarted(op.id) },
                intent = TrustedSignerIntent(op.method, op.params_json, origin(), originSeenByBrowser()),
                // Between the passkey and the relay POST (RB2).
                ceremonyDone = { stillAsked(op); ports.ceremonyDone(op.id) },
            )
            val hash = submitted.userOpHash
            ports.opSubmitted(op.id, submitted)
            // §4: the durable record precedes anything the dApp could poll —
            // the core persists it on `OpSubmitted`; the answer waits for it.
            withTimeoutOrNull(5_000) { persisted.first { hash.lowercase() in it } }
            // The desktop's `await_receipt`: a dApp's `eth_sendTransaction`
            // resolves to a TX hash — a reverted one too (ruling 9); the op
            // hash only when the receipt is late — reported as `ReceiptPending`
            // so the core answers the page but leaves the record pending for
            // the tracker (issue 262). A lost reply (RA2) waits the same way,
            // polling the local hash: one answer, never "not sent".
            val wait = receiptWaitMs ?: dappReceiptWaitMs((System.currentTimeMillis() - approvedAt).toDouble()).toLong()
            afterReceiptWait(hash, awaitReceipt(op.chain_id, hash, wait))
        } catch (_: AskerGone) {
            SignSubmitOutcome.AskerGone
        } catch (refused: UserOpSpine.Refused) {
            VelaLog.event("sign.submit", "refused", "why" to refused.failure.toString().take(120))
            when (val failure = refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled -> SignSubmitOutcome.PasskeyCancelled
                UserOpSpine.Failure.BundlerUnderfunded -> SignSubmitOutcome.Underfunded("The relay's gas account is underfunded", null)
                UserOpSpine.Failure.RelayerUnavailable -> SignSubmitOutcome.Failed("The gas relayer is unavailable right now")
                // Nothing left the device (RA10): the core's fixed sentence,
                // never the pool's text.
                UserOpSpine.Failure.NotSent -> SignSubmitOutcome.Failed(userOpNotSentDetail())
                is UserOpSpine.Failure.Other -> SignSubmitOutcome.Failed(failure.message ?: "Signing failed")
            }
        }
    }

    /** A message: hashed the way the page's verifier hashes it, signed once, answered as the EIP-1271 envelope. */
    private suspend fun signMessage(op: SignOperation.SignAndSubmit): SignSubmitOutcome {
        val original = messageHash(op.method, op.params_json)
            ?: return SignSubmitOutcome.Failed("${op.method} carried nothing this wallet could sign")
        return try {
            SignSubmitOutcome.Succeeded(
                spine.signMessage(
                    op.chain_id, op.address, original,
                    signingStarted = { stillAsked(op); ports.ceremonyStarted(op.id) },
                    intent = TrustedSignerIntent(op.method, op.params_json, origin(), originSeenByBrowser()),
                    ceremonyDone = { ports.ceremonyDone(op.id) },
                ),
            )
        } catch (_: AskerGone) {
            SignSubmitOutcome.AskerGone
        } catch (refused: UserOpSpine.Refused) {
            when (val failure = refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled -> SignSubmitOutcome.PasskeyCancelled
                is UserOpSpine.Failure.Other -> SignSubmitOutcome.Failed(failure.message ?: "Signing failed")
                else -> SignSubmitOutcome.Failed("Signing failed")
            }
        }
    }

    private suspend fun awaitReceipt(chainId: Int, userOpHash: String, waitMs: Long): String? {
        val deadline = System.currentTimeMillis() + waitMs
        while (true) {
            val remaining = deadline - System.currentTimeMillis()
            if (remaining <= 0) break
            // Each poll gets only what is left of the window (spec 079,
            // device-found): with the relay unreachable a single poll hung for
            // its own timeouts and retries, and the page waited 268 s for a
            // two-minute wait. The pool's await is cancellable; a late answer
            // is dropped, and the tracker keeps following the operation.
            val answer = withTimeoutOrNull(remaining) { relay.userOpReceipt(chainId, userOpHash) }
            if (answer is RelayClient.ReceiptAnswer.Resolved) return answer.txHash
            val left = deadline - System.currentTimeMillis()
            if (left <= 0) break
            delay(minOf(receiptPollMs, left))
        }
        return null
    }

    companion object {
        /**
         * What the receipt wait means for the core: in time, `Succeeded` with
         * the TX hash; late, `ReceiptPending` with the op hash — never a
         * confirmation of something that may not have landed (issue 262).
         */
        fun afterReceiptWait(userOpHash: String, receipt: String?): SignSubmitOutcome =
            if (receipt != null) SignSubmitOutcome.Succeeded(receipt) else SignSubmitOutcome.ReceiptPending(userOpHash)

        /**
         * What the page's verifier will hash: `personal_sign` is the
         * EIP-191 prefix over the bytes (hex or text, the web's rule); typed
         * data is its EIP-712 digest, computed by the core.
         */
        /**
         * What the site asked to sign, before the Safe's wrap — the core's one
         * rule (`sign_message::original_hash`), which the desktop and the
         * Trusted Signer's page share. It used to be copied here, reading typed
         * data from `params[1]` even for `eth_signTypedData`, which carries it
         * first.
         */
        fun messageHash(method: String, paramsJson: String): ByteArray? =
            uniffi.vela_core_uniffi.signMessageHash(method, paramsJson)

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
                    // Every leg or none: a leg this reader refuses refuses the
                    // batch. Dropping it sent the others alone — a batch the
                    // page never asked for, answered as if it had run.
                    val calls = (0 until raw.length()).map { index -> raw.optJSONObject(index)?.let(::call) ?: return null }
                    calls.takeIf { it.isNotEmpty() }
                }
                else -> call(first)?.let { listOf(it) }
            }
        }

        private fun call(raw: JSONObject): UserOpCall? {
            val to = raw.optString("to").ifBlank { return null }
            // Spec 082 RC4/RC6: absent or a JSON null is zero (`optString` would
            // read the text "null" and refuse the call); a value that is not a
            // string — a JSON number — is refused, never read as hex text: the
            // sheet showed no figure for it, and 1000 must not leave as 0x1000.
            val valueHex = when (val value = raw.opt("value")) {
                null, JSONObject.NULL -> "0x0"
                is String -> value.ifBlank { "0x0" }
                else -> return null
            }
            val value = valueHex.removePrefix("0x").ifEmpty { "0" }.toBigIntegerOrNull(16) ?: return null
            return UserOpCall(to = to, value = value.toString(), data = raw.optString("data").ifBlank { "0x" })
        }

        /**
         * The feed's row for a dApp's request (the desktop's `persist_record`,
         * field for field). The call is the request's FIRST call — a
         * `wallet_sendCalls` batch's first leg, not the batch envelope — read
         * the way the sheet reads it ([SigningController.firstCall]: a JSON
         * null is absent, never the text "null").
         */
        fun recordRow(record: SignRecord, nativeSymbol: String): JSONObject {
            val call = SigningController.firstCall(record.params_json)
            val (kind, to, value, symbol, decimals) = when (record.kind) {
                SignRecordKind.DappTx -> Row("dapp_tx", call?.first.orEmpty(), call?.third ?: "0x0", nativeSymbol, 18)
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
                // Spec 082 T184: kept with the row, read back into the tracker's
                // pending record — a restart keeps a lost reply followed as one.
                .put("maybeSent", record.maybe_sent)
                .put("submitBlock", record.submit_block ?: JSONObject.NULL)
        }

        private data class Row(val kind: String, val to: String, val value: String, val symbol: String, val decimals: Int)
    }
}
