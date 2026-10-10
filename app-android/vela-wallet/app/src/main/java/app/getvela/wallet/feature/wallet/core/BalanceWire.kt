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
    /** Empty while [hidden]: the core withholds every switcher figure. */
    val balances: List<BalanceCacheEntry> = emptyList(),
    /** Balance privacy is on: every row and the total draw the mask (`app::privacy`). */
    val hidden: Boolean = false,
)

/** One network the last read could not reach, and what was last read there (spec 092). */
@Serializable
data class UnreachableNetwork(
    val chain_id: Int,
    /** `held` / `empty` / `not_read`. */
    val last_known: String = "not_read",
    /** The worth of what it last held, USD; `null` when nothing priced was held, or while hidden. */
    val last_seen_usd: Double? = null,
    /** The corpus key of the row's line; `assets.lastSeen` fills `{{amount}}`. */
    val line_key: String = "",
    /**
     * What kept it from being read: `network` (none of its RPC endpoints
     * answered) or `token_list` (its RPC answers; the list that names what to
     * read there could not be loaded, and it has no coin of its own to read
     * without one — Tempo). A string on purpose: a cause this build does not
     * know still has [rpc_fixable] to say whether a "Fix" belongs on the row.
     */
    val cause: String = CAUSE_NETWORK,
    /**
     * May the row offer its RPC editor ("Fix")? The core's word — true only
     * when the network itself did not answer. For any other cause the
     * endpoint is fine, and a "Fix RPC" there sends a person to repair what
     * is working. `true` from a core that predates the field: every row was
     * the network's then.
     */
    val rpc_fixable: Boolean = true,
    /**
     * The corpus key of the row's SHORT status in the balance breakdown — the
     * core's word (PR 3 final note F21): `home.balanceDetailStatusFailed`
     * ("RPC unavailable") when none of its endpoints answered,
     * `home.balanceDetailStatusTokenList` ("Token list unavailable") when its
     * RPC answers and its token list could not be loaded. The default is the
     * one status there was, for a core that predates the field.
     */
    val status_key: String = STATUS_RPC_UNAVAILABLE,
) {
    companion object {
        const val CAUSE_NETWORK = "network"
        const val CAUSE_TOKEN_LIST = "token_list"

        /** `balance_dashboard::STATUS_RPC_UNAVAILABLE`. */
        const val STATUS_RPC_UNAVAILABLE = "home.balanceDetailStatusFailed"
    }
}

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
     * no cache to fall back on (#188, spec 038 finding 15). A skeleton and a
     * reason, never a zero — and since the PR 2 polish the core says so in
     * the figure too: `display_total_usd` is `null` in this state (it was
     * `0.0`, which every screen had to know to keep off the hero).
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
    /**
     * Every network the wallet cannot reach (spec 092): failed minus
     * rate-limited, held or not — while one cannot be read nobody knows what it
     * holds now. In the core's order: last seen holding something first.
     */
    val unreachable_networks: List<UnreachableNetwork> = emptyList(),
    /** The corpus key of the home line over them; `null` when every network answered. */
    val unreachable_key: String? = null,
    /**
     * PR 2 note 11: the failed chains whose read never left the app — a fault
     * inside Vela, not the network's. Never in [unreachable_networks].
     */
    val internal_chain_ids: List<Int> = emptyList(),
    /**
     * The home line when the last read failed inside Vela itself (the fee's
     * own sentence for the same fault): drawn where the unreachable line
     * goes, in place of any "Can't reach …". `null` otherwise.
     */
    val internal_key: String? = null,
    /**
     * The hero's line while the FIRST read of this account is still out (PR 3
     * final note F19): `componentsUi.funding.checking` ("Checking…"). Until a
     * round has ended nothing here was said by a chain — a cached total of 0
     * is last session's — so the line under the total says the wallet is
     * being read, and neither "live" nor "can't reach" yet. `null` from the
     * first round's end on: a later refresh is not "checking".
     */
    val checking_key: String? = null,
    /**
     * The hero's line under a LIVE zero: `home.liveIndicator` ("Live ·
     * listening for payments"). Set only when the last round settled, every
     * chain it asked answered and the wallet holds nothing. **The "zero,
     * live" state is this key being set, and nothing else**: derived here
     * from the total and the partial flag, a cached zero drew "Live ·
     * listening" over a wallet nothing had read, then swapped it for "Can't
     * reach 24 networks".
     */
    val live_key: String? = null,
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

    /** Spec 092: the unreachable list is on screen — read again, and keep re-reading while it is. */
    @Serializable
    @SerialName("unreachable_list_opened")
    data object UnreachableListOpened : BalanceEvent()

    @Serializable
    @SerialName("unreachable_list_closed")
    data object UnreachableListClosed : BalanceEvent()

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
        /** Spec 092: every chain this round asked — one that answered empty is not "not read yet". */
        val read_chain_ids: List<Int> = emptyList(),
        /**
         * PR 2 note 11: the failed chains whose read never left the app (an
         * exception inside this shell before anything was sent) — a subset of
         * [failed_chain_ids], never said as "can't reach".
         */
        val internal_chain_ids: List<Int> = emptyList(),
        /**
         * The failed chains whose RPC was never the problem: the chain's
         * token list (the registry document that names its stablecoins) could
         * not be loaded, and the chain has no native coin to read without it
         * (Tempo). A subset of [failed_chain_ids]; the core says "can't load
         * its token list" for them and offers no RPC fix.
         */
        val registry_chain_ids: List<Int> = emptyList(),
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
    data class FetchErrored(
        val address: String,
        val pull: Boolean,
        /** PR 2 note 11: it threw inside the app before anything left it — Vela's own fault, never "can't reach". */
        val internal: Boolean = false,
    ) : BalanceShellResult()

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
