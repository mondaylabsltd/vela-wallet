package app.getvela.wallet.feature.wallet

import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.wallet.core.BalanceView

/**
 * The home status line's rescue (spec 048, 092): which sheet is up over the
 * wallet. It used to push Settings and open the first failed network's RPC
 * fix there (F08: 「4 个网络 RPC 不可用」 showed BNB and hid the other three).
 * Now the line opens a sheet ON the wallet — every unreachable network (SR6)
 * when any is, the breakdown (SR3) otherwise — and a row's 修复 swaps that same
 * sheet to its network's fix (SR2); closing the fix steps back to the list.
 * One host, its content swapped: never a sheet stacked on a sheet.
 */
data class WalletRescue(
    val overlay: SettingsOverlay = SettingsOverlay.None,
    /** The network SR2 is about, while it is up. */
    val chainId: Long? = null,
    /** SR2 was opened from SR6's row: closing it returns to the list. */
    val fromList: Boolean = false,
) {
    /** The core re-reads every network while this is true (`UnreachableListOpened`). */
    val listOpen: Boolean get() = overlay == SettingsOverlay.Unreachable || fromList

    /** A row's 修复: that network's fix, in the same sheet. */
    fun fix(chainId: Int): WalletRescue = WalletRescue(SettingsOverlay.RpcFix, chainId.toLong(), fromList = true)

    /** ✕, a swipe, or Done once restored: a fix from the list steps back to it; anything else closes. */
    fun dismissed(): WalletRescue =
        if (overlay == SettingsOverlay.RpcFix && fromList) WalletRescue(SettingsOverlay.Unreachable) else WalletRescue()

    /** What moving to [next] tells the balance core about the list's re-reads, if anything. */
    fun listEdge(next: WalletRescue): ListEdge? = when {
        !listOpen && next.listOpen -> ListEdge.Opened
        listOpen && !next.listOpen -> ListEdge.Closed
        else -> null
    }

    enum class ListEdge { Opened, Closed }

    companion object {
        /** The status line, tapped: every unreachable network when any is; the breakdown otherwise. */
        fun opened(view: BalanceView): WalletRescue = WalletRescue(
            if (view.unreachable_networks.isNotEmpty()) SettingsOverlay.Unreachable else SettingsOverlay.BalanceDetail,
        )
    }
}
