package app.getvela.wallet.feature.wallet

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * Wallet view models (spec 015, data-model.md — Android port of the web's
 * `src/lib/wallet/model.ts`).
 *
 * Components consume ONLY these display-ready shapes — no service types, no
 * formatting, no fetching (spec FR-005 / SC-005). A later "real data" feature
 * replaces the fixture layer that builds them and nothing else.
 */

enum class WalletScreenState { H1, H1S, H2, H3, H4, H5, H6, H7, H7X, H8 }

@Immutable
data class WalletHeaderModel(
    val name: String,
    val addressDisplay: String,
    /** Identicon seed (the full address); rendered via core/identicon. */
    val identiconSeed: String,
)

@Immutable
sealed interface NetworkPillModel {
    data class All(val dots: List<Color>, val label: String) : NetworkPillModel
    data class Single(val dot: Color, val label: String) : NetworkPillModel
}

enum class BalanceStateKind { Normal, ZeroLive, Loading, Hidden }

enum class BalanceStatusKind { Warning, Refreshing }

@Immutable
data class BalanceStatusModel(val kind: BalanceStatusKind, val text: String)

@Immutable
data class BalanceModel(
    val label: String,
    val currency: String,
    val state: BalanceStateKind,
    /** e.g. "$1,383" — null while loading; mask dots while hidden. */
    val integer: String? = null,
    /** e.g. "28" — rendered de-emphasised after the separator. */
    val decimals: String? = null,
    /**
     * Spec 049: the mark between the two is the number preset's (the web's
     * `decimalMark`). A `.` drawn after `1.575` read as a second thousands
     * separator.
     */
    val decimalMark: String = ".",
    val liveText: String? = null,
    val status: BalanceStatusModel? = null,
    val a11yHide: String,
    val a11yShow: String,
)

enum class ActivityKind { Sent, Received, Dapp }

@Immutable
data class ActivityRowModel(
    /**
     * Which transaction this row is.
     *
     * Every row opened the same detail screen before this existed, so tapping
     * one payment showed another. A row that can be tapped has to be able to
     * say what it is.
     */
    val id: String = "",
    val kind: ActivityKind,
    val title: String,
    val subtitle: String,
    val amount: String,
    val unit: String,
    val positive: Boolean,
    val masked: Boolean,
    val badgeColor: Color,
    /**
     * The chain's logo for the avatar's badge (spec 047's rule, §8.3's
     * drawing): the row draws the network it happened on, and the coloured
     * dot is what is left when the endpoint has no logo. Without this the
     * badge was ALWAYS the dot — a colour nobody can read as a network.
     */
    val badgeLogoUrl: String? = null,
)

@Immutable
data class ActivityGroupModel(val label: String, val rows: List<ActivityRowModel>)

@Immutable
sealed interface AssetFiatModel {
    data class Value(val text: String) : AssetFiatModel
    data class NoPrice(val text: String) : AssetFiatModel
    data object Masked : AssetFiatModel

    /**
     * Spec 021 SD2d: the row has no fiat line at all. Distinct from [Masked],
     * which HIDES a figure that exists — a sweep row is an editable amount, and
     * dots under it read as a concealed second number.
     */
    data object None : AssetFiatModel
}

@Immutable
data class AssetRowModel(
    /** `chainId:contract`, or `chainId:native` — which holding this row is. */
    val id: String = "",
    val ticker: String,
    val chain: String,
    val badgeColor: Color,
    val balance: String,
    val fiat: AssetFiatModel,
    val masked: Boolean,
    /** Spec 047: the web's logo rules — candidates in order, the badge's logo, the badge hidden when it repeats the token. */
    val logoUrls: List<String> = emptyList(),
    val badgeLogoUrl: String? = null,
    val badgeHidden: Boolean = false,
)

enum class SectionMode { Rows, Empty, Loading }

@Immutable
data class EmptyStateModel(val title: String, val caption: String)

@Immutable
data class SectionModel(
    val title: String,
    val action: String,
    val mode: SectionMode,
    val empty: EmptyStateModel? = null,
)

@Immutable
data class ChainRowModel(
    val name: String,
    /** null = the all-networks row (neutral dot). */
    val dot: Color?,
    val count: Int,
    val selected: Boolean,
)

@Immutable
data class SheetModel(val title: String, val rows: List<ChainRowModel>)

@Immutable
data class TabsModel(
    val wallet: String,
    val contacts: String,
    val explore: String,
    val settings: String,
)

@Immutable
data class ActionsModel(val receive: String, val send: String, val scan: String)

@Immutable
data class WalletHomeModel(
    val state: WalletScreenState,
    val header: WalletHeaderModel,
    val pill: NetworkPillModel,
    val balance: BalanceModel,
    val actions: ActionsModel,
    val activitySection: SectionModel,
    val activityGroups: List<ActivityGroupModel>,
    val assetsSection: SectionModel,
    val assetRows: List<AssetRowModel>,
    val tabs: TabsModel,
    val sheet: SheetModel? = null,
    /** 1 or 1.35 — multiplies the font scale via LocalDensity (spec FR-011). */
    val textScale: Float = 1f,
) {
    /**
     * Swap in the signed-in wallet's real address (spec 019).
     *
     * The rest of this screen is still the spec-015 fixture layer, and that is
     * the point of doing it here rather than inside the fixtures: an address is
     * the ONE thing on the home screen a person acts on, and showing a fixture
     * address after a real wallet has been created would be the app telling them
     * their money is somewhere it is not. Everything else on the page is
     * visibly-placeholder balance data; an address is not.
     *
     * An empty address changes nothing — the developer routes reach this screen
     * with no session at all.
     */
    fun withAddress(address: String): WalletHomeModel = if (address.isEmpty()) {
        this
    } else {
        copy(
            header = header.copy(
                addressDisplay = shortenAddress(address),
                identiconSeed = address,
            ),
        )
    }

    /**
     * Swap in the signed-in wallet's real NAME (spec 019).
     *
     * Same argument as the address, and found the same way — on a device. The
     * header drew the fixture's 大表哥 over a real address and a real
     * identicon, so the one line on the screen that was wrong was the one line
     * a person reads as "this is my wallet". An empty name changes nothing:
     * the developer routes reach this screen with no session at all.
     */
    fun withName(name: String): WalletHomeModel = if (name.isEmpty()) {
        this
    } else {
        copy(header = header.copy(name = name))
    }
}

/** `0x1234…cdef` — the house short form, matching the other three clients. */
private fun shortenAddress(address: String): String =
    if (address.length <= SHORT_ADDRESS_KEEP_HEAD + SHORT_ADDRESS_KEEP_TAIL) {
        address
    } else {
        address.take(SHORT_ADDRESS_KEEP_HEAD) + "\u2026" + address.takeLast(SHORT_ADDRESS_KEEP_TAIL)
    }

private const val SHORT_ADDRESS_KEEP_HEAD = 6
private const val SHORT_ADDRESS_KEEP_TAIL = 4
