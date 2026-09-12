package app.getvela.wallet.feature.signing.components

import androidx.compose.animation.core.animateFloatAsState
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.components.ExploreMetrics
import kotlin.math.roundToInt

/**
 * The one way to confirm a signature (spec 022 §4, product contract).
 *
 * There is no reject button beside it: dismissing the sheet IS the rejection,
 * so the only deliberate act on this screen is the affirmative one. The gesture
 * asks for 88% of the track — far more than a mis-tap, far less than a fight —
 * and TalkBack gets the same power through a click action, because a
 * confirmation only a thumb can perform is one some people could never give.
 */
@Composable
fun SlideToConfirm(
    hint: String,
    action: String,
    enabled: Boolean,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
) {
    // Spec 048: the threshold is a detent the finger feels.
    val slideHaptic = rememberVelaHaptic()
    val colors = VelaTheme.colors
    val label = "$hint · $action"
    var progress by remember { mutableFloatStateOf(0f) }
    var dragging by remember { mutableStateOf(false) }
    var done by remember { mutableStateOf(false) }
    val settled by animateFloatAsState(progress, label = "slideProgress")
    val shown = if (dragging) progress else settled

    BoxWithConstraints(
        modifier = modifier
            .fillMaxWidth()
            .height(ExploreMetrics.slideTrack)
            .background(colors.bgSunken, CircleShape)
            .alpha(if (enabled) 1f else VelaOpacity.disabled)
            .semantics {
                contentDescription = label
                role = Role.Button
                onClick(label) {
                    if (enabled && !done) {
                        progress = 1f
                        done = true
                        slideHaptic(VelaHaptic.Detent)
                        onConfirm()
                    }
                    true
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        val density = LocalDensity.current
        // The knob sits inside a half-gap at each end, so the travel is the
        // track minus the knob minus both insets (the mock's 342 − 48 − 8).
        val insetPx = with(density) { VelaSpacing.sm.toPx() }
        val travelPx = with(density) {
            (maxWidth - ExploreMetrics.slideKnob).toPx() - insetPx * 2
        }

        Box(
            Modifier
                .fillMaxWidth(shown)
                .height(ExploreMetrics.slideTrack)
                .background(colors.accentSoft, CircleShape),
        )
        Text(
            text = label,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            modifier = Modifier
                .align(Alignment.Center)
                .alpha(1f - shown),
        )
        Box(
            modifier = Modifier
                .offset {
                    androidx.compose.ui.unit.IntOffset(
                        (insetPx + shown * travelPx).roundToInt(), 0,
                    )
                }
                .size(ExploreMetrics.slideKnob)
                .background(colors.accentBase, CircleShape)
                .pointerInput(enabled, done) {
                    if (!enabled || done) return@pointerInput
                    detectHorizontalDragGestures(
                        onDragStart = { dragging = true },
                        onDragEnd = {
                            dragging = false
                            if (progress >= ExploreMetrics.SLIDE_COMMIT) {
                                progress = 1f
                                done = true
                                slideHaptic(VelaHaptic.Detent)
                                onConfirm()
                            } else {
                                progress = 0f
                            }
                        },
                        onDragCancel = {
                            dragging = false
                            progress = 0f
                        },
                    ) { _, delta ->
                        progress = (progress + delta / travelPx).coerceIn(0f, 1f)
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            Icon(VelaIcons.ArrowRight, null, tint = VelaOnAccent)
        }
    }
}
