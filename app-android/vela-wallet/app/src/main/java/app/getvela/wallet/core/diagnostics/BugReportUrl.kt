package app.getvela.wallet.core.diagnostics

import java.net.URLEncoder

/**
 * The prefilled GitHub issue form (spec 081 FR-016).
 *
 * ## Why this file exists at all
 *
 * Three places built this URL by hand and all three built it wrong, in the
 * same way: `?template=bug.yml&body=…`.
 *
 * **With `template=bug.yml`, GitHub ignores `body` entirely.** An issue FORM
 * (as opposed to a plain markdown template) is prefilled by its own field
 * ids — `what`, `steps`, `environment`, `area`, exactly as
 * `.github/ISSUE_TEMPLATE/bug.yml` declares them — and a `body` parameter is
 * dropped without a word. So the settings 发送 button and the crash sheet's
 * report button both opened a completely EMPTY form, having just told the
 * person their details were attached. Everything they had typed, and every
 * line of the stack trace, was gone at the moment they were promised it was
 * being sent.
 *
 * ## What may be in one
 *
 * The same rule the whole feature runs on: version, platform, language, the
 * NAMES of unreachable networks and recent failure classes. Never an address,
 * a balance, an endpoint or RPC URL (they carry API keys), or a raw `vela.*`
 * value. Nothing here reads storage, which is what makes that true rather
 * than merely intended.
 */
object BugReportUrl {

    private const val FORM = "https://github.com/mondaylabsltd/vela-wallet/issues/new"

    /**
     * The `area` dropdown's option, spelled exactly as the form spells it — a
     * value that does not match an option leaves the dropdown unset, which is
     * the same as not prefilling it at all.
     */
    const val AREA_OTHER: String = "Other (explain above)"

    private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8")

    /**
     * Build it.
     *
     * [steps] and [area] are optional only in the sense that an empty one is
     * left out; the form marks steps required, and an empty box the person can
     * see the cursor in beats a missing one.
     */
    fun build(
        what: String,
        steps: String = "",
        environment: String = "",
        area: String = AREA_OTHER,
        titlePrefix: String = "[android] ",
    ): String {
        val title = (titlePrefix + what.lineSequence().firstOrNull().orEmpty()).take(90)
        val fields = buildList {
            add("template" to "bug.yml")
            add("title" to title)
            add("what" to what)
            if (steps.isNotBlank()) add("steps" to steps)
            if (environment.isNotBlank()) add("environment" to environment)
            if (area.isNotBlank()) add("area" to area)
        }
        return FORM + "?" + fields.joinToString("&") { (key, value) -> "$key=${encode(value)}" }
    }
}
