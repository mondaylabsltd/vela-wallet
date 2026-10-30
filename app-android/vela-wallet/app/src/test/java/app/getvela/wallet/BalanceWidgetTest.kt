package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.BalanceModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.core.widget.BalanceWidgetModel
import app.getvela.wallet.core.widget.BalanceWidgetSnapshot
import app.getvela.wallet.core.widget.BalanceWidgetSnapshot.Write
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The home-screen balance widget (native parity Phase 3): what the wallet
 * route writes from the hero it drew, and what the widget then says — the
 * home's own figure, honestly dated, hidden when the home is hidden, and
 * nothing of its own making. Words through the real i18n runtime.
 */
class BalanceWidgetTest {
    private fun runtime(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        return I18nRuntime { t -> File(root, "assets/i18n/$t.json").readBytes() }.apply { initialize(tag) }
    }

    private fun hero(state: BalanceStateKind, integer: String? = "$1,383", decimals: String? = "28") = BalanceModel(
        label = "Total balance",
        currency = "USD",
        state = state,
        integer = integer,
        decimals = decimals,
        a11yHide = "",
        a11yShow = "",
    )

    private val at = 1_761_800_000_000.0

    @Test
    fun `a settled figure is saved with the core's time of the read`() {
        val write = BalanceWidgetSnapshot.of(hero(BalanceStateKind.Normal), at, "Main", testnet = false)
        assertEquals(Write.Save(BalanceWidgetSnapshot("$1,383", "28", ".", masked = false, atMs = at.toLong(), account = "Main")), write)
        // Zero, live, is a figure too.
        assertTrue(BalanceWidgetSnapshot.of(hero(BalanceStateKind.ZeroLive, "$0", "00"), at, "Main", false) is Write.Save)
    }

    @Test
    fun `hidden on the home is hidden on the widget - no figure is kept at all`() {
        val write = BalanceWidgetSnapshot.of(hero(BalanceStateKind.Hidden, WalletLive.BALANCE_MASK, null), at, "Main", false) as Write.Save
        assertTrue(write.snapshot.masked)
        assertNull(write.snapshot.integer)
        assertNull(write.snapshot.decimals)
    }

    @Test
    fun `still reading, no value, or the testnet space keep the last real figure`() {
        assertEquals(Write.Keep, BalanceWidgetSnapshot.of(hero(BalanceStateKind.Loading, null, null), at, "Main", false))
        assertEquals(Write.Keep, BalanceWidgetSnapshot.of(hero(BalanceStateKind.NoValue, null, null), at, "Main", false))
        assertEquals(Write.Keep, BalanceWidgetSnapshot.of(hero(BalanceStateKind.Normal, null, null), at, "Main", false))
        assertEquals(Write.Keep, BalanceWidgetSnapshot.of(hero(BalanceStateKind.Normal), at, "Main", testnet = true))
    }

    @Test
    fun `the widget says the figure, how old it is, and whose`() {
        val strings = runtime("en")
        val snapshot = BalanceWidgetSnapshot("$1,383", "28", ".", masked = false, atMs = at.toLong(), account = "Main")
        val model = BalanceWidgetModel.of(snapshot, strings, nowMs = at.toLong() + 2 * 60 * 60 * 1000)
        assertEquals("Total balance", model.label)
        assertEquals("$1,383", model.integer)
        assertEquals(".28", model.decimals)
        assertTrue("dated by the core's relative time: ${model.footnote}", model.footnote!!.startsWith("Updated "))
        assertTrue(model.footnote!!.endsWith(" · Main"))
        // The number preset's own mark between whole and cents.
        val comma = BalanceWidgetModel.of(snapshot.copy(decimalMark = ","), strings, at.toLong())
        assertEquals(",28", comma.decimals)
    }

    @Test
    fun `masked, undated, and empty`() {
        val strings = runtime("zh")
        val masked = BalanceWidgetModel.of(BalanceWidgetSnapshot(null, null, ".", masked = true, atMs = null, account = "Main"), strings, at.toLong())
        assertEquals(WalletLive.BALANCE_MASK, masked.integer)
        assertNull(masked.decimals)
        assertEquals("never dated when the core never said when", "Main", masked.footnote)
        val none = BalanceWidgetModel.of(null, strings, at.toLong())
        assertEquals("总余额", none.label)
        assertNull("no wallet: no figure", none.integer)
        assertNull(none.footnote)
        val nameless = BalanceWidgetModel.of(BalanceWidgetSnapshot("$1", "00", ".", false, null, ""), strings, at.toLong())
        assertNull(nameless.footnote)
    }
}
