package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.animation.core.withInfiniteAnimationFrameMillis
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.runtime.State
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.marks.RemoteLogo
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing

/**
 * Shared wallet-component metrics, derived strictly from tokens (research D6:
 * where a mock measure has no direct token, platforms derive from existing
 * tokens and record the mapping here).
 */
internal object WalletMetrics {
    /** 40dp header avatar / row leading circle (mock; space.xl4 32 + space.md 8). */
    val avatarSize: Dp = VelaSpacing.xl4 + VelaSpacing.md

    /**
     * 160dp identicon-viewer artwork (spec 019 founder call) — big enough to
     * read as a picture rather than an avatar, and to sit above a wrapped
     * address on the narrowest phone.
     */
    val identiconViewerSize: Dp = VelaSpacing.xl4 * 5


    /** 12dp chain-dot badge on row circles (icon.xs). */
    val badgeDotSize: Dp = VelaIconSize.xs

    /** Badge ring: badge dot + 2dp ring on each side (space.sm 4 total). */
    val badgeRingSize: Dp = badgeDotSize + VelaSpacing.sm

    /**
     * A badge that carries a chain's logo is a size a logo can be read at
     * (078 H-09), the same on all four shells: 16 across (icon.base) with a
     * 1.5 ring (border.emphasis) in the page colour — the web's
     * `.badge.with-logo` and the desktop's, which are 16 wide ring included.
     */
    val badgeLogoRingSize: Dp = VelaIconSize.base

    /** The logo inside that ring, and the dot it falls back to: 16 − 2 × 1.5 = 13. */
    val badgeLogoSize: Dp = badgeLogoRingSize - VelaBorder.emphasis * 2

    /** 14dp pill dots (icon.sm). */
    val pillDotSize: Dp = VelaIconSize.sm

    /** 10dp chain-list dots (space.md 8 + space.xs 2). */
    val listDotSize: Dp = VelaSpacing.md + VelaSpacing.xs

    /** 8dp live indicator dot (space.md). */
    val liveDotSize: Dp = VelaSpacing.md
}

/** Solid chain-color dot; null = the all-networks neutral dot. */
@Composable
internal fun ChainDot(color: Color?, size: Dp, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .size(size)
            .background(color ?: VelaTheme.colors.fgSubtle, CircleShape),
    )
}

/**
 * Bottom-end chain badge over a row circle: the dot sits on a ring of the
 * screen background so it reads as an overlay (mock H1 row icons).
 *
 * With a logo it is the four shells' one badge — 16 with a 1.5 ring, the
 * logo (and the dot while it loads, or when it cannot) filling the inside;
 * it was a 12 logo in a 2 ring here, a quarter smaller than on the web and
 * the desktop. A bare dot (a gallery's, which names no endpoint) keeps the
 * mock's 12 in its 2 ring.
 */
@Composable
internal fun ChainBadge(color: Color, modifier: Modifier = Modifier, logoUrl: String? = null) {
    val ring = if (logoUrl != null) WalletMetrics.badgeLogoRingSize else WalletMetrics.badgeRingSize
    val inside = if (logoUrl != null) WalletMetrics.badgeLogoSize else WalletMetrics.badgeDotSize
    Box(
        modifier = modifier
            .size(ring)
            .background(VelaTheme.colors.bgBase, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        // Spec 047: the chain's logo when the chain-data endpoint has one; the
        // coloured dot stays the fallback.
        RemoteLogo(urls = listOfNotNull(logoUrl), size = inside) {
            ChainDot(color = color, size = inside)
        }
    }
}

/**
 * Shared skeleton/live-dot pulse: opacity.disabled → 1, gentle loop.
 *
 * Applied in the DRAW phase (Phase 4): the value is read inside the layer, so
 * a frame of the pulse redraws one small layer. It used to be read in
 * composition (`alpha(rememberPulseAlpha())`), which recomposed, measured and
 * laid out the whole hero every frame for as long as "Checking…" or "Live"
 * stood on the home — 60 frames a second on an idle home, measured.
 */
@Composable
internal fun Modifier.pulsing(): Modifier {
    val alpha = rememberPulseAlpha()
    return this.graphicsLayer { this.alpha = alpha.value }
}

/**
 * The same linear there-and-back ([VelaMotion.durationSlow] x 2 each way), but
 * stepped [PULSE_FPS] times a second rather than redrawn on every vsync (Phase 4).
 * "Live · listening for payments" stands on a settled home indefinitely; on
 * the frame clock it kept a 120 Hz panel producing a frame every vsync for a
 * dot six pixels wide — the measured idle home rendered continuously, and so
 * did every screen and sheet laid over it. A 0.4 → 1 fade over 800 ms moves
 * ~3% of opacity a step at this rate, which reads as the same smooth pulse.
 */
@Composable
private fun rememberPulseAlpha(): State<Float> {
    val alpha = remember { mutableFloatStateOf(VelaOpacity.disabled) }
    LaunchedEffect(Unit) {
        val sweep = VelaMotion.durationSlow * 2L
        var start = -1L
        var lastStep = -1L
        while (true) {
            // On the frame clock, so a step lands inside a vsync's frame (a
            // timer's invalidation lands between them and the frame reads as
            // late); the frame callbacks in between change nothing and draw
            // nothing. `withInfiniteAnimationFrameMillis`: a test clock knows
            // it never ends, as it knows the transition this replaced.
            withInfiniteAnimationFrameMillis { now ->
                if (start < 0) start = now
                val step = (now - start) / (1_000L / PULSE_FPS)
                if (step != lastStep) {
                    lastStep = step
                    val t = (now - start) % (2 * sweep)
                    val there = if (t < sweep) t.toFloat() / sweep else 2f - t.toFloat() / sweep
                    alpha.floatValue = VelaOpacity.disabled + (1f - VelaOpacity.disabled) * there
                }
            }
        }
    }
    return alpha
}

/** How often the pulse steps (see [rememberPulseAlpha]). */
private const val PULSE_FPS = 24
