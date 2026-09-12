package app.getvela.wallet.feature.contacts.components

import androidx.compose.animation.core.LinearOutSlowInEasing
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntOffset
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import androidx.compose.foundation.layout.offset

/**
 * A–Z index rail (spec vocabulary #5, mobile SPEC sheet): a fixed-width column
 * of letters at the right edge. Touching or sliding maps the finger's y to a
 * letter, jumps the list straight to that section (no smooth scroll — the SPEC
 * says direct positioning), fires one selection haptic per crossed letter and
 * shows a bubble HUD beside the finger (fade-in 120ms / fade-out 80ms,
 * ease-out). With reduce-motion the bubble does not animate and the jump is
 * still direct.
 *
 * The full alphabet always renders; letters with no section resolve to the
 * nearest existing one — that mapping is the caller's (screen) job so this
 * component stays free of business rules.
 */
@Composable
fun AlphaIndexRail(
    letters: List<String>,
    modifier: Modifier = Modifier,
    /** Static bubble for the component board (renders without a gesture). */
    pinnedBubble: String? = null,
    onLetter: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    val reduced = rememberReducedMotion()

    var activeIndex by remember { mutableIntStateOf(-1) }
    var touchY by remember { mutableIntStateOf(0) }
    var touching by remember { mutableStateOf(false) }

    val bubbleLetter = when {
        touching && activeIndex in letters.indices -> letters[activeIndex]
        else -> pinnedBubble
    }
    val visible = bubbleLetter != null
    val bubbleAlpha by animateFloatAsState(
        targetValue = if (visible) 1f else 0f,
        animationSpec = tween(
            durationMillis = when {
                reduced -> 0
                visible -> ContactsMotion.bubbleIn
                else -> ContactsMotion.bubbleOut
            },
            easing = LinearOutSlowInEasing,
        ),
        label = "indexBubbleAlpha",
    )

    Box(modifier = modifier.fillMaxHeight()) {
        Column(
            modifier = Modifier
                .fillMaxHeight()
                .width(ContactsMetrics.indexRailWidth)
                .pointerInput(letters) {
                    awaitEachGesture {
                        fun update(y: Float) {
                            touchY = y.toInt()
                            val height = size.height.toFloat().coerceAtLeast(1f)
                            val index = ((y / height) * letters.size)
                                .toInt()
                                .coerceIn(0, letters.lastIndex)
                            if (index != activeIndex) {
                                activeIndex = index
                                // One selection tick per crossed letter (SPEC).
                                haptic(VelaHaptic.Detent)
                                onLetter(letters[index])
                            }
                        }

                        val down = awaitFirstDown(requireUnconsumed = false)
                        down.consume()
                        touching = true
                        activeIndex = -1
                        update(down.position.y)
                        var pressed = true
                        while (pressed) {
                            val event = awaitPointerEvent()
                            event.changes.forEach { change ->
                                if (change.pressed) {
                                    update(change.position.y)
                                    change.consume()
                                }
                            }
                            pressed = event.changes.any { it.pressed }
                        }
                        touching = false
                    }
                },
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            letters.forEachIndexed { index, letter ->
                Box(
                    modifier = Modifier.weight(1f),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = letter,
                        color = if (touching && index == activeIndex) {
                            colors.accentBase
                        } else {
                            colors.fgSubtle
                        },
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.medium,
                        fontSize = VelaTextSize.xs,
                        maxLines = 1,
                    )
                }
            }
        }

        if (bubbleLetter != null || bubbleAlpha > 0f) {
            val label = bubbleLetter ?: letters.firstOrNull().orEmpty()
            Box(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .offset {
                        IntOffset(
                            x = -(ContactsMetrics.indexRailWidth.roundToPx() +
                                ContactsMetrics.bubbleSize.roundToPx()),
                            y = (touchY - ContactsMetrics.bubbleSize.roundToPx() / 2)
                                .coerceAtLeast(0),
                        )
                    }
                    .alpha(bubbleAlpha)
                    .size(ContactsMetrics.bubbleSize)
                    .background(colors.bgRaised, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = label,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl2,
                    maxLines = 1,
                )
            }
        }
    }
}
