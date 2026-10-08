package app.getvela.wallet.feature.wallet.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
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

/** Shared skeleton/live-dot pulse: opacity.disabled → 1, gentle loop. */
@Composable
internal fun rememberPulseAlpha(): Float {
    val transition = rememberInfiniteTransition(label = "walletPulse")
    val alpha = transition.animateFloat(
        initialValue = VelaOpacity.disabled,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(
                durationMillis = VelaMotion.durationSlow * 2,
                easing = LinearEasing,
            ),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "walletPulseAlpha",
    )
    return alpha.value
}
