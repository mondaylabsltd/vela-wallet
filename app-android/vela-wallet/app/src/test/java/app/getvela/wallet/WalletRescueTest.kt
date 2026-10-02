package app.getvela.wallet

import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.wallet.WalletRescue
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.UnreachableNetwork
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 092 (F08): the home status line's rescue is a sheet over the wallet.
 * It used to push Settings and open the first failed network's RPC fix there;
 * now SR6 lists every unreachable network, a row's 修复 swaps the same sheet to
 * that network's SR2, and closing the fix returns to the list.
 */
class WalletRescueTest {
    private val down = BalanceView(
        unreachable_networks = listOf(UnreachableNetwork(56), UnreachableNetwork(137), UnreachableNetwork(10)),
        unreachable_key = "assets.unreachableMany",
    )

    @Test
    fun `the line opens the list when any network is down, the breakdown otherwise`() {
        assertEquals(SettingsOverlay.Unreachable, WalletRescue.opened(down).overlay)
        assertEquals(SettingsOverlay.BalanceDetail, WalletRescue.opened(BalanceView()).overlay)
    }

    @Test
    fun `a row's fix swaps to THAT network and its close button returns to the list`() {
        val list = WalletRescue.opened(down)
        val fix = list.fix(137)
        assertEquals(SettingsOverlay.RpcFix, fix.overlay)
        assertEquals(137L, fix.chainId)
        // Not the first network (BNB) — the one tapped.
        assertTrue(fix.fromList)

        val back = fix.closed()
        assertEquals(SettingsOverlay.Unreachable, back.overlay)
        assertNull(back.chainId)

        val closed = back.closed()
        assertEquals(SettingsOverlay.None, closed.overlay)
    }

    /**
     * Device pass, 2026-10-02: a swipe, the scrim or Back reach the host only
     * after Material has hidden the sheet — "back to the list" there kept the
     * list (and its re-reads) alive behind a sheet nobody could see. Those
     * close everything, as a swipe does on the iPhone.
     */
    @Test
    fun `a swipe closes the whole sheet, from the fix or the list`() {
        val list = WalletRescue.opened(down)
        val fix = list.fix(56)
        assertEquals(SettingsOverlay.None, fix.swiped().overlay)
        assertEquals(WalletRescue.ListEdge.Closed, fix.listEdge(fix.swiped()))
        assertEquals(SettingsOverlay.None, list.swiped().overlay)
    }

    @Test
    fun `the core re-reads while the list or a fix opened from it is up, and only then`() {
        val none = WalletRescue()
        val list = WalletRescue.opened(down)
        val fix = list.fix(56)
        assertEquals(WalletRescue.ListEdge.Opened, none.listEdge(list))
        assertNull("still the list's: a fix from it keeps the re-reads", list.listEdge(fix))
        assertNull(fix.listEdge(fix.closed()))
        assertEquals(WalletRescue.ListEdge.Closed, list.listEdge(list.closed()))

        val detail = WalletRescue.opened(BalanceView())
        assertFalse(detail.listOpen)
        assertNull(none.listEdge(detail))
    }

    /**
     * The tap stays on the wallet: the route no longer hands the rescue to
     * Settings (the parked-overlay flow that pushed it is gone), and the wallet
     * route hosts the sheet itself.
     */
    @Test
    fun `the status line never sends the person to Settings`() {
        val root = File(System.getProperty("vela.repo.root")!!, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
        val nav = File(root, "navigation/VelaNavHost.kt").readText()
        val app = File(root, "VelaWalletApplication.kt").readText()
        assertFalse(nav.contains("pendingSettingsOverlay"))
        assertFalse(app.contains("pendingSettingsOverlay"))
        val tap = nav.substringAfter("onStatusClick = {").substringBefore("},")
        assertTrue(tap, tap.contains("moveRescue(WalletRescue.opened(balances))"))
        assertFalse(tap, tap.contains("navController"))
        // The wallet hosts the sheet itself, through the one composable whose
        // close button and swipe are driven on a device by WalletRescueSheetTest.
        assertTrue(nav.contains("WalletRescueSheet("))
    }
}
