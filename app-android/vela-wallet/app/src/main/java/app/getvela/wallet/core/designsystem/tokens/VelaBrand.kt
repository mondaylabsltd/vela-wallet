package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.ui.graphics.Color

/**
 * Brand constants for the in-app mark (docs/design/onboarding/logo-{dark,light}.svg and
 * the design-system brief's brand-asset rules). The sails are identical in both
 * modes; only the hull is themed. These are brand values, not UI tokens — they do
 * not appear in the DTCG export.
 */
object VelaBrand {
    /** Proper noun — rendered verbatim, never translated (spec FR-003). */
    const val WORDMARK: String = "Vela Wallet"

    val sailMain: Color = Color(0xFFFF6A45)
    val sailSoft: Color = Color(0xFFFFA98E)

    /** Hull: Dusk Ivory on dark UI, Warm Graphite on light UI. */
    val hullOnDark: Color = Color(0xFFDED5CE)
    val hullOnLight: Color = Color(0xFF554B46)
}
