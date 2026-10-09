package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.TrustedSignerIntent
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.send.core.WriteAhead
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.delay
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.wallet.core.DappSummary
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import kotlinx.serialization.builtins.ListSerializer
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.dappReceiptWaitMs
import uniffi.vela_core_uniffi.userOpNotSentDetail
import uniffi.vela_core_uniffi.userOpRefusedDappDetail

/**
 * The `sign_request` machine's seven arms (spec 044 T029; the desktop's
 * `executor/sign_request.rs`). Seven sentences, and the shell decides none
 * of them: single-flight, "a rejected pipeline may not submit", the
 * record-then-respond order and the account sequencing live in the core;
 * this answers the transport, writes the record, asks the relay, and runs
 * the ceremony — through [UserOpSpine], the SAME pipeline a person's own
 * transfer runs, deliberately.
 *
 * It reports three times: the signed op before any POST (`OpSigned`, spec
 * 082 RJ1 — the core writes the record ahead, and the POST waits for its
 * `ClearToPost`), the accepted user-op hash mid-flight (`OpSubmitted`), and
 * the final outcome. The page is answered with the user-op hash at once (the web's
 * non-blocking rule after spec 028's finding: blocking on the receipt made
 * `eth_sendTransaction` time out); `eth_getTransactionReceipt` for that
 * hash is translated by the router once the receipt exists. A
 * `wallet_sendCalls` is answered by the core with its id — the op hash — the
 * moment `OpSubmitted` says the relay took it (spec 097 E); the receipt wait
 * then ends answered (RJ4), and its result is dropped. That answer is in the
 * shape the request declared — `{ id }` for EIP-5792 2.0.0 (spec 097 G) — and
 * reaches the page as the core formed it.
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
    /** Spec 082 RJ1: the write-ahead gate; tests pin its wait. */
    private val writeAhead: WriteAhead = WriteAhead(),
) {
    /** User-op hashes whose pending record has been written — the response never precedes the record. */
    private val persisted = MutableStateFlow<Set<String>>(emptySet())

    /**
     * The requests the core has answered (spec 082 RJ4): once it has — from
     * the tracker's verdict, `OpTracked` — the receipt wait for that request
     * ends; the page has its one answer and the tracker keeps following.
     */
    private val answeredIds = MutableStateFlow<Set<String>>(emptySet())

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

        /**
         * Spec 082 RJ1: the op for request [id] is signed and hashed, and
         * nothing has been POSTed — the core writes the record ahead
         * (`OpSigned`) and answers `ClearToPost` once it is on disk.
         */
        fun opSigned(id: String, userOpHash: String, submitBlock: Long?) {}

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
            answeredIds.value = answeredIds.value + operation.id
            SignShellResult.Responded
        }
        // RJ1: the record is on disk — the waiting submit may POST.
        is SignOperation.ClearToPost -> {
            writeAhead.clear(clearanceKey(operation.id, operation.user_op_hash), operation.user_op_hash)
            SignShellResult.Responded
        }
        // RJ1: the written-ahead op is proven never sent — its row goes.
        is SignOperation.DeleteRecord -> {
            feed.deleteRecords(listOf(operation.record_id))
            VelaLog.event("sign.record", "write-ahead record withdrawn: never sent")
            ports.recordsPersisted()
            SignShellResult.RecordUpdated
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
            val stored = feed.writeRecords(listOf(recordRow(operation.record, ports.nativeSymbol(operation.record.chain_id))))
            if (stored) writeAhead.written(operation.record.user_op_hash)
            persisted.value = persisted.value + operation.record.user_op_hash.lowercase()
            ports.recordsPersisted()
            ports.recordPersisted(operation.record.record_id)
            SignShellResult.RecordPersisted
        }
        is SignOperation.UpdateRecord -> {
            when (val close = operation.close) {
                is SignRecordClose.Confirmed -> feed.patchRecords(listOf(operation.record_id), "confirmed", close.tx_hash)
                SignRecordClose.Failed -> feed.patchRecords(listOf(operation.record_id), "failed", null)
                // RJ1: the relay took it — no longer "may have been sent"; still pending.
                SignRecordClose.Admitted -> feed.markAdmitted(listOf(operation.record_id))
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
        // No go for the submit: it waits out its clearance and posts nothing.
        is SignOperation.ClearToPost -> SignShellResult.Responded
        is SignOperation.DeleteRecord -> SignShellResult.RecordUpdated
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
                // RJ1: the record is written ahead, then the asker is asked
                // once more, immediately before the POST.
                beforePost = { localHash, submitBlock -> writtenAhead(op, localHash, submitBlock) },
            )
            val hash = submitted.userOpHash
            ports.opSubmitted(op.id, submitted)
            // §4: the durable record precedes anything the dApp could poll —
            // the core persists it on `OpSubmitted`; the answer waits for it.
            withTimeoutOrNull(5_000) { persisted.first { hash.lowercase() in it } }
            // A dApp's `eth_sendTransaction` resolves to a TX hash; a reverted
            // receipt is the core's revert error (083); a late one is
            // reported as `ReceiptPending`, which the core answers "not
            // confirmed yet" and leaves the record pending for the tracker
            // (issue 262). A lost reply (RA2) waits the same way, polling the
            // local hash: one answer, never "not sent", never the op hash.
            val wait = receiptWaitMs ?: dappReceiptWaitMs((System.currentTimeMillis() - approvedAt).toDouble()).toLong()
            val receipt = awaitReceipt(op.id, op.chain_id, hash, wait)
            afterReceiptWait(hash, receipt?.txHash, reverted = receipt?.confirmed == false)
        } catch (_: AskerGone) {
            SignSubmitOutcome.AskerGone
        } catch (refused: UserOpSpine.Refused) {
            VelaLog.event("sign.submit", "refused", "why" to refused.failure.toString().take(120))
            outcomeOf(refused.failure)
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
                UserOpSpine.Failure.PasskeyCancelled, is UserOpSpine.Failure.Other, is UserOpSpine.Failure.Signer,
                is UserOpSpine.Failure.VenueBlocked -> outcomeOf(failure)
                else -> SignSubmitOutcome.Failed("Signing failed")
            }
        }
    }

    /**
     * RJ1: the op is signed and hashed. The core writes its record ahead; the
     * POST goes only on its `ClearToPost`, and only while the page still asks
     * (RB2, immediately before the POST). Neither: nothing is posted.
     */
    private suspend fun writtenAhead(op: SignOperation.SignAndSubmit, localHash: String, submitBlock: Long?) {
        val clearance = writeAhead.expect(clearanceKey(op.id, localHash), localHash)
        ports.opSigned(op.id, localHash, submitBlock)
        if (!clearance.await()) throw UserOpSpine.Refused(UserOpSpine.Failure.NotCleared)
        stillAsked(op)
    }

    /**
     * The receipt wait (the desktop's `await_receipt`), which also ends the
     * moment the core has answered request [id] from the tracker's verdict
     * (spec 082 RJ4): the page has its one answer, and polling on would only
     * produce a result the core drops.
     */
    private suspend fun awaitReceipt(
        id: String,
        chainId: Int,
        userOpHash: String,
        waitMs: Long,
    ): RelayClient.ReceiptAnswer.Resolved? = coroutineScope {
        val polled = async { pollReceipt(chainId, userOpHash, waitMs) }
        val answered = async { answeredIds.first { id in it } }
        val receipt = select<RelayClient.ReceiptAnswer.Resolved?> {
            polled.onAwait { it }
            answered.onAwait {
                VelaLog.event("sign.receipt", "the core answered from the tracker: wait ended", "op" to userOpHash.take(12))
                null
            }
        }
        polled.cancel()
        answered.cancel()
        receipt
    }

    private suspend fun pollReceipt(chainId: Int, userOpHash: String, waitMs: Long): RelayClient.ReceiptAnswer.Resolved? {
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
            if (answer is RelayClient.ReceiptAnswer.Resolved) return answer
            val left = deadline - System.currentTimeMillis()
            if (left <= 0) break
            delay(minOf(receiptPollMs, left))
        }
        return null
    }

    companion object {
        /** The write-ahead gate's key on the dApp path: the request and its op. */
        private fun clearanceKey(id: String, userOpHash: String) = "$id|${userOpHash.lowercase()}"

        /**
         * A refused submit or signature, as the signing machine hears it. A
         * passkey that failed carries its kind (spec 099 R8) — the one the
         * app's passkey classifier gave create and login — so the core answers
         * `signer_unavailable` / `signer_not_discoverable` / `signer_failed`;
         * a dismissed prompt stays `passkey_cancelled`.
         */
        fun outcomeOf(failure: UserOpSpine.Failure): SignSubmitOutcome = when (failure) {
            UserOpSpine.Failure.PasskeyCancelled -> SignSubmitOutcome.PasskeyCancelled
            is UserOpSpine.Failure.Signer -> SignSubmitOutcome.Failed(failure.message, signer = failure.kind)
            UserOpSpine.Failure.BundlerUnderfunded -> SignSubmitOutcome.Underfunded("The relay's gas account is underfunded", null)
            UserOpSpine.Failure.RelayerUnavailable -> SignSubmitOutcome.Failed("The gas relayer is unavailable right now")
            // Nothing left the device (RA10, and RJ1's write-ahead that was
            // not cleared in time): the core's fixed sentence, never the
            // pool's text.
            UserOpSpine.Failure.NotSent, UserOpSpine.Failure.NotCleared -> SignSubmitOutcome.Failed(userOpNotSentDetail())
            // RJ3: the relay refused it — the page is told the core's
            // "refused" sentence, the sheet never says "try again".
            is UserOpSpine.Failure.Rejected -> SignSubmitOutcome.Failed(failure.message ?: userOpRefusedDappDetail(), refused = true)
            is UserOpSpine.Failure.Other -> SignSubmitOutcome.Failed(failure.message ?: "Signing failed")
            // Spec 102: this account cannot sign here — the core says why, translated.
            is UserOpSpine.Failure.VenueBlocked -> SignSubmitOutcome.VenueBlocked(failure.block)
        }

        /**
         * What the receipt wait means for the core: in time, `Succeeded` with
         * the TX hash; late, `ReceiptPending` with the op hash — never a
         * confirmation of something that may not have landed (issue 262).
         */
        /**
         * What the receipt wait means for the core: a receipt whose op
         * executed is its tx hash; one that REVERTED is reported as such —
         * the core answers the page the revert, never the hash a site reads
         * as done (083, owner ruling 2026-10-01); none yet is `ReceiptPending`,
         * which the core answers "not confirmed yet" for a transaction.
         */
        fun afterReceiptWait(userOpHash: String, receipt: String?, reverted: Boolean = false): SignSubmitOutcome =
            when {
                receipt == null -> SignSubmitOutcome.ReceiptPending(userOpHash)
                reverted -> SignSubmitOutcome.Reverted(userOpHash, receipt)
                else -> SignSubmitOutcome.Succeeded(receipt)
            }

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
         * The calls a request carries: one for `eth_sendTransaction`, many for
         * `wallet_sendCalls` — every leg or none, and an empty batch is not a
         * batch. The core's one reading (`tx_request::calls_of`, spec 096 F1),
         * value in decimal to the submit: every shell used to read `value` its
         * own way (this one accepted a negative bare string), and one request
         * could be signed as different amounts on different shells.
         */
        fun callsOf(method: String, paramsJson: String): List<UserOpCall>? =
            uniffi.vela_core_uniffi.dappRequestCalls(method, paramsJson)

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
            return JSONObject()
                .put("id", record.record_id)
                .put("userOpHash", record.user_op_hash)
                // Spec 093: empty for a signature — the disk keeps that it was
                // given, never the signature itself.
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
                // 083 H2: the origin the request arrived from, which Activity
                // names the site by.
                .put("dappUrl", record.dapp_url)
                // Spec 093: the request as the CORE kept it (≤ 8 KB, cut once,
                // in one place) — never a cut of this shell's own.
                .put("signedRequest", record.stored_request)
                .put("requestTruncated", record.request_truncated)
                .apply { record.intent?.let { put("intent", it) } }
                // Spec 093: what the request was, and (083 F1) what the sheet's
                // simulation said it moves — both the core's, kept verbatim and
                // handed back to the feed untouched.
                .apply { record.summary?.let { put("dappSummary", JSONObject(Wire.json.encodeToString(DappSummary.serializer(), it))) } }
                .apply { record.balance_changes?.let { put("balanceChanges", JSONArray(Wire.json.encodeToString(JUDGMENTS, it))) } }
                // Spec 082 T184: kept with the row, read back into the tracker's
                // pending record — a restart keeps a lost reply followed as one.
                .put("maybeSent", record.maybe_sent)
                .put("submitBlock", record.submit_block ?: JSONObject.NULL)
        }

        private data class Row(val kind: String, val to: String, val value: String, val symbol: String, val decimals: Int)

        private val JUDGMENTS = ListSerializer(TrustSimJudgment.serializer())
    }
}
