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
    /**
     * [everyChain]: no chain answered — every read failed inside the app (a
     * faulted pool) and nothing is cached: the core says `unreachable`, so
     * the home draws no "$0.00" and no "Deposit your first asset", only the
     * fault's sentence over a skeleton.
     */
    fun internalFault(address: String, nowMs: Double, everyChain: Boolean = false): BalanceView {
        val failed = if (everyChain) listOf(1, 100) else listOf(1)
        val settled = BalanceShellResult.FetchSettled(
            address = address,
            pull = false,
            tokens = if (everyChain) {
                emptyList()
            } else {
                listOf(
                    BalanceToken(chain_id = 100, symbol = "xDAI", name = "xDAI", balance = "418.25", decimals = 18, price_usd = 1.0),
                    BalanceToken(chain_id = 100, symbol = "USDC", name = "USDC", balance = "376.54321", decimals = 6, token_address = "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0", price_usd = 1.0),
                )
            },
            failed_chain_ids = failed,
            internal_chain_ids = failed,
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

    /** Where the FIRST read of an empty wallet stands ([firstRead]). */
    enum class FirstRead {
        /** Still out: only last session's cached zero is known. */
        Out,

        /** It settled and every network answered: the wallet holds nothing. */
        Answered,

        /** It settled and some networks did not answer. */
        Missing,
    }

    /** The networks [firstRead] asks. */
    val FIRST_READ_CHAINS = mapOf(1 to "Ethereum", 100 to "Gnosis", 137 to "Polygon", 42161 to "Arbitrum")

    /**
     * An EMPTY wallet around its first read (PR 3 final note F19), as the
     * real machine says it. The account opens with last session's total — a
     * cached zero — and its first read is out ([FirstRead.Out]: the core's
     * `checking_key`, and no `live_key` — nothing has been read), then
     * settles with every network answering ([FirstRead.Answered]:
     * `live_key`) or with three that did not ([FirstRead.Missing]: the
     * unreachable line, never "live").
     */
    fun firstRead(address: String, nowMs: Double, stage: FirstRead): BalanceView {
        val asked = FIRST_READ_CHAINS.keys.toList()
        val script = CoreScript(BalanceDashboardCore().asBridge()) { operation ->
            val result: BalanceShellResult? = when (operation.optString("type")) {
                "fetch_tokens" -> when (stage) {
                    // The read is still out: nothing answers it.
                    FirstRead.Out -> null
                    FirstRead.Answered -> BalanceShellResult.FetchSettled(
                        address = address, pull = false, read_chain_ids = asked, now_ms = nowMs,
                    )
                    FirstRead.Missing -> BalanceShellResult.FetchSettled(
                        address = address, pull = false, failed_chain_ids = asked.drop(1), read_chain_ids = asked, now_ms = nowMs,
                    )
                }
                "read_balance_cache" -> BalanceShellResult.CachedTotalLoaded(address, 0.0)
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

    /** Tempo: a chain with no coin of its own — its money is all in its token list's stablecoins. */
    const val TEMPO = 4217

    /**
     * The home's balance when a chain's TOKEN LIST could not be loaded (the
     * integration's note 4), as the real machine says it. Two rounds: the
     * first reads everything, Tempo's stablecoin included; in the second
     * Tempo's registry document does not load, and — having no native coin
     * to read without it — the chain is not read. The executor reports it
     * failed AND in `registry_chain_ids`, so the core says "Can't load
     * Tempo's token list" (never "Can't reach Tempo": its RPC was not even
     * asked) and its row offers no RPC fix.
     */
    fun tokenListUnreachable(address: String, nowMs: Double): BalanceView {
        val gnosis = listOf(
            BalanceToken(chain_id = 100, symbol = "xDAI", name = "xDAI", balance = "418.25", decimals = 18, price_usd = 1.0),
            BalanceToken(chain_id = 100, symbol = "USDC", name = "USDC", balance = "376.54321", decimals = 6, token_address = "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0", price_usd = 1.0),
        )
        val tempo = BalanceToken(
            chain_id = TEMPO, symbol = "pathUSD", name = "pathUSD", balance = "120.5", decimals = 6,
            token_address = "0x20c0000000000000000000000000000000000000", price_usd = 1.0,
        )
        var round = 0
        val script = CoreScript(BalanceDashboardCore().asBridge()) { operation ->
            val result: BalanceShellResult? = when (operation.optString("type")) {
                "fetch_tokens" -> {
                    round += 1
                    if (round == 1) {
                        BalanceShellResult.FetchSettled(
                            address = address, pull = false, tokens = gnosis + tempo,
                            read_chain_ids = listOf(100, TEMPO), now_ms = nowMs - 60_000.0,
                        )
                    } else {
                        BalanceShellResult.FetchSettled(
                            address = address, pull = false, tokens = gnosis,
                            failed_chain_ids = listOf(TEMPO), registry_chain_ids = listOf(TEMPO),
                            read_chain_ids = listOf(100, TEMPO), now_ms = nowMs,
                        )
                    }
                }
                "read_balance_cache" -> BalanceShellResult.CachedTotalLoaded(address, null)
                "read_balance_cache_many" -> BalanceShellResult.CachedBalancesLoaded(emptyList())
                "write_balance_cache" -> BalanceShellResult.BalanceCacheWritten
                "fetch_account_assets" -> BalanceShellResult.AccountAssetsFetched(address, null)
                "write_privacy" -> BalanceShellResult.PrivacyWritten
                else -> null
            }
            result?.let { Wire.json.encodeToString(BalanceShellResult.serializer(), it) }
        }
        fun send(event: BalanceEvent) = script.dispatch(Wire.json.encodeToString(BalanceEvent.serializer(), event))
        send(BalanceEvent.PrivacyHydrated(false))
        send(BalanceEvent.AccountChanged(address))
        send(BalanceEvent.RefreshRequested(force = true, pull = false))
        return Wire.json.decodeFromString(BalanceView.serializer(), script.viewJson())
    }
}
