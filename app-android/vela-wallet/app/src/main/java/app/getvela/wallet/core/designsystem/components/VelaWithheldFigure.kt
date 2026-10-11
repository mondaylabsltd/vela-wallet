package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing

/**
 * The room a fiat figure keeps while it is WITHHELD — the display currency is
 * not the person's yet (the core's rule, `app::display_currency`: no fiat
 * figure before `CurrencyView.committed`, on any surface, and withholding
 * never moves the layout).
 *
 * It goes on the very `Text` that will draw the figure, whose text is empty
 * meanwhile. So the line is the figure's own line — same face, same size,
 * same line height, measured by the same text stack — and when the figure
 * lands it lands in place: nothing above or below it moves, by construction
 * rather than by a placeholder box whose height was matched by hand (the
 * holdings' first one was 22 dp standing in for an 18 dp line).
 *
 * What it draws in that room is a quiet pill, the same one a holding's worth
 * waits as — so a waiting figure reads as "on its way" on every surface, not
 * as "there is none". Nothing is announced: there is no figure to read yet.
 */
fun Modifier.withheldFigure(withheld: Boolean, color: Color): Modifier {
    if (!withheld) return this
    return this
        .widthIn(min = WITHHELD_WIDTH)
        .drawBehind {
            val height = minOf(size.height * WITHHELD_FILL, WITHHELD_MAX_HEIGHT.toPx())
            drawRoundRect(
                color = color,
                topLeft = Offset(0f, (size.height - height) / 2f),
                size = Size(size.width, height),
                cornerRadius = CornerRadius(height / 2f),
            )
        }
        .testTag(WITHHELD_FIGURE_TAG)
        .clearAndSetSemantics { }
}

/**
 * Room that, once kept, stays kept — for a line that will GROW when a
 * withheld figure joins it: a fee's "0.000123 ETH" becoming "0.000123 ETH ·
 * ≈CN¥2.24", which no longer fits beside its label and takes a second line.
 * How wide the figure will be is not knowable while it is withheld (the rate
 * is what is on its way), so the row takes the taller arrangement from the
 * first frame and keeps it for as long as the screen lives: the figure then
 * lands in room that was already there, and a short one that would have fit
 * after all does not pull the row back up under the person's finger.
 *
 * A plain holder, not state (as the signing sheet's `HeldLine`): reading it
 * schedules no recomposition, and the same inputs always give the same room.
 */
class KeptRoom {
    private var kept = false

    /** Keep the room [now] if asked; `true` from then on. */
    fun keep(now: Boolean): Boolean {
        kept = kept || now
        return kept
    }
}

/** A withheld figure's test tag: a board's measurement finds the line by it. */
const val WITHHELD_FIGURE_TAG: String = "fiat-withheld"

private val WITHHELD_WIDTH = VelaSpacing.xl5
private val WITHHELD_MAX_HEIGHT = VelaSpacing.lg + VelaSpacing.xs
private const val WITHHELD_FILL = 0.6f
