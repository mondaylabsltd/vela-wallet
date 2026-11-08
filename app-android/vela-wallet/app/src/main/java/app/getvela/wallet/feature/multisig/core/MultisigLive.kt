package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.TrackLifecycle
import app.getvela.wallet.feature.send.core.TrustedSignerIntent
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.core.PageFailure
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerChannel
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.math.BigInteger
import java.security.SecureRandom
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.TrustedSignerApprovalInput
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.multisigApprovalRoute
import uniffi.vela_core_uniffi.trustedSignerApprovalRequest

/**
 * Why a passkey or a send did not happen, as the shell saw it — JSON in a
 * result's `detail`, the web's `ShellFailure` (`shell/failure.ts`), so the
 * shells hand the core, and their sheets, the same words.
 */
sealed class MultisigShellFailure {
    data class VenueBlocked(val block: JSONObject) : MultisigShellFailure()
    data object PageDeclined : MultisigShellFailure()
    data class PageSignFailed(val page: String, val message: String? = null) : MultisigShellFailure()
    data class Passkey(val failure: String, val timedOut: Boolean = false, val message: String? = null) : MultisigShellFailure()
    data object NotSent : MultisigShellFailure()
    data class CannotPay(val need: String, val have: String) : MultisigShellFailure()

    /**
     * r17-12 (the desktop's r16-18): a read before the passkey — the
     * deployment, the nonce — failed on a network that is failing: nothing
     * was signed or sent, said as that network out of reach.
     */
    data class ChainUnreachable(val chainId: Int) : MultisigShellFailure()
    data class Unknown(val message: String) : MultisigShellFailure()

    val detail: String
        get() = when (this) {
            is VenueBlocked -> JSONObject().put("kind", "venue_blocked").put("block", block)
            PageDeclined -> JSONObject().put("kind", "page_declined")
            is PageSignFailed -> JSONObject().put("kind", "page_sign_failed").put("page", page).put("message", message ?: JSONObject.NULL)
            is Passkey -> JSONObject().put("kind", "passkey").put("failure", failure).put("timedOut", timedOut).put("message", message ?: JSONObject.NULL)
            NotSent -> JSONObject().put("kind", "not_sent")
            is CannotPay -> JSONObject().put("kind", "cannot_pay").put("need", need).put("have", have)
            is ChainUnreachable -> JSONObject().put("kind", "chain_unreachable").put("chainId", chainId)
            is Unknown ->JSONObject().put("kind", "unknown").put("message", message)
        }.toString()

    companion object {
        /** A `detail` the shell wrote, or `null` for one it did not (the core's own words) — the web's `readShellFailure`. */
        fun read(detail: String?): JsonObject? {
            if (detail == null || !detail.startsWith("{")) return null
            val parsed = runCatching { Wire.json.parseToJsonElement(detail) as? JsonObject }.getOrNull() ?: return null
            return parsed.takeIf { it["kind"].str != null }
        }

        /**
         * r17-12: an execution's submit refused, as the core is told it —
         * the desktop's rule (`execute_failure`): the relay that never took
         * it (or could not be reached) is "not sent"; a failure before the
         * passkey that is a chain read nobody answered ([chainRead]), or on
         * a network the pool finds failing ([chainFailing]), is that network
         * out of reach. Anything else by [of]. Never "Something went wrong"
         * for what has a kind.
         */
        fun ofSubmit(
            failure: UserOpSpine.Failure,
            chainRead: Boolean,
            signing: Boolean,
            chainFailing: Boolean,
            chainId: Int,
        ): MultisigShellFailure = when {
            failure == UserOpSpine.Failure.NotSent || failure == UserOpSpine.Failure.RelayerUnavailable -> NotSent
            failure is UserOpSpine.Failure.Other && !signing && (chainRead || chainFailing) -> ChainUnreachable(chainId)
            else -> of(failure)
        }

        /** A spine refusal in the shell's failure words. */
        fun of(failure: UserOpSpine.Failure): MultisigShellFailure = when (failure) {
            UserOpSpine.Failure.PasskeyCancelled -> Passkey("cancelled")
            UserOpSpine.Failure.PasskeyTimedOut -> Passkey("cancelled", timedOut = true)
            is UserOpSpine.Failure.Signer -> Passkey(failure.kind.wire, message = failure.message)
            UserOpSpine.Failure.PageDeclined -> PageDeclined
            is UserOpSpine.Failure.PageFailed -> PageSignFailed(pageWire(failure.failure))
            is UserOpSpine.Failure.VenueBlocked -> VenueBlocked(JSONObject(failure.block.json()))
            UserOpSpine.Failure.NotSent -> NotSent
            UserOpSpine.Failure.RelayerUnavailable -> Unknown("The relay is unavailable right now.")
            UserOpSpine.Failure.BundlerUnderfunded -> Unknown("The relay cannot pay for transactions right now.")
            is UserOpSpine.Failure.Other -> Unknown(failure.message.orEmpty())
            is UserOpSpine.Failure.Rejected -> Unknown(failure.message ?: "The network refused this transaction.")
            UserOpSpine.Failure.PreviousPending -> Unknown("Waiting for the last transaction of this account on this network.")
            UserOpSpine.Failure.NonceUsed -> Unknown("Another transaction of this account went first.")
            // §35: the window closed before the EntryPoint saw it.
            UserOpSpine.Failure.Expired -> Unknown("The transaction expired before it was submitted; nothing was sent.")
            UserOpSpine.Failure.NotCleared -> Unknown("cancelled")
        }
    }
}

/**
 * The app's own pipelines behind the multisig shell ([MultisigPorts]) — the
 * web's `shell/live.ts`, iOS's `MultisigLive.swift`: the RPC pool, the
 * key-value store, the passkey (in Vela, or on the member's trusted signing
 * page), the high-risk question, the send and fee pipelines, the signing
 * sheet's simulation, recipient facts, the relay's word on a user
 * operation, the PeerConnect rooms.
 *
 * Each answers in the vocabulary and never decides: a failure is reported as
 * what it was ([MultisigShellFailure], carried as the `detail` the core
 * hands back in its views) so a sheet can say it in the corpus's words.
 */
class MultisigLive(
    private val pool: RpcPool,
    private val relay: RelayClient,
    private val spine: UserOpSpine,
    private val accountStore: AccountStore,
    private val store: KeyValueStore,
    /** The multisig's own fee session: a quote here never supersedes the send screen's. */
    private val fees: () -> SpeedControl,
    private val trustedSigner: () -> TrustedSignerChannel?,
    /** A chain's name and coin, for the page's request. */
    private val chainLabel: (Int) -> Pair<String?, String?> = { null to null },
    /** Whether this device has ever sent to [address] (`null`: its history does not read). */
    private val sentTo: suspend (String) -> Boolean? = { null },
    /** One of the person's own: an account or a multisig on this device (never a "first time" recipient). */
    private val own: (String) -> Boolean = { false },
    override val peer: MultisigPeerTransport = NoPeer,
) : MultisigShellPorts {

    private val accountPort = StoreAccountPort(accountStore)
    private val random = SecureRandom()

    /** Where a notice goes (the hub's router), set once the hub exists. */
    @Volatile
    override var onNotice: (suspend (JsonObject) -> Unit)? = null

    /** The high-risk question (§3.5): the hub's sheet. `null`: nobody can be asked, so "do not sign". */
    @Volatile
    override var onAskRisk: (suspend (MultisigOperation.AskRisk) -> Boolean)? = null

    /** A first-party transaction through the signing sheet (a multisig's record saved to Ethereum). */
    @Volatile
    var onFirstParty: (suspend (MultisigOperation.SendCalls) -> MultisigShellResult)? = null

    /** The approvals pages answered, by member, chain and hash: the same bytes approved twice are one approval. */
    private val pageAnswers = java.util.concurrent.ConcurrentHashMap<String, String>()

    // -- HTTP -------------------------------------------------------------------

    override suspend fun http(method: String, url: String, body: String?, timeoutMs: Long): HttpAnswer =
        withContext(Dispatchers.IO) {
            val request = Request.Builder().url(url).apply {
                val payload = body?.toRequestBody("application/json".toMediaType())
                method(method.uppercase(), payload ?: if (method.equals("POST", true) || method.equals("PUT", true)) ByteArray(0).toRequestBody() else null)
                header("cache-control", "no-store")
            }.build()
            val client = VelaHttp.client.newBuilder().callTimeout(timeoutMs.coerceAtLeast(1_000), TimeUnit.MILLISECONDS).build()
            suspendCancellableCoroutine { cont ->
                val call = java.util.concurrent.atomic.AtomicReference<okhttp3.Call?>(null)
                cont.invokeOnCancellation { call.get()?.cancel() }
                try {
                    // r12-5: a mailbox read that met a pooled connection the
                    // service (or a proxy, a tunnel) had closed is asked again
                    // on a new one — never answered `http_unreachable`, which
                    // the core counts as the service away ("Multisig service
                    // unreachable" while every read it got was a 200). A write
                    // goes on a connection of its own (`VelaHttp.execute`).
                    VelaHttp.execute(client, request) { call.set(it); if (!cont.isActive) it.cancel() }.use { response ->
                        cont.resumeWith(Result.success(HttpAnswer(response.code, response.header("retry-after"), response.body?.string().orEmpty())))
                    }
                } catch (error: Throwable) {
                    if (cont.isActive) cont.resumeWith(Result.failure(error))
                }
            }
        }

    // -- RPC --------------------------------------------------------------------

    override suspend fun rpc(chainId: Int, method: String, params: JsonElement): MultisigRpcReply {
        val list: List<Any?> = when (params) {
            is JsonArray -> params.map(::toOrg)
            JsonNull -> emptyList()
            else -> listOf(toOrg(params))
        }
        return when (val answer = pool.call(chainId, method, list)) {
            is RpcResult.Body -> {
                val body = runCatching { Wire.json.parseToJsonElement(answer.json.toString()) }.getOrNull()
                val error = body["error"]
                when {
                    error != null -> MultisigRpcReply.Error(error["code"].long, error["message"].str ?: "error", (error as? JsonObject)?.get("data")?.takeUnless { it is JsonNull })
                    body is JsonObject && body.containsKey("result") -> MultisigRpcReply.Result(body["result"] ?: JsonNull)
                    else -> MultisigRpcReply.Unanswered("no result from chain $chainId")
                }
            }
            is RpcResult.Failed ->
                MultisigRpcReply.Unanswered(if (answer.rateLimited) "every node of $chainId is rate limiting" else "no node of $chainId answered")
            is RpcResult.RangeCapped -> MultisigRpcReply.Unanswered("the node capped the range")
        }
    }

    // -- store ------------------------------------------------------------------

    override suspend fun storeRead(key: String): StoreRead = StoreRead.Value(store.read(key))

    override suspend fun storeWrite(key: String, value: String?): Boolean =
        if (value == null) store.remove(key) else store.write(key, value)

    override fun nowMs(): Double = System.currentTimeMillis().toDouble()

    override fun random(bytes: Int): ByteArray = ByteArray(bytes).also(random::nextBytes)

    // -- passkey ----------------------------------------------------------------

    /**
     * [MultisigOperation.Passkey.owner] signs the challenge: in Vela, or on its
     * trusted page for an approval routed there (the web's `passkey`). Where
     * is the core's (`multisigApprovalRoute`, `signingRoute`), never this.
     */
    override suspend fun passkey(op: MultisigOperation.Passkey): MultisigShellResult {
        val accountJson = accountPort.accountJson(op.owner)
            ?: return failed(MultisigShellFailure.Unknown("not an account on this device"))
        val purpose = op.purpose
        if (purpose.kind == "approve") {
            val route = multisigApprovalRoute(accountJson)?.let { runCatching { Wire.json.parseToJsonElement(it) }.getOrNull() }
            when (route.kind ?: route.str) {
                "blocked" -> {
                    val block = route["block"]?.let { JSONObject(it.toString()) } ?: JSONObject()
                    return failed(MultisigShellFailure.VenueBlocked(block))
                }
                "page" -> {
                    val page = route["page"].str ?: route["url"].str
                        ?: return failed(MultisigShellFailure.PageSignFailed("not_opened"))
                    return approveOnPage(op, accountJson, page)
                }
            }
        }
        val challenge = unhex(op.challenge) ?: return failed(MultisigShellFailure.Unknown("the challenge does not read"))
        return try {
            // §16: a member's owner proof goes where the core routes
            // `vela_multisigCreate` — in Vela, or the account's page, handed the
            // core's params (the genesis), never the challenge; a page's answer
            // is the core's to accept, over exactly this challenge.
            val assertion = if (purpose.kind == "record") {
                spine.signOwnerProof(op.owner, op.chain_id.toInt(), challenge, (purpose["params"] ?: JsonNull).toString())
            } else {
                spine.assertInVela(op.owner, challenge)
            }
            MultisigShellResult.Asserted(
                credential_id = assertion.credentialIdHex,
                authenticator_data = hex0x(assertion.authenticatorDataHex),
                client_data_json = hex0x(assertion.clientDataJsonHex),
                signature = hex0x(assertion.signatureDerHex),
            )
        } catch (refused: UserOpSpine.Refused) {
            if (refused.failure == UserOpSpine.Failure.PasskeyCancelled) MultisigShellResult.PasskeyCancelled
            else failed(MultisigShellFailure.of(refused.failure))
        }
    }

    /** The member's approval on its trusted page: the core builds the request and accepts the answer. */
    private suspend fun approveOnPage(op: MultisigOperation.Passkey, accountJson: String, page: String): MultisigShellResult {
        val purpose = op.purpose
        val hash = purpose["safe_tx_hash"].str.orEmpty()
        val answerKey = "${op.owner.lowercase()}:${op.chain_id}:${hash.lowercase()}"
        pageAnswers[answerKey]?.let { kept ->
            return MultisigShellResult.PageApproved(Wire.json.parseToJsonElement(kept))
        }
        val channel = trustedSigner() ?: return failed(MultisigShellFailure.PageSignFailed("not_opened"))
        val keys = accountPort.keysOf(op.owner)
        val plan = app.getvela.wallet.feature.signing.trustedsigner.SigningPlan.of(accountJson)
        val (chainName, nativeSymbol) = chainLabel(op.chain_id.toInt())
        val name = runCatching { JSONObject(accountJson).optString("name").ifBlank { null } }.getOrNull()
        val request = runCatching {
            trustedSignerApprovalRequest(
                TrustedSignerApprovalInput(
                    chainId = op.chain_id.toULong(),
                    chainName = chainName,
                    nativeSymbol = nativeSymbol,
                    multisig = purpose["multisig"].str.orEmpty(),
                    multisigName = purpose["name"].str,
                    txJson = (purpose["tx"] ?: JsonObject(emptyMap())).toString(),
                    member = op.owner,
                    memberName = name,
                    credentialIdsHex = keys.map { it.credentialId },
                    keyRouteJson = plan?.keyRouteJson,
                    // r9-10: the review asked for "… anyway" — the page says it too.
                    anyway = purpose["anyway"].bool ?: false,
                    // Core §9.5: the members and the rule it is signed under, so the
                    // page says a member change before → after.
                    owners = purpose["owners"].arr.mapNotNull { it.str }.takeIf { it.isNotEmpty() },
                    threshold = purpose["threshold"].long?.toULong(),
                ),
            )
        }.getOrElse { return failed(MultisigShellFailure.PageSignFailed("not_opened")) }
        return when (val ending = channel.approve(request, op.chain_id, op.owner, hash, keys, page)) {
            is TrustedSignerChannel.ApprovalEnding.Accepted -> {
                pageAnswers[answerKey] = ending.approvalJson
                MultisigShellResult.PageApproved(Wire.json.parseToJsonElement(ending.approvalJson))
            }
            TrustedSignerChannel.ApprovalEnding.Declined -> MultisigShellResult.PasskeyCancelled
            is TrustedSignerChannel.ApprovalEnding.Failed -> failed(MultisigShellFailure.PageSignFailed(pageWire(ending.failure)))
        }
    }

    private fun failed(failure: MultisigShellFailure) = MultisigShellResult.PasskeyFailed(failure.detail)

    // -- risk -------------------------------------------------------------------

    override suspend fun askRisk(op: MultisigOperation.AskRisk): Boolean = onAskRisk?.invoke(op) ?: false

    // -- send -------------------------------------------------------------------

    /**
     * `from` sends `calls` as one user operation, by what it is for. An
     * execution: the multisig path's own — ONE signature, the executor's own
     * operation, its page handed what it runs; the coin checked before the
     * passkey. A record (Ethereum backup): the signing sheet.
     */
    override suspend fun sendCalls(op: MultisigOperation.SendCalls): MultisigShellResult {
        val purpose = op.purpose
        val isRecord = purpose?.let { it.kind == "record" } ?: (op.fee is JsonNull)
        if (isRecord) {
            return onFirstParty?.invoke(op)
                ?: MultisigShellResult.CallsFailed(MultisigShellFailure.Unknown("nothing to sign it on").detail, maybe_sent = false)
        }
        if (accountPort.accountJson(op.from) == null) {
            return MultisigShellResult.CallsFailed(MultisigShellFailure.Unknown("the executor is not on this device").detail, maybe_sent = false)
        }
        val calls = op.calls.map { call ->
            UserOpCall(to = call["to"].str.orEmpty(), value = decimal(call["value"]), data = call["data"].str?.ifEmpty { null } ?: "0x")
        }
        val fee = op.fee as? JsonObject
        val quoted = fee?.let { f ->
            val recipient = f["recipient"].str ?: return@let null
            val amount = f["amount"] ?: return@let null
            UserOpSpine.Quoted(
                amount = decimal(amount),
                recipient = recipient,
                tier = f["tier"].str?.let { t -> runCatching { Wire.json.decodeFromString(FeeTier.serializer(), "\"$t\"") }.getOrNull() },
            )
        }
        // The coin, before any prompt: a fee in the chain's own coin the
        // executor cannot pay is said, never signed.
        if (quoted != null && fee["token"].isNullish) {
            val value = calls.fold(BigInteger.ZERO) { sum, call -> sum + (call.value.toBigIntegerOrNull() ?: BigInteger.ZERO) }
            val have = (rpc(op.chain_id.toInt(), "eth_getBalance", JsonArray(listOf(JsonPrimitive(op.from), JsonPrimitive("latest")))) as? MultisigRpcReply.Result)
                ?.value.str?.let { runCatching { BigInteger(it.removePrefix("0x"), 16) }.getOrNull() }
            val need = value + (quoted.amount.toBigIntegerOrNull() ?: BigInteger.ZERO)
            if (have != null && have < need) {
                return MultisigShellResult.CallsFailed(
                    MultisigShellFailure.CannotPay(need = need.toString(), have = have.toString()).detail,
                    maybe_sent = false,
                )
            }
        }
        val intent = TrustedSignerIntent(
            method = "vela_multisigExecute",
            paramsJson = executeParams(purpose).toString(),
            origin = "",
        )
        // r17-12: whether the passkey was asked — a failure before it may be
        // the network's alone; after it, it is not.
        var signing = false
        return try {
            val submitted = spine.submit(
                chainId = op.chain_id.toInt(),
                account = op.from,
                calls = calls,
                gasFeeToken = fee?.get("token").str,
                quotedFee = quoted,
                signingStarted = { signing = true },
                intent = intent,
            )
            // The proposals machine follows the execution itself (its
            // `executions`, by this hash): nothing waits on a tracker.
            if (submitted.maybeSent) {
                MultisigShellResult.CallsFailed(submitted.userOpHash, maybe_sent = true)
            } else {
                MultisigShellResult.CallsSent(submitted.userOpHash, null)
            }
        } catch (refused: UserOpSpine.Refused) {
            when (refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled, UserOpSpine.Failure.PageDeclined -> MultisigShellResult.CallsCancelled
                else -> {
                    val chain = op.chain_id.toInt()
                    val failing = pool.view.value.let { v -> chain in v.failed_chains || chain in v.unreached_chains || chain in v.rate_limited_chains }
                    val said = MultisigShellFailure.ofSubmit(refused.failure, refused.chainRead, signing, failing, chain)
                    VelaLog.event("multisig.execute", "refused", "chain" to chain, "kind" to said::class.simpleName, "signing" to signing)
                    MultisigShellResult.CallsFailed(said.detail, maybe_sent = false)
                }
            }
        }
    }

    // -- quote ------------------------------------------------------------------

    /**
     * Price the executor's calls the way every other send is priced: one
     * fee-policy quote of the real operation, at the relay's price (the web's
     * `quoteExecutionFee`). Its own fee session, so a quote here never
     * supersedes the send screen's.
     */
    override suspend fun quoteCalls(op: MultisigOperation.QuoteCalls): MultisigShellResult {
        val calls = op.calls.map { call ->
            FeeCall(to = call["to"].str.orEmpty(), value = decimal(call["value"]), data = call["data"].str?.ifEmpty { null } ?: "0x")
        }
        val keys = accountPort.keysOf(op.from)
        val quoted = fees().quote(
            chainId = op.chain_id.toInt(),
            account = op.from,
            publicKeyAvailable = keys.isNotEmpty(),
            calls = calls,
            feeToken = null,
        )
        val view = (quoted as? SpeedControl.Quoted.Settled)?.view
        // r12-6: each look's quote, as it ended — what tells "the fee machine
        // priced it" from "the look said it could not".
        VelaLog.event(
            "multisig.quote", if (view == null) "timed out" else if (view.fee != null && view.failed == null) "priced" else "failed",
            "chain" to op.chain_id, "failed" to view?.failed?.let { it::class.simpleName }, "failure" to view?.failure?.failure?.wire,
            "retrying" to view?.failure?.retrying,
        )
        view ?: return MultisigShellResult.Quoted(jo("kind" to "failed"))
        val estimate = view.fee
        val recipient = estimate?.fee_recipient
        if (estimate != null && view.failed == null && recipient != null) {
            val amount = when (val asset = estimate.fee_asset) {
                is FeeAssetView.Erc20 -> asset.amount
                FeeAssetView.Native -> estimate.total_wei
            }
            if ((amount.toBigIntegerOrNull() ?: BigInteger.ZERO) > BigInteger.ZERO) {
                val feeObject = linkedMapOf<String, Any?>("amount" to amount, "recipient" to recipient)
                if (estimate.tier != FeeTier.Rapid) feeObject["tier"] = Wire.json.encodeToJsonElement(FeeTier.serializer(), estimate.tier)
                return MultisigShellResult.Quoted(
                    jo("kind" to "ok", "fee" to feeObject, "totalGas" to estimate.total_gas, "executorDeployed" to estimate.deployed),
                )
            }
        }
        if (view.failed == FeeFailure.WouldFail) {
            return MultisigShellResult.Quoted(jo("kind" to "reverts", "reason" to null))
        }
        return MultisigShellResult.Quoted(jo("kind" to "failed"))
    }

    // -- reads ------------------------------------------------------------------

    override suspend fun simulate(op: MultisigOperation.Simulate): String? {
        val legs = op.calls.map { SimDeltas.Call(it["to"].str.orEmpty(), decimal(it["value"]), it["data"].str ?: "0x") }
        val body = SimDeltas.body(op.from, legs) ?: return null
        // Real device S2 (core §9.5): the multisig as its deployment leaves it, as the core gave it.
        op.state_overrides?.takeUnless { it is JsonNull }?.let { overrides ->
            body.getJSONArray("blockStateCalls").getJSONObject(0).put("stateOverrides", JSONObject(overrides.toString()))
        }
        val answer = pool.call(op.chain_id.toInt(), "eth_simulateV1", listOf(body, "latest"))
        return SimDeltas.replyJson(answer)
    }

    override suspend fun recipientFacts(op: MultisigOperation.RecipientRisk): Pair<Boolean?, Boolean?> {
        val contract = runCatching { relay.isContract(op.chain_id.toInt(), op.address) }.getOrNull()
        // r10 S4: the person's own account or multisig is never "first time
        // sending here" (the web's `ownAddress`; the send flow's `own`).
        if (own(op.address)) return contract to false
        val sent = runCatching { sentTo(op.address) }.getOrNull()
        return contract to sent?.let { !it }
    }

    override suspend fun userOpStatus(op: MultisigOperation.UserOpStatus): Pair<String?, String?>? {
        val answer = relay.userOpStatus(op.chain_id.toInt(), op.user_op_hash) ?: return null
        val status = Wire.json.encodeToJsonElement(TrackLifecycle.serializer(), answer.status).str
        return status to answer.txHash
    }

    // -- wallet, notices --------------------------------------------------------

    /** Android has no injected wallet: the outside page runs in a browser. */
    override suspend fun wallet(method: String, params: JsonElement): JsonElement =
        throw MultisigWalletRefusal(4900, "no wallet connected")

    override suspend fun notice(notice: JsonObject) {
        runCatching { onNotice?.invoke(notice) }.onFailure { VelaLog.failure("multisig", "notice routing", it) }
    }

    companion object {
        /** What the executor's trusted page is shown beside the operation (r7-1, the web's `executeRequest`). */
        fun executeParams(purpose: JsonObject?): JsonObject {
            if (purpose?.kind != "execute") return JsonObject(emptyMap())
            return jo(
                "tx" to (purpose["tx"] ?: JsonNull),
                "multisig" to (purpose["multisig"] ?: JsonNull),
                "name" to (purpose["name"] ?: JsonNull),
                "threshold" to (purpose["threshold"] ?: JsonNull),
                // Core §9.5: the members it runs under (`SendPurpose::Execute.owners`).
                "owners" to (purpose["owners"] ?: JsonNull),
                // D1: the one review asks for "… anyway" — the page says "Execute anyway".
                "anyway" to (purpose["anyway"] ?: JsonPrimitive(false)),
            )
        }

        /** `0x…` or decimal → decimal text, as a call's value travels. */
        fun decimal(value: JsonElement?): String {
            val text = value.str ?: return "0"
            if (text.startsWith("0x") || text.startsWith("0X")) {
                return runCatching { BigInteger(text.substring(2).ifEmpty { "0" }, 16).toString() }.getOrDefault("0")
            }
            return text.ifEmpty { "0" }
        }

        fun unhex(hex: String?): ByteArray? {
            val text = hex?.removePrefix("0x")?.removePrefix("0X") ?: return null
            if (text.length % 2 != 0) return null
            return runCatching { ByteArray(text.length / 2) { i -> text.substring(i * 2, i * 2 + 2).toInt(16).toByte() } }.getOrNull()
        }

        fun hex0x(text: String): String = if (text.startsWith("0x")) text else "0x$text"

        /** JSON as the pool's params travel (org.json). */
        fun toOrg(element: JsonElement): Any? = when (element) {
            JsonNull -> JSONObject.NULL
            is JsonPrimitive -> when {
                element.isString -> element.content
                element.content == "true" -> true
                element.content == "false" -> false
                else -> element.content.toLongOrNull() ?: element.content.toDoubleOrNull() ?: element.content
            }
            is JsonObject -> JSONObject().apply { element.forEach { (k, v) -> put(k, toOrg(v)) } }
            is JsonArray -> JSONArray().apply { element.forEach { put(toOrg(it)) } }
        }
    }
}

/** A shell with no PeerConnect rooms: every open is refused (said, never pretended). */
object NoPeer : MultisigPeerTransport {
    override suspend fun open(relay: String?, invite: String?, pairing: String?): PeerOpened =
        throw IllegalStateException("no PeerConnect transport")

    override suspend fun send(room: String, message: JsonElement) = Unit
    override suspend fun receive(room: String): PeerArrival = PeerArrival.Closed("closed")
    override suspend fun close(room: String, cancel: Boolean, end: Boolean) = Unit
}


/** A page failure by the core's name for it (`unanswered`, `mismatch`, `refused`, `not_opened`). */
fun pageWire(failure: PageFailure): String = Wire.json.encodeToJsonElement(PageFailure.serializer(), failure).str ?: "refused"
