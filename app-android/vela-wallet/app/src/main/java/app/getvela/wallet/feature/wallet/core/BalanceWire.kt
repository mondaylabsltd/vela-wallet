package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `balance_dashboard` machine's wire types, in Kotlin.
 *
 * A transcription of `rust/crates/vela-core/src/app/balance_dashboard.rs`. What
 * a total means, when a figure is stale, what "partial" is, which notice to
 * show, whether a coin can be priced at all — all of it stays there.
 *
 * Numerics from the Rust (research D5): `chain_id`, `decimals`, `ms` and
 * `timer_id` are `u32` → `Int`; every `*_ms`, `usd` and `price_usd` is `f64` →
 * `Double`.
 */

// -- what the screen renders -------------------------------------------------

/**
 * One holding.
 *
 * **`balance` is a HUMAN DECIMAL, not raw units.** The core parses it straight
 * into a float and multiplies it by the price
 * (`balance_dashboard.rs:159` — `token_balance_double(&token.balance) *
 * token.price_usd`). Handing over `"1500000000000000000"` where `"1.5"` belongs
 * produces a total 10^18 times too large — and **nothing shows it until a price
 * exists**, which is exactly what spec 041 makes happen. The desktop sibling
 * found this dormant in its own read path (spec 031); it is inherited here as a
 * warning, and `BalanceLiveTest` pins it.
 */
@Serializable
data class BalanceToken(
    val chain_id: Int,
    val symbol: String,
    val name: String,
    /** Human decimal. See the note above before touching this. */
    val balance: String,
    val decimals: Int,
    /** `null` = this chain's native coin. */
    val token_address: String? = null,
    /** `null` = nothing could price it. **Not zero.** */
    val price_usd: Double? = null,
    val spam: Boolean = false,
)

@Serializable
enum class BalanceNotice {
    @SerialName("still_updating") StillUpdating,

    @SerialName("unpriced") Unpriced,
}

@Serializable
data class BalanceCacheEntry(val address: String, val usd: Double)

@Serializable
data class BalanceSwitcherView(
    val open: Boolean = false,
    val loading: Boolean = false,
    val balances: List<BalanceCacheEntry> = emptyList(),
)

/**
 * `BalanceView`.
 *
 * The distinctions here are the ones a money screen must not blur:
 * `balance_unknown` (we do not know) is not zero; `balance_partial` (some
 * chains answered) is not complete; `rate_limited_chain_ids` (busy) is not
 * `failed_chain_ids` (broken).
 */
@Serializable
data class BalanceView(
    val address: String? = null,
    /** `null` = unknown. A screen that renders this as `0` is lying. */
    val display_total_usd: Double? = null,
    val balance_unknown: Boolean = false,
    val balance_partial: Boolean = false,
    /**
     * Nothing could be read and nothing is known: the first fetch failed with
     * no cache to fall back on (#188, spec 038 finding 15). The core's
     * `display_total_usd` is `0.0` in this state — which is exactly the number
     * this flag exists to keep off the hero. A skeleton and a reason, never a
     * zero.
     */
    val unreachable: Boolean = false,
    val notice: BalanceNotice? = null,
    val hidden: Boolean = false,
    val refreshing: Boolean = false,
    val last_refreshed_at_ms: Double? = null,
    val tokens: List<BalanceToken> = emptyList(),
    /** Held, but with no price — shown, never counted. */
    val unpriced_tokens: List<BalanceToken> = emptyList(),
    val failed_chain_ids: List<Int> = emptyList(),
    val rate_limited_chain_ids: List<Int> = emptyList(),
    val banner_chain_ids: List<Int> = emptyList(),
    val holdings_loading: Boolean = false,
    val cached_total_usd: Double? = null,
    val switcher: BalanceSwitcherView = BalanceSwitcherView(),
)

// -- what the screen sends ---------------------------------------------------

@Serializable
sealed class BalanceEvent {
    @Serializable
    @SerialName("account_changed")
    data class AccountChanged(val address: String) : BalanceEvent()

    @Serializable
    @SerialName("refresh_requested")
    data class RefreshRequested(val force: Boolean, val pull: Boolean) : BalanceEvent()

    /** One chain's holdings arrived early — the streaming half of a fetch. */
    @Serializable
    @SerialName("chain_assets_arrived")
    data class ChainAssetsArrived(
        val address: String,
        val tokens: List<BalanceToken> = emptyList(),
    ) : BalanceEvent()

    @Serializable
    @SerialName("app_focused")
    data object AppFocused : BalanceEvent()

    @Serializable
    @SerialName("app_backgrounded")
    data object AppBackgrounded : BalanceEvent()

    @Serializable
    @SerialName("privacy_toggled")
    data object PrivacyToggled : BalanceEvent()

    @Serializable
    @SerialName("privacy_hydrated")
    data class PrivacyHydrated(val hidden: Boolean) : BalanceEvent()

    @Serializable
    @SerialName("fix_chain_resolved")
    data class FixChainResolved(val chain_id: Int) : BalanceEvent()

    @Serializable
    @SerialName("switcher_opened")
    data class SwitcherOpened(val addresses: List<String> = emptyList()) : BalanceEvent()

    @Serializable
    @SerialName("switcher_closed")
    data object SwitcherClosed : BalanceEvent()
}

// -- what the core asks the shell to do --------------------------------------

@Serializable
sealed class BalanceOperation {
    /**
     * Read every holding for this address, across every chain.
     *
     * The heavy one: token discovery, a multicall per chain, prices. `pull`
     * says a person asked for it by pulling, which the core uses to decide what
     * the screen may show while it runs.
     */
    @Serializable
    @SerialName("fetch_tokens")
    data class FetchTokens(
        val address: String,
        val force: Boolean,
        val pull: Boolean,
    ) : BalanceOperation()

    /** The account switcher's rows: TTL-cached, never forced, never streaming. */
    @Serializable
    @SerialName("fetch_account_assets")
    data class FetchAccountAssets(val address: String) : BalanceOperation()

    @Serializable
    @SerialName("read_balance_cache")
    data class ReadBalanceCache(val address: String) : BalanceOperation()

    @Serializable
    @SerialName("read_balance_cache_many")
    data class ReadBalanceCacheMany(val addresses: List<String> = emptyList()) : BalanceOperation()

    @Serializable
    @SerialName("write_balance_cache")
    data class WriteBalanceCache(val address: String, val usd: Double) : BalanceOperation()

    @Serializable
    @SerialName("start_retry_timer")
    data class StartRetryTimer(val ms: Int, val timer_id: Int) : BalanceOperation()

    @Serializable
    @SerialName("write_privacy")
    data class WritePrivacy(val hidden: Boolean) : BalanceOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class BalanceShellResult {
    @Serializable
    @SerialName("fetch_settled")
    data class FetchSettled(
        val address: String,
        val pull: Boolean,
        val tokens: List<BalanceToken> = emptyList(),
        val failed_chain_ids: List<Int> = emptyList(),
        val rate_limited_chain_ids: List<Int> = emptyList(),
        val now_ms: Double,
    ) : BalanceShellResult()

    /**
     * The fetch threw.
     *
     * Distinct from settling with nothing: the core keeps the last-known tokens
     * and total, so the screen loses its skeleton and nothing else moves.
     */
    @Serializable
    @SerialName("fetch_errored")
    data class FetchErrored(val address: String, val pull: Boolean) : BalanceShellResult()

    @Serializable
    @SerialName("account_assets_fetched")
    data class AccountAssetsFetched(
        val address: String,
        val tokens: List<BalanceToken>? = null,
    ) : BalanceShellResult()

    @Serializable
    @SerialName("cached_total_loaded")
    data class CachedTotalLoaded(
        val address: String,
        val usd: Double? = null,
    ) : BalanceShellResult()

    @Serializable
    @SerialName("cached_balances_loaded")
    data class CachedBalancesLoaded(
        val balances: List<BalanceCacheEntry> = emptyList(),
    ) : BalanceShellResult()

    @Serializable
    @SerialName("balance_cache_written")
    data object BalanceCacheWritten : BalanceShellResult()

    @Serializable
    @SerialName("retry_elapsed")
    data class RetryElapsed(val timer_id: Int) : BalanceShellResult()

    @Serializable
    @SerialName("privacy_written")
    data object PrivacyWritten : BalanceShellResult()
}
