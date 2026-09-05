package app.getvela.wallet.feature.settings.core

import android.content.Context
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.DisplayCurrencyCore

/**
 * The settings surface's machines, app-resident.
 *
 * One per process and created on first use, not at launch (research D5): a
 * person who never opens Settings should not pay for its boot, and a machine
 * rebuilt per screen would re-read storage on every rotation and flash the USD
 * placeholder over a settled choice.
 *
 * What a screen sees is a `StateFlow` of a core view and a handful of named
 * intents. It never sees a host, a driver, a `JSONObject` or an operation —
 * that separation is what lets spec 041 add six more machines here without any
 * screen learning about it.
 */
class SettingsController(context: Context, scope: CoroutineScope) {

    private val store = VelaStore(context)

    private val currencyExecutor = CurrencyExecutor(store) { primaryLocale(context) }

    private val currencyHost = CoreHost(
        bridge = DisplayCurrencyCore().asBridge(),
        scope = scope,
        // What the screen renders before the core has read anything: the same
        // USD placeholder the core itself starts from, uncommitted, unpriced.
        initial = CurrencyView(code = "USD", rate = null, committed = false),
        serializer = CurrencyView.serializer(),
        perform = JsonShell.perform(
            CurrencyOperation.serializer(),
            CurrencyShellResult.serializer(),
            currencyExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            CurrencyOperation.serializer(),
            CurrencyShellResult.serializer(),
            fallback = CurrencyShellResult.StoredCode(null),
            answer = currencyExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("settings.currency.fault", "core fault", error) },
    )

    /** The display currency the core has settled on. */
    val currency: StateFlow<CurrencyView> = currencyHost.view

    /**
     * Re-read the preference. Cheap by design — a second `refresh` while one is
     * in flight supersedes it, so a screen may call this on every entry.
     */
    fun refreshCurrency() =
        currencyHost.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())

    /** An explicit pick in the currency sheet. User choice wins over any seed. */
    fun chooseCurrency(code: String) =
        currencyHost.dispatch(CurrencyEvent.UserChose(code), CurrencyEvent.serializer())

    private companion object {
        /**
         * The device's primary locale, for the region seed.
         *
         * The FIRST of the configured locales, not the JVM default: a person
         * with `[ja-JP, en-US]` set is telling the system what they prefer, and
         * `Locale.getDefault()` on Android can be either depending on what the
         * app declared support for.
         */
        fun primaryLocale(context: Context): Locale? =
            context.resources.configuration.locales.takeIf { it.size() > 0 }?.get(0)
    }
}
