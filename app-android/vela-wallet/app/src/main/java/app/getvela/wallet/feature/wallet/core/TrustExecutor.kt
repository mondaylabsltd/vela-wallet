package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import java.math.BigInteger
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `token_trust` core touches the outside world.
 *
 * Seven operations, and **not one judgement about a token**. Whether a log is
 * a genuine Transfer to this wallet, whether its contract is trusted, whether
 * an unresolvable symbol may be displayed, and whether a token may be added to
 * somebody's list are all `token_trust.rs`. Every one of those is a door a
 * scam token walks through if a shell starts deciding, which is exactly why
 * this class hands over raw logs and nothing else.
 *
 * Port source: `app-web/vela-wallet/src/lib/wallet/core/token-trust-executor.ts`.
 *
 * **The range cap is a fact, not a failure.** An endpoint that refuses a wide
 * `eth_getLogs` span is not broken; the core narrows the window and asks again.
 * The pool already classifies this, so the distinction survives all the way in.
 */
class TrustExecutor(
    private val pool: RpcPool,
    private val store: KeyValueStore,
    /** Forget cached balances for an address, so an admitted token shows up now. */
    private val invalidate: (String) -> Unit = {},
) {

    suspend fun perform(operation: TrustOperation): TrustShellResult = when (operation) {

        is TrustOperation.RpcBlockNumber -> TrustShellResult.BlockNumber(
            address = operation.address,
            chain_id = operation.chain_id,
            block_hex = (pool.call(operation.chain_id, "eth_blockNumber") as? RpcResult.Body)
                ?.json?.optString("result")?.takeIf { it.startsWith("0x") },
        )

        is TrustOperation.RpcGetLogs -> TrustShellResult.Logs(
            address = operation.address,
            chain_id = operation.chain_id,
            outcome = getLogs(operation),
        )

        is TrustOperation.RpcGetBlockByNumber -> blockTimestamp(operation)

        is TrustOperation.MulticallErc20Meta -> TrustShellResult.ErcMeta(
            chain_id = operation.chain_id,
            entries = tokenMetadata(operation.chain_id, operation.addrs),
        )

        is TrustOperation.ReadCustomTokens -> TrustShellResult.CustomTokens(
            tokens = readCustomTokens(),
        )

        is TrustOperation.WriteCustomToken ->
            TrustShellResult.TokenWritten(ok = writeCustomToken(operation.token))

        is TrustOperation.InvalidateTokenCache -> {
            invalidate(operation.address)
            TrustShellResult.CacheInvalidated
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: TrustOperation): TrustShellResult = when (operation) {
        is TrustOperation.RpcBlockNumber ->
            TrustShellResult.BlockNumber(operation.address, operation.chain_id, null)
        is TrustOperation.RpcGetLogs ->
            TrustShellResult.Logs(operation.address, operation.chain_id, TrustLogsOutcome.Failed)
        is TrustOperation.RpcGetBlockByNumber -> TrustShellResult.BlockTimestamp(
            address = operation.address,
            chain_id = operation.chain_id,
            block_number = hexToDouble(operation.block),
            timestamp_sec = null,
            now_ms = System.currentTimeMillis().toDouble(),
        )
        // Every requested address is answered, resolved or not — a silent
        // omission would leave the core waiting on metadata forever.
        is TrustOperation.MulticallErc20Meta ->
            TrustShellResult.ErcMeta(operation.chain_id, operation.addrs.map { TrustMetaEntry(it) })
        // `null`, not an empty list: "the read failed" fails admission closed,
        // while "no custom tokens" would let an unknown token through.
        is TrustOperation.ReadCustomTokens -> TrustShellResult.CustomTokens(null)
        is TrustOperation.WriteCustomToken -> TrustShellResult.TokenWritten(ok = false)
        is TrustOperation.InvalidateTokenCache -> TrustShellResult.CacheInvalidated
    }

    // -- chain reads ---------------------------------------------------------

    private suspend fun getLogs(operation: TrustOperation.RpcGetLogs): TrustLogsOutcome {
        val filter = JSONObject()
            .put("fromBlock", operation.from_block)
            .put("toBlock", operation.to_block)
            // topic0 = Transfer(address,address,uint256); topic1 = anyone;
            // topic2 = this wallet. The core built the topic — including which
            // slot the recipient sits in — because getting that wrong reads
            // SENT money as received.
            .put(
                "topics",
                JSONArray()
                    .put(TRANSFER_TOPIC)
                    .put(JSONObject.NULL)
                    .put(operation.recipient_topic),
            )
        // The allowlist. An empty list is not "every contract" — it is none,
        // and the core means it: an unrestricted log query is how a wallet
        // ends up announcing airdropped scam tokens as receipts.
        if (operation.contracts.isNotEmpty()) {
            filter.put("address", JSONArray(operation.contracts))
        }

        return when (val answer = pool.call(operation.chain_id, "eth_getLogs", listOf(filter))) {
            is RpcResult.Body -> {
                val logs = answer.json.optJSONArray("result")
                    ?: return TrustLogsOutcome.Failed
                TrustLogsOutcome.Ok(
                    logs = (0 until logs.length()).mapNotNull { index ->
                        logs.optJSONObject(index)?.let(::rawLog)
                    },
                )
            }
            // Not a failure: the endpoint will answer a narrower window, and
            // the core is the thing that decides how much narrower.
            is RpcResult.RangeCapped ->
                TrustLogsOutcome.RangeCapped(cap = answer.maxSpan.toInt().coerceAtLeast(0))
            is RpcResult.Failed -> TrustLogsOutcome.Failed
        }
    }

    private fun rawLog(row: JSONObject): TrustRawLog {
        val topics = row.optJSONArray("topics")
        return TrustRawLog(
            address = row.optString("address"),
            topics = topics?.let { array ->
                (0 until array.length()).map { array.optString(it) }
            } ?: emptyList(),
            data = row.optString("data"),
            transaction_hash = row.optString("transactionHash"),
            block_number = row.optString("blockNumber").ifBlank { null },
            log_index = row.optString("logIndex").ifBlank { null },
        )
    }

    private suspend fun blockTimestamp(
        operation: TrustOperation.RpcGetBlockByNumber,
    ): TrustShellResult.BlockTimestamp {
        val answer = pool.call(
            operation.chain_id,
            "eth_getBlockByNumber",
            // `false`: headers only. Asking for full transactions would pull
            // an entire block down a phone connection to read one number.
            listOf(operation.block, false),
        )
        val header = (answer as? RpcResult.Body)?.json?.optJSONObject("result")
        return TrustShellResult.BlockTimestamp(
            address = operation.address,
            chain_id = operation.chain_id,
            block_number = hexToDouble(operation.block),
            timestamp_sec = header?.optString("timestamp")
                ?.takeIf { it.startsWith("0x") }
                ?.let { hexToDouble(it) },
            now_ms = System.currentTimeMillis().toDouble(),
        )
    }

    /**
     * `symbol()` + `decimals()` for several tokens in one batch.
     *
     * **Every requested address is answered**, resolved or not. A token whose
     * metadata could not be read comes back with `meta = null`, which the core
     * treats as a fact — not as an invitation to invent a symbol.
     */
    private suspend fun tokenMetadata(chainId: Int, addrs: List<String>): List<TrustMetaEntry> {
        if (addrs.isEmpty()) return emptyList()

        val calls = addrs.flatMap { addr ->
            listOf(Abi.Call(addr, Abi.encodeSymbol()), Abi.Call(addr, Abi.encodeDecimals()))
        }
        val request = JSONObject()
            .put("to", Abi.MULTICALL3)
            .put("data", Abi.encodeAggregate3(calls))
        val answer = pool.call(chainId, "eth_call", listOf(request, "latest"))
        val hex = (answer as? RpcResult.Body)?.json?.optString("result")
            ?.takeIf { it.startsWith("0x") }
            ?: return addrs.map { TrustMetaEntry(it) }
        val results = Abi.decodeAggregate3(hex)

        return addrs.mapIndexed { index, addr ->
            val symbolResult = results.getOrNull(index * 2)?.takeIf { it.success }
            val decimalsResult = results.getOrNull(index * 2 + 1)?.takeIf { it.success }
            val symbol = symbolResult?.data?.let(Abi::decodeString)
            val decimals = decimalsResult?.data?.let(Abi::decodeUint8)
            TrustMetaEntry(
                addr = addr,
                // BOTH or neither. A token with decimals but no name cannot be
                // shown as an amount of anything, and a name with no decimals
                // is an amount off by an unknown power of ten.
                meta = if (symbol != null && decimals != null) {
                    TrustTokenMeta(symbol, decimals)
                } else {
                    null
                },
            )
        }
    }

    // -- the custom-token store ----------------------------------------------

    private suspend fun readCustomTokens(): List<TrustCustomToken>? {
        val raw = store.read(KeyValueStore.Keys.CUSTOM_TOKENS) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return null
        return (0 until array.length()).mapNotNull { index ->
            val row = array.optJSONObject(index) ?: return@mapNotNull null
            val id = row.optString("id").ifBlank { return@mapNotNull null }
            TrustCustomToken(
                id = id,
                chain_id = row.optInt("chainId"),
                contract_address = row.optString("contractAddress"),
                symbol = row.optString("symbol"),
                name = row.optString("name"),
                decimals = row.optInt("decimals", 18),
            )
        }
    }

    /** Replaces by id, never duplicates — the core's invariant, kept by the write. */
    private suspend fun writeCustomToken(token: TrustCustomToken): Boolean = runCatching {
        val raw = store.read(KeyValueStore.Keys.CUSTOM_TOKENS)
        val existing = raw?.let { runCatching { JSONArray(it) }.getOrNull() } ?: JSONArray()
        val kept = JSONArray()
        for (index in 0 until existing.length()) {
            val row = existing.optJSONObject(index) ?: continue
            if (row.optString("id") != token.id) kept.put(row)
        }
        kept.put(
            JSONObject()
                .put("id", token.id)
                .put("chainId", token.chain_id)
                .put("contractAddress", token.contract_address)
                .put("symbol", token.symbol)
                .put("name", token.name)
                .put("decimals", token.decimals),
        )
        store.write(KeyValueStore.Keys.CUSTOM_TOKENS, kept.toString())
        true
    }.getOrDefault(false)

    private fun hexToDouble(hex: String): Double =
        runCatching { BigInteger(hex.removePrefix("0x").ifEmpty { "0" }, 16).toDouble() }
            .getOrDefault(0.0)

    private companion object {
        /** `keccak256("Transfer(address,address,uint256)")`. */
        const val TRANSFER_TOPIC =
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    }
}
