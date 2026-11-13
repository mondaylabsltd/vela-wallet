package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.SessionUnsynced
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * P2.4: the home's unsynced line (ob-2, the web's `syncLine`), the account
 * rows' note, the erase sheet naming what it would lose (`withEraseUnsynced`),
 * and the balance breakdown saying a network is read through public nodes
 * (owner decision 2026-10-28, `fallback_chain_ids`).
 */
class HomeSyncAndNodesTest {

    private fun runtime(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        return I18nRuntime { t -> File(root, "assets/i18n/$t.json").readBytes() }.apply { initialize(tag) }
    }

    private val strings by lazy { runtime("en") }

    private val retrying = listOf(SessionUnsynced(name = "Daily", address = ME, retrying = true))

    // -- the home's line -----------------------------------------------------------

    @Test
    fun nothingOwedSaysNothing() {
        assertNull(WalletLive.syncLine(emptyList(), ME, strings, failedRounds = 0))
        assertNull("another wallet's debt is not this one's", WalletLive.syncLine(retrying, OTHER, strings, failedRounds = 5))
    }

    @Test
    fun syncingWhileTheSessionStillTriesThenStalledWithARetry() {
        val early = WalletLive.syncLine(retrying, ME.uppercase().replace("0X", "0x"), strings, failedRounds = 1)!!
        assertEquals(strings.t(I18nKeys.Wallet.SYNCING), early.text)
        assertFalse(early.stalled)
        assertNull(early.retry)
        val late = WalletLive.syncLine(retrying, ME, strings, failedRounds = 2)!!
        assertEquals(strings.t(I18nKeys.Wallet.SYNC_STALLED), late.text)
        assertTrue(late.stalled)
        assertEquals(strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRY), late.retry)
        assertTrue("real words, not a key", late.text != I18nKeys.Wallet.SYNC_STALLED)
    }

    @Test
    fun aWalletTheSessionCannotFinishSaysSoWithNoRetry() {
        val stuck = WalletLive.syncLine(listOf(SessionUnsynced(name = "", address = ME, retrying = false)), ME, strings, failedRounds = 0)!!
        assertTrue(stuck.stalled)
        assertNull(stuck.retry)
    }

    @Test
    fun theLineIsInChineseInChinese() {
        val zh = runtime("zh")
        assertEquals(zh.t(I18nKeys.Wallet.SYNCING), WalletLive.syncLine(retrying, ME, zh, 0)!!.text)
        assertTrue(zh.t(I18nKeys.Wallet.SYNCING) != strings.t(I18nKeys.Wallet.SYNCING))
    }

    // -- the account rows --------------------------------------------------------------

    @Test
    fun anUnsyncedAccountRowSaysSoAndTheOthersDoNot() {
        val sheet = WalletLive.accountSwitcher(
            listOf("Daily" to ME, "Savings" to OTHER),
            activeIndex = 0,
            switcher = BalanceSwitcherView(),
            currency = CurrencyView("USD", 1.0, true),
            strings = strings,
            unsynced = retrying,
        )
        assertEquals(strings.t(I18nKeys.Wallet.SYNCING), sheet.rows[0].note)
        assertNull(sheet.rows[1].note)
    }

    // -- the erase sheet ---------------------------------------------------------------

    @Test
    fun theEraseSheetNamesTheUnsyncedWalletsAndWaitsForTheTick() {
        val base = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        assertEquals("nothing owed: the sheet as it was", base, SettingsLive.withEraseUnsynced(base, emptyList(), strings))
        val owed = listOf(SessionUnsynced(name = "Daily", address = ME, retrying = true), SessionUnsynced(name = " ", address = OTHER, retrying = false))
        val sheet = SettingsLive.withEraseUnsynced(base, owed, strings).eraseSheet
        assertTrue(sheet.warning!!.contains("Daily"))
        assertTrue("an unnamed wallet by its short address", sheet.warning!!.contains("0x222222…222222"))
        assertEquals(strings.t(I18nKeys.SettingsUi.ERASE_UNSYNCED_ACK), sheet.ack)
        assertEquals(base.eraseSheet.title, sheet.title)
    }

    // -- the breakdown's public nodes ----------------------------------------------------

    @Test
    fun aNetworkReadThroughPublicNodesSaysSo() {
        val base = SettingsFixtures.buildState(SettingsScreenState.ST1, strings).balanceDetail
        val view = BalanceView(
            address = ME,
            tokens = listOf(
                BalanceToken(chain_id = 1, symbol = "ETH", name = "Ether", balance = "1", decimals = 18, price_usd = 2000.0),
                BalanceToken(chain_id = 100, symbol = "XDAI", name = "xDAI", balance = "3", decimals = 18, price_usd = 1.0),
            ),
            fallback_chain_ids = listOf(100, 8453),
        )
        val names = mapOf(1 to "Ethereum", 100 to "Gnosis", 8453 to "Base")
        val detail = SettingsLive.balanceDetail(base, view, CurrencyView("USD", 1.0, true), names, strings)
        val gnosis = detail.done.single { it.id == "100" }
        assertEquals(strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_PUBLIC_NODES), gnosis.status)
        assertEquals(SettingsTone.Warn, gnosis.tone)
        assertNull(detail.done.single { it.id == "1" }.status)
        // A fallback network with nothing priced is still listed, at its real zero.
        val base8453 = detail.done.single { it.id == "8453" }
        assertEquals(strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_PUBLIC_NODES), base8453.status)
    }

    companion object {
        const val ME = "0x1111111111111111111111111111111111111111"
        const val OTHER = "0x2222222222222222222222222222222222222222"
    }
}
