package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.CurrencyExecutor
import app.getvela.wallet.feature.settings.core.CurrencyOperation
import app.getvela.wallet.feature.settings.core.CurrencyShellResult
import java.util.Locale
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The `display_currency` shell contract, operation by operation
 * (`specs/040-android-live-shell/contracts/shell-operations.md`).
 *
 * Four operations is few enough to check exhaustively, which is the point of
 * starting here: if the road is right, this file is short and the next machine
 * costs the same four kinds of test.
 */
class CurrencyExecutorTest {

    private fun executor(
        store: KeyValueStore = FakeStore(),
        locale: Locale? = Locale.US,
    ) = CurrencyExecutor(store) { locale }

    // -- read_stored_code ----------------------------------------------------

    @Test
    fun anUnwrittenPreferenceIsNull() = runBlocking {
        val result = executor().perform(CurrencyOperation.ReadStoredCode)
        assertEquals(CurrencyShellResult.StoredCode(null), result)
    }

    @Test
    fun aStoredPreferenceComesBackVerbatim() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.DISPLAY_CURRENCY to "JPY"))
        assertEquals(
            CurrencyShellResult.StoredCode("JPY"),
            executor(store).perform(CurrencyOperation.ReadStoredCode),
        )
    }

    @Test
    fun anUnreadableStoreReadsAsNeverChose() = runBlocking {
        // Not as a default code. "USD" here would tell the core a choice was
        // made and the region seed would never run — a person in Tokyo would
        // be shown dollars because their preferences file was damaged.
        val store = FakeStore(mapOf(KeyValueStore.Keys.DISPLAY_CURRENCY to "JPY"))
        store.refuseReads = true
        assertEquals(
            CurrencyShellResult.StoredCode(null),
            executor(store).perform(CurrencyOperation.ReadStoredCode),
        )
    }

    // -- write_stored_code ---------------------------------------------------

    @Test
    fun aChoiceIsStoredUnderTheSharedKey() = runBlocking {
        val store = FakeStore()
        val result = executor(store).perform(CurrencyOperation.WriteStoredCode("EUR"))
        assertEquals(CurrencyShellResult.CodeWritten, result)
        // The key name is the cross-client contract, not an implementation
        // detail: the web and desktop clients read this exact one.
        assertEquals("EUR", store.values["vela.displayCurrency"])
    }

    @Test
    fun aRefusedWriteStillAnswers() = runBlocking {
        val store = FakeStore()
        store.refuseWrites = true
        // The core is not waiting to hear whether the disk was willing; it is
        // waiting to be unblocked. An unanswered effect is a sheet that never
        // closes, with no error anywhere.
        assertEquals(
            CurrencyShellResult.CodeWritten,
            executor(store).perform(CurrencyOperation.WriteStoredCode("EUR")),
        )
        assertTrue(store.values.isEmpty())
    }

    // -- read_device_currency ------------------------------------------------

    @Test
    fun aRegionGivesItsCurrency() = runBlocking {
        assertEquals(
            CurrencyShellResult.DeviceCurrency("JPY"),
            executor(locale = Locale.JAPAN).perform(CurrencyOperation.ReadDeviceCurrency),
        )
    }

    @Test
    fun aRegionlessLocaleHasNoCurrency() = runBlocking {
        // `Locale("en")` throws inside Currency.getInstance. The answer is
        // "no region", not a crash and not a guess at USD.
        val result = executor(locale = Locale.ENGLISH)
            .perform(CurrencyOperation.ReadDeviceCurrency)
        assertEquals(CurrencyShellResult.DeviceCurrency(null), result)
    }

    @Test
    fun noLocaleAtAllHasNoCurrency() = runBlocking {
        assertEquals(
            CurrencyShellResult.DeviceCurrency(null),
            executor(locale = null).perform(CurrencyOperation.ReadDeviceCurrency),
        )
    }

    @Test
    fun theWebShellsNullIsNotCopiedHere() = runBlocking {
        // The one place this shell is deliberately MORE capable than the web
        // one it was ported from. If someone "aligns" the two by returning
        // null, this fails — which is the intent.
        val result = executor(locale = Locale.GERMANY)
            .perform(CurrencyOperation.ReadDeviceCurrency)
        assertEquals(CurrencyShellResult.DeviceCurrency("EUR"), result)
    }

    // -- resolve_rate --------------------------------------------------------

    @Test
    fun anUnpricedRateIsNullAndNotOne() = runBlocking {
        val result = executor().perform(CurrencyOperation.ResolveRate("GBP"))
        val resolved = result as CurrencyShellResult.RateResolved
        assertEquals("GBP", resolved.code)
        // `null` and `1.0` are different answers and the core splits on the
        // difference: formatting may degrade to the USD figure, converting may
        // not. A defaulted 1 is a mispayment, not a rounding error.
        assertNull(resolved.rate)
    }

    // -- the net -------------------------------------------------------------

    @Test
    fun everyOperationHasANeutralAnswer() {
        // Exhaustiveness is the compiler's job here — `neutralAnswer` is a
        // `when` over a sealed hierarchy with no `else`, so an operation added
        // in Rust and transcribed here cannot be forgotten. This checks the
        // shapes are the ones the core will accept.
        val net = executor()
        assertEquals(
            CurrencyShellResult.StoredCode(null),
            net.neutralAnswer(CurrencyOperation.ReadStoredCode),
        )
        assertEquals(
            CurrencyShellResult.CodeWritten,
            net.neutralAnswer(CurrencyOperation.WriteStoredCode("EUR")),
        )
        assertEquals(
            CurrencyShellResult.DeviceCurrency(null),
            net.neutralAnswer(CurrencyOperation.ReadDeviceCurrency),
        )
        assertEquals(
            CurrencyShellResult.RateResolved("EUR", null),
            net.neutralAnswer(CurrencyOperation.ResolveRate("EUR")),
        )
    }
}
