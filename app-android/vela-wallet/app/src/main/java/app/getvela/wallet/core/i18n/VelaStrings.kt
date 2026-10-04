package app.getvela.wallet.core.i18n

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Translation access for UI code. The only implementation shipped in the app is
 * [I18nRuntime] (vela-core engine); previews substitute a sample-copy fake.
 */
interface VelaStrings {
    fun t(key: String): String

    fun t(key: String, vars: Map<String, String>): String

    /**
     * A PLURAL key: [count] chooses the form — `_one`, `_few`, `_many` or
     * `_other`, by the language's CLDR rule, in the core — and fills
     * `{{count}}`.
     *
     * The `vars` overload cannot stand in for this: it fills `{{count}}` as
     * text and never selects a form, so a plural key handed to it has no value
     * (issue #409). Never pick the suffix here either — `count == 1` is not
     * Russian's rule (2–4 is `_few`), nor Chinese's (one form for every count).
     */
    fun t(key: String, count: Int): String
}

val LocalVelaStrings = staticCompositionLocalOf<VelaStrings> {
    error("VelaStrings not provided — wrap content in CompositionLocalProvider(LocalVelaStrings provides …)")
}
