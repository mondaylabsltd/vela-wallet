package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** core.space — 4dp base grid; never invent intermediate gaps. */
object VelaSpacing {
    val none: Dp = 0.dp
    val xs: Dp = 2.dp
    val sm: Dp = 4.dp
    val md: Dp = 8.dp
    val lg: Dp = 12.dp
    val xl: Dp = 16.dp
    val xl2: Dp = 20.dp
    val xl3: Dp = 24.dp
    val xl4: Dp = 32.dp
    val xl5: Dp = 48.dp
    val xl6: Dp = 64.dp
}

/** core.radius. */
object VelaRadius {
    val none: Dp = 0.dp
    val sm: Dp = 4.dp
    val md: Dp = 8.dp
    val lg: Dp = 12.dp
    val xl: Dp = 16.dp
    val xl2: Dp = 20.dp

    /** radius.full = 9999 — pill/capsule. */
    val full: Dp = 9999.dp
}

/** core.border widths. */
object VelaBorder {
    val hairline: Dp = 1.dp
    val emphasis: Dp = 1.5.dp
}

/** core.size + core.layout (phone-relevant subset) + control heights. */
object VelaSizing {
    val hitTarget: Dp = 44.dp
    val hitSlop: Dp = 8.dp
    val emptyStateCircle: Dp = 56.dp
    val screenPaddingX: Dp = 24.dp

    /** The v2 welcome's brand mark — the web and the desktop draw it at 60. */
    val brandMark: Dp = 60.dp

    /** The done card's identicon, and the disc its outcome tick sits in. */
    val doneAvatar: Dp = 44.dp
    val doneCheck: Dp = 34.dp
    val scanFabSize: Dp = 56.dp

    /**
     * The mobile tab bar (spec 078 round 2): icons only — the labels
     * truncated in es/pt/de/it ("Configuración", "Einstellungen") — so the
     * glyph is larger than the old icon-over-label pair's, centred in a bar
     * this tall (without the system inset). The labels live on as each tab's
     * accessible name.
     */
    val tabIcon: Dp = 28.dp
    val tabBar: Dp = 56.dp

    // sizing.control.* comes from the design-system brief (not present in the DTCG
    // export — do not add to the drift test until the export grows it).
    val controlSm: Dp = 36.dp
    val controlMd: Dp = 44.dp
    val controlLg: Dp = 52.dp

    // Spec 014 onboarding-flow geometry (mock-measured; not in the DTCG export —
    // kept outside the drift test like sizing.control.* above).
    /** Stepped/single progress bar thickness (A4–A8, B1). */
    val progressBar: Dp = 4.dp

    /** Elapsed-seconds ring diameter — fits 1- and 2-digit values (A4c/B1c). */
    val elapsedRing: Dp = 40.dp

    /** Elapsed-seconds ring stroke width. */
    val elapsedRingStroke: Dp = 3.dp

    /** Acknowledgment checkbox square (A1–A3). */
    val checkboxBox: Dp = 20.dp

    /** Flow sheet drag-handle bar (token-tinted custom handle, research D2). */
    val sheetHandleWidth: Dp = 36.dp
    val sheetHandleHeight: Dp = 4.dp

    // Spec 021 wallet-flow geometry, measured off docs/design/wallet-2 (not in the
    // DTCG export — kept outside the drift test like the two blocks above).
    /**
     * The receive QR card, measured 344x344 in R2. Fixed, NOT fluid: the SPEC
     * sheet pins it at 1.35x text scale too, because a code that shrinks with
     * its caption stops scanning.
     */
    val qrCard: Dp = 344.dp

    /** The send-receipt status disc, measured 88 in SD4a/SD4c. */
    val statusHero: Dp = 88.dp

    /** The receive network-row chain badge, measured 40 in R1. */
    val chainBadge: Dp = 40.dp
}

/** core.icon sizes. */
object VelaIconSize {
    val xs: Dp = 12.dp
    val sm: Dp = 14.dp
    val base: Dp = 16.dp
    val md: Dp = 18.dp
    val lg: Dp = 20.dp
    val xl: Dp = 26.dp
    val xl2: Dp = 30.dp
    val xl3: Dp = 36.dp
}

/** core.opacity. */
object VelaOpacity {
    const val disabled: Float = 0.45f
    const val dim: Float = 0.4f
    const val backdrop: Float = 0.35f
}
