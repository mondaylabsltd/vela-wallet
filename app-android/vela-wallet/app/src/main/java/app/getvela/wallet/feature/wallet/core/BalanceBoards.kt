package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * The home's balance after a read that failed inside the app (PR 2 note 11),
 * as the REAL `balance_dashboard` machine says it — for the wallet gallery's
 * board, never a live surface. The account opens, its round settles with
 * Gnosis answered and Ethereum's read never having left the app (the
 * executor's `internal_chain_ids`), and the core's view is drawn: the
 * fault's own sentence (`internal_key`) where the unreachable line goes, and
 * no "Can't reach Ethereum".
 */
object BalanceBoards {
    fun internalFault(address: String, nowMs: Double): BalanceView {
        val settled = BalanceShellResult.FetchSettled(
            address = address,
            pull = false,
            tokens = listOf(
                BalanceToken(chain_id = 100, symbol = "xDAI", name = "xDAI", balance = "418.25", decimals = 18, price_usd = 1.0),
                BalanceToken(chain_id = 100, symbol = "USDC", name = "USDC", balance = "376.54321", decimals = 6, token_address = "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0", price_usd = 1.0),
            ),
            failed_chain_ids = listOf(1),
            internal_chain_ids = listOf(1),
            read_chain_ids = listOf(1, 100),
            now_ms = nowMs,
        )
        val script = CoreScript(BalanceDashboardCore().asBridge()) { operation ->
            val result: BalanceShellResult? = when (operation.optString("type")) {
                "fetch_tokens" -> settled
                "read_balance_cache" -> BalanceShellResult.CachedTotalLoaded(address, null)
                "read_balance_cache_many" -> BalanceShellResult.CachedBalancesLoaded(emptyList())
                "write_balance_cache" -> BalanceShellResult.BalanceCacheWritten
                "fetch_account_assets" -> BalanceShellResult.AccountAssetsFetched(address, null)
                "write_privacy" -> BalanceShellResult.PrivacyWritten
                // A retry timer stays out: the board is the round on screen.
                else -> null
            }
            result?.let { Wire.json.encodeToString(BalanceShellResult.serializer(), it) }
        }
        script.dispatch(Wire.json.encodeToString(BalanceEvent.serializer(), BalanceEvent.PrivacyHydrated(false)))
        script.dispatch(Wire.json.encodeToString(BalanceEvent.serializer(), BalanceEvent.AccountChanged(address)))
        return Wire.json.decodeFromString(BalanceView.serializer(), script.viewJson())
    }
}
