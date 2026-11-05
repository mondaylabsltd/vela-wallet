package app.getvela.wallet.feature.scan

/** What the scanner's "Grant permission" does. */
enum class GrantAction {
    /** Show the system's camera dialog. */
    Ask,

    /** Open this app's page in Settings: the dialog will not be shown again. */
    OpenSettings,
}

/**
 * The scanner's way back from a refused camera (NATIVE-LAUNCH-CHECKLIST C).
 *
 * Android says "don't ask again" only indirectly: once the app has asked and
 * been refused, `shouldShowRequestPermissionRationale` is `true` while asking
 * again would still show the dialog, and `false` once it would not (a second
 * refusal, or "Don't allow" with "don't ask again"). Before any ask it is
 * `false` too — which is why [askedBefore] matters; the scanner asks once on
 * entry, so by the time the button is there it has.
 */
object CameraRefusal {
    fun grantAction(askedBefore: Boolean, showRationale: Boolean): GrantAction =
        if (askedBefore && !showRationale) GrantAction.OpenSettings else GrantAction.Ask
}
