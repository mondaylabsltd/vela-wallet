package app.getvela.wallet

import app.getvela.wallet.core.i18n.AppLanguage
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.StoredAccountsNotice
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.onboarding.core.SessionRoute
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * §18 item 2 (the desktop's DESKTOP-1, on Android): a stored account list —
 * or one record of it — this device cannot read is SAID (the welcome's
 * notice, a line under the switcher's list), KEPT byte for byte on every
 * write (never a shorter list written back), and signing in with the passkey
 * is the way back. The rule is the core's (`stored_accounts`, through UniFFI).
 */
class StoredAccountsUnreadableTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")

    @After
    fun tearDown() {
        scope.cancel()
        AppLanguage.tag = I18nRuntime.FALLBACK
    }

    private fun strings(): VelaStrings = I18nRuntime { name -> File(root, "assets/i18n/$name.json").readBytes() }.apply { initialize("en") }

    private val good = """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","public_key_hex":"04ab","created_at_iso":"2026-08-25T10:00:00.000Z","keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}"""
    private val other = """{"id":"cred-2","name":"Bo","address":"0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984","public_key_hex":"04cd","created_at_iso":"2026-08-26T10:00:00.000Z","keys":[{"credential_id":"cred-2","public_key_hex":"04cd","name":"Bo"}]}"""
    /** Its key's id is a number: a record this build cannot read. Its `\/` would not survive org.json. */
    private val bad = """{"id":"x","address":"0x00000000000000000000000000000000000000Cc","keys":[{"credential_id":7}],"note":"a\/b"}"""

    @Test
    fun `one record it cannot read costs only itself, is counted, and is kept byte for byte`() = runBlocking {
        val store = FakeStore(mapOf("vela.accounts" to "[$good,$bad]"))
        val accounts = AccountStore(store)
        val read = accounts.loadAccounts()
        assertEquals(1, read.length())
        assertEquals("cred-1", read.getJSONObject(0).getString("id"))
        assertEquals(1, AccountStore.unreadable.value)

        accounts.saveAccount(JSONObject(other))
        val saved = store.values["vela.accounts"].orEmpty()
        assertTrue("the record it cannot read, as it was stored: $saved", saved.contains(bad))
        assertEquals(2, accounts.loadAccounts().length())

        accounts.removeAccountAtAddress("0x1f9840a85d5af5bf1d1762f925bdaddc4201f984")
        val removed = store.values["vela.accounts"].orEmpty()
        assertTrue("never a shorter list by what could not be read: $removed", removed.contains(bad))
        assertFalse(removed.contains("cred-2"))
        accounts.removeAccount("cred-1")
        assertEquals("[$bad]", store.values["vela.accounts"])
        assertEquals(1, AccountStore.unreadable.value)
    }

    @Test
    fun `a stored value that is no list is one unreadable record, and signing in keeps it`() = runBlocking {
        val store = FakeStore(mapOf("vela.accounts" to """{"a":1}"""))
        val accounts = AccountStore(store)
        assertEquals(0, accounts.loadAccounts().length())
        assertEquals(1, AccountStore.unreadable.value)
        // The way back: the passkey's sign-in writes its record — beside it.
        accounts.saveAccount(JSONObject(good))
        assertEquals("""[{"a":1},""" + JSONObject(good).toString() + "]", store.values["vela.accounts"])
        assertEquals(1, accounts.loadAccounts().length())
    }

    @Test
    fun `an unreadable list says so on the welcome rather than looking signed out`() = runBlocking {
        val store = FakeStore(mapOf("vela.accounts" to "[$bad]"))
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        val settled = withTimeout(10_000) { session.view.first { !it.loading } }
        assertFalse(settled.hasWallet)
        assertEquals(SessionRoute.Onboarding, settled.allowedRoute)
        assertTrue(StoredAccountsNotice.shown(settled.accountsUnreadable, AccountStore.unreadable.value))
        assertEquals("[$bad]", store.values["vela.accounts"])
    }

    @Test
    fun `a good record beside a bad one still opens the wallet`() = runBlocking {
        val store = FakeStore(mapOf("vela.accounts" to "[$bad,$good]", "vela.activeAccountIndex" to "0"))
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        val settled = withTimeout(10_000) { session.view.first { !it.loading } }
        assertTrue(settled.hasWallet)
        assertEquals(listOf("0x88cCA0EeDbF2C4426110bbFc998F048689266894"), settled.accounts.map { it.address })
        assertEquals(1, AccountStore.unreadable.value)
    }

    @Test
    fun `the notice and the switcher's line are the device-neutral words`() {
        assertTrue(StoredAccountsNotice.shown(sessionUnreadable = true, storedUnreadable = 0))
        assertTrue(StoredAccountsNotice.shown(sessionUnreadable = false, storedUnreadable = 2))
        assertFalse(StoredAccountsNotice.shown(sessionUnreadable = false, storedUnreadable = 0))
        val words = strings()
        val title = words.t(I18nKeys.Storage.DEVICE_UNREADABLE_TITLE)
        assertFalse(title.startsWith("onboarding."))
        assertFalse("no browser on a phone: $title", title.contains("browser", ignoreCase = true))
        assertFalse(words.t(I18nKeys.Storage.DEVICE_UNREADABLE_BODY).contains("browser", ignoreCase = true))
        val currency = CurrencyView(code = "USD", rate = 1.0, committed = true)
        val switcher = { unreadable: Int ->
            WalletLive.accountSwitcher(listOf("Ann" to "0x88cCA0EeDbF2C4426110bbFc998F048689266894"), 0, BalanceSwitcherView(), currency, words, unreadable = unreadable)
        }
        assertEquals(title, switcher(1).unreadable)
        assertNull(switcher(0).unreadable)
    }
}
