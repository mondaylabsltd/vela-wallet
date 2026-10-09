package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSizing

/**
 * Outcome status badge variants (spec 014 data-model §3) — soft tinted disc +
 * base-colored glyph, 56dp ([VelaSizing.emptyStateCircle]).
 */
enum class BadgeVariant {
    /** Green ✓ — A11, B5. */
    Success,

    /** Amber ! — A12, A13, E8. */
    Warning,

    /** Dark disc ! — E4, E5, B6. */
    Neutral,

    /**
     * Red ! — E1, E2, E6, E7, E9, E10, B3, B4. Never ×: the sheet's column is
     * leading-aligned, so a × in a disc sat top-left where a close button
     * goes, and it closed nothing (issue #460). Only the colour differs from
     * [Warning].
     */
    Error,

    /** Amber clock — E3. */
    Timeout,

    /** Blue ! — B2. */
    Info,
}

/** The single status-badge authority (decorative; outcomes carry the headline). */
@Composable
fun VelaStatusBadge(
    variant: BadgeVariant,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    val disc: Color
    val glyphTint: Color
    when (variant) {
        BadgeVariant.Success -> {
            disc = colors.successSoft
            glyphTint = colors.successBase
        }
        BadgeVariant.Warning -> {
            disc = colors.warningSoft
            glyphTint = colors.warningBase
        }
        BadgeVariant.Neutral -> {
            disc = colors.bgSunken
            glyphTint = colors.fgBase
        }
        BadgeVariant.Error -> {
            disc = colors.errorSoft
            glyphTint = colors.errorBase
        }
        BadgeVariant.Timeout -> {
            disc = colors.warningSoft
            glyphTint = colors.warningBase
        }
        BadgeVariant.Info -> {
            disc = colors.infoSoft
            glyphTint = colors.infoBase
        }
    }
    val glyph = badgeGlyph(variant)
    Box(
        modifier = modifier
            .size(VelaSizing.emptyStateCircle)
            .clip(CircleShape)
            .background(disc),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = glyph,
            contentDescription = null,
            tint = glyphTint,
            modifier = Modifier.size(VelaIconSize.xl),
        )
    }
}

/**
 * The badge's glyph: ✓ for success, a clock for a timeout, and ! for every
 * other outcome, error included — the close glyph is for things that close.
 */
fun badgeGlyph(variant: BadgeVariant): ImageVector = when (variant) {
    BadgeVariant.Success -> VelaIcons.Check
    BadgeVariant.Timeout -> VelaIcons.Clock
    BadgeVariant.Warning, BadgeVariant.Neutral, BadgeVariant.Error, BadgeVariant.Info -> VelaIcons.Exclamation
}
