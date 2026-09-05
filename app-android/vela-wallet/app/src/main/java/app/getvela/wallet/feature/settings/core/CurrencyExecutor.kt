package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import java.util.Currency
import java.util.Locale

/**
 * The only place the `display_currency` core touches the outside world.
 *
 * Four operations, no decisions. Whether a device's currency becomes the
 * display currency, whether a stored choice outranks a seed, what an unpriced
 * currency does to a balance — all of that is
 * `rust/crates/vela-core/src/app/display_currency.rs`. This class reads a
 * preference, writes a preference, asks the platform what region it is in, and
 * reports what it observed.
 *
 * Port source: `app-web/vela-wallet/src/lib/settings/core/currency-executor.ts`,
 * with **one deliberate difference**, and it is the first place the Android
 * shell is more capable than the web one rather than a translation of it —
 * see [readDeviceCurrency].
 */
class CurrencyExecutor(
    private val store: KeyValueStore,
    /**
     * The device's primary locale.
     *
     * A parameter, not a call to `Locale.getDefault()`, because "what does a
     * regionless locale answer" is a question this class must be tested on and
     * the JVM's default locale is not a thing a test should mutate.
     */
    private val primaryLocale: () -> Locale?,
) {

    suspend fun perform(operation: CurrencyOperation): CurrencyShellResult = when (operation) {
        // Absent ALWAYS means "the user never chose" — never a default code.
        // Returning "USD" here would tell the core a choice was made, and the
        // region seed would never run.
        is CurrencyOperation.ReadStoredCode ->
            CurrencyShellResult.StoredCode(store.read(KeyValueStore.Keys.DISPLAY_CURRENCY))

        // Best effort by contract: a storage failure still answers, because a
        // machine waiting on an unanswered effect is a settings sheet that
        // never closes.
        is CurrencyOperation.WriteStoredCode -> {
            store.write(KeyValueStore.Keys.DISPLAY_CURRENCY, operation.code)
            CurrencyShellResult.CodeWritten
        }

        is CurrencyOperation.ReadDeviceCurrency ->
            CurrencyShellResult.DeviceCurrency(readDeviceCurrency())

        // live in 041 — pricing needs the network layer that spec brings.
        // `null`, NOT `1`: the core splits on the difference, and a fiat amount
        // multiplied by a defaulted 1 is a real mispayment rather than a
        // cosmetic one.
        is CurrencyOperation.ResolveRate ->
            CurrencyShellResult.RateResolved(operation.code, rate = null)
    }

    /**
     * The device region's currency, or `null`.
     *
     * The web shell answers `null` here because a browser has no region. A
     * phone does, and a person in Japan whose wallet opens in dollars has been
     * told something wrong about their own money — so this asks.
     *
     * Two ways it legitimately has no answer, both `null` rather than a guess:
     * a locale with no region (`en`, `zh`) makes `Currency.getInstance` throw,
     * and a region with no ISO-4217 currency has none to give. The core's guard
     * is three ASCII uppercase letters; anything else is not a currency code
     * and is not offered as one.
     */
    private fun readDeviceCurrency(): String? {
        val locale = primaryLocale() ?: return null
        val code = runCatching { Currency.getInstance(locale)?.currencyCode }.getOrNull()
        return code?.takeIf { it.matches(CURRENCY_CODE) }
    }

    /**
     * What to answer when the shell itself threw — a shell bug, since nothing
     * above can fail. It still has to answer: an unanswered effect is a
     * spinner with no end and no error anywhere.
     */
    fun neutralAnswer(operation: CurrencyOperation): CurrencyShellResult = when (operation) {
        is CurrencyOperation.ReadStoredCode -> CurrencyShellResult.StoredCode(null)
        is CurrencyOperation.WriteStoredCode -> CurrencyShellResult.CodeWritten
        is CurrencyOperation.ReadDeviceCurrency -> CurrencyShellResult.DeviceCurrency(null)
        is CurrencyOperation.ResolveRate ->
            CurrencyShellResult.RateResolved(operation.code, rate = null)
    }

    private companion object {
        /** The core's own guard, at `display_currency.rs:390`. */
        val CURRENCY_CODE = Regex("^[A-Z]{3}$")
    }
}
