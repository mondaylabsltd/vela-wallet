package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.feature.wallet.core.Abi

/**
 * The `approval_guard` machine's three reads (spec 044 T031): token
 * metadata in one Multicall3 round trip, an allowance, a balance. Nothing
 * here judges an approval — the core detects every approval-granting shape
 * off the raw calldata and decides what may leave.
 */
class GuardExecutor(
    /** `eth_call` through the pool: the `result` hex or `null`. */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
) {
    suspend fun perform(operation: GuardOperation): GuardShellResult = when (operation) {
        is GuardOperation.ReadTokenMetadata -> GuardShellResult.MetaResolved(metas(operation.chain_id, operation.tokens))
        is GuardOperation.ReadErc20Allowance -> GuardShellResult.AllowanceRead(
            ethCall(operation.chain_id, operation.token, Abi.encodeAllowance(operation.owner, operation.spender))
                ?.let { runCatching { Abi.decodeUint256(it).toString() }.getOrNull() },
        )
        is GuardOperation.ReadErc20Balance -> GuardShellResult.BalanceRead(
            ethCall(operation.chain_id, operation.token, Abi.encodeBalanceOf(operation.owner))
                ?.let { runCatching { Abi.decodeUint256(it).toString() }.getOrNull() },
        )
    }

    fun neutralAnswer(operation: GuardOperation): GuardShellResult = when (operation) {
        is GuardOperation.ReadTokenMetadata -> GuardShellResult.MetaResolved(null)
        is GuardOperation.ReadErc20Allowance -> GuardShellResult.AllowanceRead(null)
        is GuardOperation.ReadErc20Balance -> GuardShellResult.BalanceRead(null)
    }

    /** No tokens answers an empty list rather than a failure. */
    private suspend fun metas(chainId: Int, tokens: List<String>): List<GuardTokenMetaEntry>? {
        if (tokens.isEmpty()) return emptyList()
        val calls = tokens.flatMap { token -> listOf(Abi.Call(token, Abi.encodeSymbol()), Abi.Call(token, Abi.encodeDecimals())) }
        val answer = ethCall(chainId, Abi.MULTICALL3, Abi.encodeAggregate3(calls)) ?: return null
        val results = runCatching { Abi.decodeAggregate3(answer) }.getOrNull() ?: return null
        if (results.size < calls.size) return null
        return tokens.mapIndexedNotNull { index, token ->
            val symbol = results[index * 2].takeIf { it.success }?.let { Abi.decodeString(it.data) } ?: return@mapIndexedNotNull null
            val decimals = results[index * 2 + 1].takeIf { it.success }?.let { runCatching { Abi.decodeUint8(it.data) }.getOrNull() } ?: return@mapIndexedNotNull null
            GuardTokenMetaEntry(token = token, symbol = symbol, decimals = decimals)
        }
    }
}
