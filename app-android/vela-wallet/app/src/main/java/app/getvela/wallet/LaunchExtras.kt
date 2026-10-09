package app.getvela.wallet

import app.getvela.wallet.navigation.VelaDestinations

/**
 * Which launch extras a build honours (spec 088 FR-003).
 *
 * `MainActivity` is exported — it is the launcher and the `velawallet://`
 * handler — so ANY installed app can start it with extras. The device-pass
 * extras below open fixture galleries full of invented balances and
 * transactions, pin settings states, skip straight to a route, load a page
 * with no question asked, or knock on the parallel space's door. They are a
 * debug build's tools; a release build reads none of them and starts where a
 * person would: Welcome, which the session then routes.
 *
 * Pure, so the rule is a JVM test rather than a hope ([LaunchExtrasTest]).
 * `vela.receipt` (the notification's own door) and the deep-link data are not
 * here: those are the app's real entry points.
 */
object LaunchExtras {
    /** Read by debug builds only. */
    val DEBUG_ONLY: Set<String> = setOf(
        "vela.startDestination",
        "vela.flowState",
        "vela.signingState",
        "vela.settingsState",
        "vela.settingsDark",
        "vela.gallery",
        "vela.openUrl",
        "vela.parallelSpace",
    )

    /** `value` when this build may act on the extra `name`, else `null`. */
    fun <T> honoured(name: String, value: T?, debug: Boolean = BuildConfig.DEBUG): T? =
        if (debug || name !in DEBUG_ONLY) value else null

    /** The first route: the extra's, in a debug build and only when it names one; Welcome otherwise. */
    fun startDestination(requested: String?, debug: Boolean = BuildConfig.DEBUG): String =
        honoured("vela.startDestination", requested, debug)?.takeIf { it in VelaDestinations.ALL }
            ?: VelaDestinations.WELCOME
}
