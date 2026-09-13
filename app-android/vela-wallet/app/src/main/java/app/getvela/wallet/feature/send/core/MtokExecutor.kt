package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.wallet.core.Abi
import org.json.JSONArray
import org.json.JSONObject

/**
 * The `manage_tokens` machine's five arms (spec 043 T045) — the web's
 * `manage-tokens-executor.ts`, the desktop's `executor/manage_tokens.rs`.
 *
 * One Multicall3 `aggregate3` per chain reads `name`/`symbol`/`decimals`;
 * the custom-token list is the SAME `vela.customTokens` document the balance
 * walk and `token_trust` read and write, in the same camelCase rows, so a
 * token added here is priced on the next refresh and a token admitted by a
 * receipt shows up in this sheet as already added.
 */
class MtokExecutor(
    private val store: KeyValueStore,
    /** `eth_call` on one chain: the `result` hex, or `null` for "no answer". */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    /** The token cache is gone: refresh the balances so the new row is priced. */
    private val onInvalidated: () -> Unit = {},
    private val haptic: () -> Unit = {},
) {
    suspend fun perform(operation: MtokOperation): MtokShellResult = when (operation) {
        is MtokOperation.MulticallErc20Meta -> MtokShellResult.ChainMetaResolved(
            chain_id = operation.chain_id,
            address = operation.address,
            meta = meta(operation.chain_id, operation.address),
        )
        MtokOperation.ReadCustomTokens -> MtokShellResult.CustomTokensLoaded(readAll())
        is MtokOperation.WriteCustomToken ->
            if (write(operation.token)) { haptic(); MtokShellResult.Saved } else MtokShellResult.SaveFailed
        is MtokOperation.RemoveCustomToken ->
            if (remove(operation.id)) { haptic(); MtokShellResult.Removed(operation.id) } else MtokShellResult.RemoveFailed(operation.id)
        MtokOperation.InvalidateTokenCache -> {
            onInvalidated()
            MtokShellResult.CacheInvalidated
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: MtokOperation): MtokShellResult = when (operation) {
        is MtokOperation.MulticallErc20Meta -> MtokShellResult.ChainMetaResolved(operation.chain_id, operation.address, null)
        MtokOperation.ReadCustomTokens -> MtokShellResult.CustomTokensLoaded()
        is MtokOperation.WriteCustomToken -> MtokShellResult.SaveFailed
        is MtokOperation.RemoveCustomToken -> MtokShellResult.RemoveFailed(operation.id)
        MtokOperation.InvalidateTokenCache -> MtokShellResult.CacheInvalidated
    }

    /** `name`/`symbol`/`decimals` in one round trip; all three or nothing (the web's rule). */
    private suspend fun meta(chainId: Int, address: String): MtokTokenMeta? {
        val data = Abi.encodeAggregate3(
            listOf(
                Abi.Call(address, Abi.encodeName()),
                Abi.Call(address, Abi.encodeSymbol()),
                Abi.Call(address, Abi.encodeDecimals()),
            ),
        )
        val answer = ethCall(chainId, Abi.MULTICALL3, data) ?: return null
        val results = runCatching { Abi.decodeAggregate3(answer) }.getOrNull() ?: return null
        if (results.size < 3 || results.take(3).any { !it.success }) return null
        val name = Abi.decodeString(results[0].data)?.trim()?.ifEmpty { null } ?: return null
        val symbol = Abi.decodeString(results[1].data)?.trim()?.ifEmpty { null } ?: return null
        val decimals = runCatching { Abi.decodeUint8(results[2].data) }.getOrNull()?.takeIf { it in 0..255 } ?: return null
        return MtokTokenMeta(name = name, symbol = symbol, decimals = decimals)
    }

    private suspend fun rows(): JSONArray =
        store.read(KeyValueStore.Keys.CUSTOM_TOKENS)?.let { runCatching { JSONArray(it) }.getOrNull() } ?: JSONArray()

    private suspend fun readAll(): List<MtokCustomToken> {
        val array = rows()
        return (0 until array.length()).mapNotNull { index ->
            val row = array.optJSONObject(index) ?: return@mapNotNull null
            val chainId = row.optLong("chainId", -1L).takeIf { it in 0..4_294_967_295L } ?: return@mapNotNull null
            MtokCustomToken(
                id = row.optString("id").ifBlank { return@mapNotNull null },
                chain_id = chainId.toInt(),
                contract_address = row.optString("contractAddress").ifBlank { return@mapNotNull null },
                symbol = row.optString("symbol"),
                name = row.optString("name"),
                decimals = row.optInt("decimals", 18).coerceIn(0, 255),
                network_name = row.optString("networkName"),
            )
        }
    }

    /** Replaces by id, never duplicates — the core's invariant, kept by the write. */
    private suspend fun write(token: MtokCustomToken): Boolean = runCatching {
        val kept = JSONArray()
        val existing = rows()
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
                .put("decimals", token.decimals)
                .put("networkName", token.network_name),
        )
        store.write(KeyValueStore.Keys.CUSTOM_TOKENS, kept.toString())
        true
    }.getOrDefault(false)

    private suspend fun remove(id: String): Boolean = runCatching {
        val kept = JSONArray()
        val existing = rows()
        for (index in 0 until existing.length()) {
            val row = existing.optJSONObject(index) ?: continue
            if (row.optString("id") != id) kept.put(row)
        }
        store.write(KeyValueStore.Keys.CUSTOM_TOKENS, kept.toString())
        true
    }.getOrDefault(false)
}
