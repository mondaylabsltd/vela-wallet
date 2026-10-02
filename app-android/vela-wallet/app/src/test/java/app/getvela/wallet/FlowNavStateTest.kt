package app.getvela.wallet

import app.getvela.wallet.feature.flows.FlowNavState
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.FlowStep
import app.getvela.wallet.feature.flows.WalletFlowEntry
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Issue #328 (Android, v0.9.5): Assets → tap ETH → its sheet → × → tap POL →
 * nothing. The × only hid the sheet: T2 stayed on top of the flow stack, so the
 * next row pushed the same step, which [FlowNavState.push] ignores — no sheet,
 * and the id the row carried was dropped with it. The same shape iOS fixed as
 * 082 X-HISTORY (`FlowNav.sheetClosed`).
 */
class FlowNavStateTest {

    private val eth = "42161:native"
    private val pol = "137:native"

    @Test
    fun `a closed token sheet takes its level with it and the next row opens`() {
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Assets)
        nav.push(FlowStep.TokenDetail, eth)
        assertEquals(listOf(FlowState.T1, FlowState.T2), nav.stack)
        assertEquals(eth, nav.selected)

        nav.sheetClosed(FlowState.T2)
        assertEquals("the closed sheet is gone", listOf(FlowState.T1), nav.stack)
        assertNull("its id went with it", nav.selected)

        nav.push(FlowStep.TokenDetail, pol)
        assertEquals("the next row opens its sheet", listOf(FlowState.T1, FlowState.T2), nav.stack)
        assertEquals(pol, nav.selected)
    }

    @Test
    fun `without the close the next row is swallowed - the reported failure`() {
        // What the × left behind before the fix: T2 still on top.
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Assets)
        nav.push(FlowStep.TokenDetail, eth)
        nav.push(FlowStep.TokenDetail, pol)
        assertEquals(listOf(FlowState.T1, FlowState.T2), nav.stack)
        assertEquals("push ignores the same step, so the hidden sheet kept ETH", eth, nav.selected)
    }

    @Test
    fun `ten closes in a row each leave the list able to open the next row`() {
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Assets)
        repeat(10) { i ->
            val id = if (i % 2 == 0) eth else pol
            nav.push(FlowStep.TokenDetail, id)
            assertEquals("open #$i", FlowState.T2, nav.top)
            assertEquals("open #$i is about the row tapped", id, nav.selected)
            nav.sheetClosed(FlowState.T2)
            assertEquals("close #$i", listOf(FlowState.T1), nav.stack)
        }
    }

    @Test
    fun `the same row opens again after its sheet was closed`() {
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Assets)
        nav.push(FlowStep.TokenDetail, eth)
        nav.sheetClosed(FlowState.T2)
        assertEquals("closed, the list is on top", FlowState.T1, nav.top)
        nav.push(FlowStep.TokenDetail, eth)
        assertEquals(FlowState.T2, nav.top)
        assertEquals(eth, nav.selected)
    }

    @Test
    fun `a sheet opened from the home closes onto the list, and the next row opens`() {
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.TokenDetail, eth)
        assertEquals(listOf(FlowState.T1, FlowState.T2), nav.stack)
        nav.sheetClosed(FlowState.T2)
        assertEquals(listOf(FlowState.T1), nav.stack)
        nav.push(FlowStep.TokenDetail, pol)
        assertEquals(pol, nav.selected)
    }

    @Test
    fun `every pushed sheet - token, transaction, code, add - reopens after a close`() {
        val cases = listOf(
            Triple(WalletFlowEntry.Assets, FlowStep.TokenDetail, FlowState.T2),
            Triple(WalletFlowEntry.Activity, FlowStep.TxDetail, FlowState.A2),
            Triple(WalletFlowEntry.Receive, FlowStep.ReceiveQr, FlowState.R2),
            Triple(WalletFlowEntry.Assets, FlowStep.AddToken, FlowState.T3),
        )
        for ((entry, step, sheet) in cases) {
            val nav = FlowNavState()
            nav.enter(entry)
            val root = nav.stack
            repeat(10) {
                nav.push(step, "row-$it")
                assertEquals("$step opens (#$it)", sheet, nav.top)
                nav.sheetClosed(sheet)
                assertEquals("$step closes onto its list (#$it)", root, nav.stack)
            }
        }
    }

    @Test
    fun `back after a close leaves the flow at once - no invisible level`() {
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Activity)
        nav.push(FlowStep.TxDetail, "0xabc")
        nav.sheetClosed(FlowState.A2)
        nav.back()
        assertFalse("one ‹ from the list is the wallet", nav.isOpen)
    }

    @Test
    fun `a close for a level that is not on top pops nothing`() {
        // A sheet the send machine derives (fee coin, contacts, import) is never
        // pushed: its close must not take the form under it.
        val nav = FlowNavState()
        nav.enter(WalletFlowEntry.Send)
        nav.push(FlowStep.SendForm)
        nav.sheetClosed(FlowState.SD2F)
        assertEquals(listOf(FlowState.SD1, FlowState.SD2), nav.stack)

        // A second report of the same close (a late dismiss) pops nothing more.
        nav.enter(WalletFlowEntry.Assets)
        nav.push(FlowStep.TokenDetail, eth)
        nav.sheetClosed(FlowState.T2)
        nav.sheetClosed(FlowState.T2)
        assertEquals(listOf(FlowState.T1), nav.stack)
        assertTrue(nav.isOpen)
    }

    /**
     * The wiring, which the state holder alone cannot prove: every door out of
     * a flow sheet — the ×, and the sheet's drag / scrim / Back through
     * `onDismissRequest` — goes through the one `dismiss`, which reports to the
     * host; and the wallet's host pops the level it was built for.
     */
    @Test
    fun `every door out of a flow sheet reports the close, and the wallet pops it`() {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val src = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
        val host = File(src, "feature/flows/FlowHost.kt").readText()
        val sheetHost = host.substringAfter("private fun FlowSheetHost(").substringBefore("private fun sheetTitle(")
        val dismiss = sheetHost.substringAfter("val dismiss = {").substringBefore("}")
        assertTrue("dismiss reports the close to the host", dismiss.contains("onSheetClosed()"))
        assertTrue("drag, scrim and Back report through dismiss", sheetHost.contains("onDismissRequest = { dismiss() }"))
        assertTrue("the × reports through dismiss", Regex("SheetTitleRow\\(.*\\)\\s*\\{\\s*dismiss\\(\\)\\s*}").containsMatchIn(sheetHost))
        val nav = File(src, "navigation/VelaNavHost.kt").readText()
        assertTrue("the wallet's flow host pops the closed sheet", nav.contains("onSheetClosed = { flows.sheetClosed(flowState) }"))
    }
}
