package app.getvela.wallet.bench

import app.getvela.wallet.core.data.KeyValueStore
import org.json.JSONArray
import org.json.JSONObject

/**
 * The returning person the Phase 4 journeys launch as: the parallel space's
 * golden 1-of-3 Safe (docs/PARALLEL-SPACE.md) — a real, deployed Safe on
 * Gnosis, so Send can quote a real operation and reach its Confirm — held
 * VIEW-ONLY: the record carries the three fixture keys' PUBLIC halves only;
 * a benchmark build has no signer for them (the keyset is the debug-only
 * libvela_dev_fixtures.so), so nothing here can ever sign. Plus a day-fresh
 * holdings cache, so the home opens on last-known rows (P2.4 "cached home"),
 * and [DEFAULT_ROWS] activity records — the store's own cap — so the history
 * list is as long as it ever gets.
 *
 * Deterministic apart from the clock: the same rows, amounts and counterparties
 * every run, so two runs of a journey draw the same screens.
 */
object BenchFixture {
    const val DEFAULT_ROWS = 200

    /** The golden multi-key Safe (identical on every EVM chain; deployed and funded with a little XDAI on Gnosis). */
    const val ADDRESS = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /** The fixture keys' credential ids (`vela-fixture-0N`, hex) and P-256 public keys — public data. */
    private val keys = listOf(
        "76656c612d666978747572652d3031" to "04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b",
        "76656c612d666978747572652d3032" to "047802f2cc39cc6ed85c41268a580c5f0df36df3f065facfb40f84265927b7ed678438b2084cb84f0d00708e40d44ed8c9901d979bbcb390117089847672c12eec",
        "76656c612d666978747572652d3033" to "043eb2ae2f4e8090837820048baca2db04a7e7ca7dc6742f342a30c6855e7d96947f36ac5a4538dc3a8a7cbf0deea1c0ecb80bfb96a3f8ca57c98f8f400548cd56",
    )

    private class Coin(val chain: Int, val symbol: String, val name: String, val balance: String, val decimals: Int, val token: String?, val price: Double)

    /** What the home last showed: the Safe's one real holding (the live read replaces it). */
    private val held = listOf(Coin(100, "XDAI", "xDai", "0.05", 18, null, 1.0))

    /** The networks and coins the history's rows move. */
    private val coins = listOf(
        Coin(1, "ETH", "Ether", "32.5", 18, null, 3000.0),
        Coin(1, "USDC", "USD Coin", "1200.5", 6, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 1.0),
        Coin(1, "USDT", "Tether USD", "830.25", 6, "0xdAC17F958D2ee523a2206206994597C13D831ec7", 1.0),
        Coin(100, "XDAI", "xDai", "15.2", 18, null, 1.0),
        Coin(8453, "ETH", "Ether", "0.42", 18, null, 3000.0),
        Coin(42161, "ARB", "Arbitrum", "100", 18, "0x912CE59144191C1204E64559FE8253a0e49E6548", 0.8),
        Coin(10, "OP", "Optimism", "50", 18, "0x4200000000000000000000000000000000000042", 1.7),
    )

    /** Writes the fixture; `false` when the store refused a key. */
    suspend fun write(store: KeyValueStore, rows: Int, nowMs: Long): Boolean {
        val lower = ADDRESS.lowercase()
        val records = JSONArray()
        keys.forEachIndexed { index, (credential, publicKey) ->
            records.put(
                JSONObject()
                    .put("credential_id", credential)
                    .put("public_key_hex", publicKey)
                    .put("name", "Bench key ${index + 1}")
                    .put("transports", "internal"),
            )
        }
        val account = JSONObject()
            .put("id", keys.first().first)
            .put("name", "Bench")
            .put("address", ADDRESS)
            .put("public_key_hex", keys.first().second)
            .put("created_at_iso", "2026-10-01T00:00:00.000Z")
            .put("keys", records)
        val tokens = JSONArray()
        var total = 0.0
        for (coin in held) {
            total += coin.balance.toDouble() * coin.price
            tokens.put(
                JSONObject()
                    .put("chain_id", coin.chain)
                    .put("symbol", coin.symbol)
                    .put("name", coin.name)
                    .put("balance", coin.balance)
                    .put("decimals", coin.decimals)
                    .put("token_address", coin.token ?: JSONObject.NULL)
                    .put("price_usd", coin.price)
                    .put("spam", false),
            )
        }
        val holdings = JSONObject().put("$lower#mainnet", JSONObject().put("at", nowMs).put("tokens", tokens))
        val balance = JSONObject().put(lower, JSONObject().put("usd", total).put("at", nowMs))
        return listOf(
            "vela.accounts" to JSONArray().put(account).toString(),
            "vela.activeAccountIndex" to "0",
            "vela.holdingsCache" to holdings.toString(),
            "vela.balanceCache" to balance.toString(),
            "vela.transactionHistory" to history(rows.coerceIn(0, DEFAULT_ROWS), nowMs / 1000).toString(),
        ).all { (key, value) -> store.write(key, value) }
    }

    /** Newest first, every few hours back over a couple of months, sends and receives across the networks above. */
    private fun history(rows: Int, nowS: Long): JSONArray {
        val out = JSONArray()
        for (i in 0 until rows) {
            val coin = coins[i % coins.size]
            val receive = i % 3 == 1
            val other = "0x" + String.format(java.util.Locale.ROOT, "%040x", 0xC0FFEE00L + i * 7919L)
            val amount = when (coin.decimals) {
                6 -> String.format(java.util.Locale.ROOT, "%d.%02d", 5 + (i * 37) % 900, (i * 13) % 100)
                else -> String.format(java.util.Locale.ROOT, "0.%03d", 1 + (i * 17) % 999)
            }
            out.put(
                JSONObject()
                    .put("id", "bench-$i")
                    .put("userOpHash", if (receive) "" else "0x" + String.format(java.util.Locale.ROOT, "%064x", 0xAB000000L + i))
                    .put("txHash", "0x" + String.format(java.util.Locale.ROOT, "%064x", 0xCD000000L + i))
                    .put("from", if (receive) other else ADDRESS)
                    .put("to", if (receive) ADDRESS else other)
                    .put("value", amount)
                    .put("symbol", coin.symbol)
                    .put("decimals", coin.decimals)
                    .put("chainId", coin.chain)
                    .put("timestamp", nowS - 600 - i * 7L * 3600)
                    .put("status", "confirmed")
                    .put("type", if (receive) "receive" else "send")
                    .put("usd", String.format(java.util.Locale.ROOT, "%.2f", amount.toDouble() * coin.price)),
            )
        }
        return out
    }
}
