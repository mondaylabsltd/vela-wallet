package app.getvela.wallet.core.designsystem.components

import androidx.compose.animation.core.animateFloatAsState
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/**
 * Vela CTA buttons: pill shape (DV-002), 52dp control height, spring press-scale
 * (design-system interactive-feedback rule) on top of the Material ripple.
 * Disabled (spec 014, mock A1) = the whole surface + label at
 * [VelaOpacity.disabled] — dimmed accent, never a gray fill.
 *
 * [loading] is the third state, and it is NOT disabled: the action is running
 * and this button is what the person is waiting on. It keeps full emphasis and
 * turns a spinner where its label was (docs/design-system.md — "Loading state:
 * ActivityIndicator replacing text"), because a dimmed button reads as
 * "unavailable", which is the one thing "working" must never look like.
 */
@Composable
fun VelaPrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
) {
    VelaButtonSurface(
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        loading = loading,
    ) { pressModifier ->
        Box(
            modifier = pressModifier.background(VelaTheme.colors.accentBase),
            contentAlignment = Alignment.Center,
        ) {
            ButtonLabel(text = text, color = VelaOnAccent, loading = loading)
            ButtonSpinner(color = VelaOnAccent, visible = loading)
        }
    }
}

@Composable
fun VelaSecondaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
) {
    VelaButtonSurface(
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        loading = loading,
    ) { pressModifier ->
        Box(
            modifier = pressModifier.border(
                width = VelaBorder.hairline,
                color = VelaTheme.colors.borderStrong,
                shape = RoundedCornerShape(VelaRadius.full),
            ),
            contentAlignment = Alignment.Center,
        ) {
            // Label uses fg.base, not the mock's low-contrast gray (spec DV-001).
            ButtonLabel(text = text, color = VelaTheme.colors.fgBase, loading = loading)
            ButtonSpinner(color = VelaTheme.colors.fgBase, visible = loading)
        }
    }
}

/**
 * The destructive CTA (spec 023): 退出登录 / 仍然退出 / 全部清除 are filled
 * buttons in the error colour, not accent ones. Accent is reserved for the
 * action that moves value (design review 2026-07), and signing out or erasing
 * a device moves none — it destroys.
 */
@Composable
fun VelaDangerButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
) {
    VelaButtonSurface(
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        loading = loading,
    ) { pressModifier ->
        Box(
            modifier = pressModifier.background(VelaTheme.colors.errorBase),
            contentAlignment = Alignment.Center,
        ) {
            ButtonLabel(text = text, color = VelaOnAccent, loading = loading)
            ButtonSpinner(color = VelaOnAccent, visible = loading)
        }
    }
}

/** Shared press-feedback surface; internal so VelaActionStack rows reuse it. */
@Composable
internal fun VelaButtonSurface(
    onClick: () -> Unit,
    modifier: Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
    content: @Composable (Modifier) -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val haptic = rememberVelaHaptic()
    val pressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed) VelaMotion.pressScaleButton else 1f,
        animationSpec = VelaMotion.pressSpring,
        label = "buttonPressScale",
    )
    content(
        modifier
            .fillMaxWidth()
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
                // Dimming follows `enabled` alone: a busy button is at full
                // strength, it just cannot be pressed again.
                alpha = if (enabled) 1f else VelaOpacity.disabled
            }
            .heightIn(min = VelaSizing.controlLg)
            .clip(RoundedCornerShape(VelaRadius.full))
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                enabled = enabled && !loading,
                // The press is answered in the same instant it lands, even when
                // what it started (a system passkey sheet, a network call) is
                // several hundred ms from showing itself.
                onClick = {
                    haptic(VelaHaptic.Press)
                    onClick()
                },
            ),
    )
}

/** The busy spinner, sized and coloured to sit where the label was. */
@Composable
private fun ButtonSpinner(color: androidx.compose.ui.graphics.Color, visible: Boolean) {
    if (!visible) return
    CircularProgressIndicator(
        color = color,
        strokeWidth = VelaBorder.emphasis,
        modifier = Modifier.size(VelaIconSize.lg),
    )
}

@Composable
internal fun ButtonLabel(
    text: String,
    color: androidx.compose.ui.graphics.Color,
    loading: Boolean = false,
) {
    Text(
        text = text,
        color = color,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.lg,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            // Vertical padding keeps wrapped long-locale labels off the pill
            // edges; the surface's heightIn(min) grows with them (spec 014).
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md)
            // Hidden rather than removed while busy: the label goes on holding
            // the button's height, so the spinner's arrival reflows nothing.
            .alpha(if (loading) 0f else 1f),
    )
}
