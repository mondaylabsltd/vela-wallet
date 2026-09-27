package app.getvela.wallet.core.designsystem.components

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.IntrinsicMeasurable
import androidx.compose.ui.layout.IntrinsicMeasureScope
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.layout.MeasureResult
import androidx.compose.ui.layout.MeasureScope
import androidx.compose.ui.layout.MultiContentMeasurePolicy
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing

/**
 * A label and its value on one line — the label at the start, the value at the
 * end — when both fit whole; otherwise the value takes a line of its own under
 * the label. Neither is ever cut and the label never breaks inside a word to
 * make room: it takes its whole line first, and wraps only when it alone is
 * wider than the row.
 *
 * One rule for every "name … value" row that meets a large text size (spec 078
 * rounds 2–3): the fee card, the speed list, the settings rows. They used to
 * give the value first claim, so at the largest size "Network fee" ran into the
 * coin mark and settings titles ended in "Idi…" / "Formato de n…".
 *
 * [stackedValue] says where the value sits once it has its own line: at the
 * end (a money figure, which reads down a right edge) or at the start (a
 * setting's current value, read as the second line of its row).
 */
@Composable
fun VelaLabelBesideValue(
    label: @Composable () -> Unit,
    value: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    gap: Dp = VelaSpacing.sm,
    rowGap: Dp = VelaSpacing.xs,
    stackedValue: Alignment.Horizontal = Alignment.End,
) {
    val policy = remember(gap, rowGap, stackedValue) { LabelBesideValuePolicy(gap, rowGap, stackedValue) }
    Layout(contents = listOf(label, value), modifier = modifier, measurePolicy = policy)
}

/**
 * The layout, intrinsics included. The fee row asks for its intrinsic height
 * (its refresh control shares the card), and the default intrinsics measure
 * with an UNBOUNDED width — which the side-by-side arithmetic once turned into
 * a width no `Constraints` can hold, and the app died drawing the Send form
 * (device-found on the Xiaomi, spec 078 round 2).
 */
private class LabelBesideValuePolicy(
    private val gap: Dp,
    private val rowGap: Dp,
    private val stackedValue: Alignment.Horizontal,
) : MultiContentMeasurePolicy {

    override fun MeasureScope.measure(measurables: List<List<Measurable>>, constraints: Constraints): MeasureResult {
        val labelPart = measurables[0].first()
        val valuePart = measurables[1].first()
        val gapPx = gap.roundToPx()
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val labelWide = labelPart.maxIntrinsicWidth(Constraints.Infinity)
        val valueWide = valuePart.maxIntrinsicWidth(Constraints.Infinity)
        val sideBySide = labelWide + gapPx + valueWide
        // Unbounded (an intrinsic pass, a scroller): both on one line.
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth else sideBySide.coerceAtLeast(constraints.minWidth)
        return if (sideBySide <= width) {
            val l = labelPart.measure(loose.copy(maxWidth = width))
            val v = valuePart.measure(loose.copy(maxWidth = (width - l.width - gapPx).coerceAtLeast(0)))
            val height = maxOf(l.height, v.height)
            layout(width, height) {
                l.placeRelative(0, (height - l.height) / 2)
                v.placeRelative(width - v.width, (height - v.height) / 2)
            }
        } else {
            val l = labelPart.measure(loose.copy(maxWidth = width))
            val v = valuePart.measure(loose.copy(maxWidth = width))
            val top = l.height + rowGap.roundToPx()
            layout(width, top + v.height) {
                l.placeRelative(0, 0)
                v.placeRelative(stackedValue.align(v.width, width, LayoutDirection.Ltr), top)
            }
        }
    }

    override fun IntrinsicMeasureScope.maxIntrinsicWidth(measurables: List<List<IntrinsicMeasurable>>, height: Int): Int =
        measurables[0].first().maxIntrinsicWidth(height) + gap.roundToPx() + measurables[1].first().maxIntrinsicWidth(height)

    override fun IntrinsicMeasureScope.minIntrinsicWidth(measurables: List<List<IntrinsicMeasurable>>, height: Int): Int =
        maxOf(measurables[0].first().minIntrinsicWidth(height), measurables[1].first().minIntrinsicWidth(height))

    override fun IntrinsicMeasureScope.minIntrinsicHeight(measurables: List<List<IntrinsicMeasurable>>, width: Int): Int =
        heightFor(measurables, width)

    override fun IntrinsicMeasureScope.maxIntrinsicHeight(measurables: List<List<IntrinsicMeasurable>>, width: Int): Int =
        heightFor(measurables, width)

    private fun IntrinsicMeasureScope.heightFor(measurables: List<List<IntrinsicMeasurable>>, width: Int): Int {
        val label = measurables[0].first()
        val value = measurables[1].first()
        val gapPx = gap.roundToPx()
        val labelWide = label.maxIntrinsicWidth(Constraints.Infinity)
        val valueWide = value.maxIntrinsicWidth(Constraints.Infinity)
        return if (width == Constraints.Infinity || labelWide + gapPx + valueWide <= width) {
            val valueRoom = if (width == Constraints.Infinity) width else (width - labelWide - gapPx).coerceAtLeast(0)
            maxOf(label.maxIntrinsicHeight(width), value.maxIntrinsicHeight(valueRoom))
        } else {
            label.maxIntrinsicHeight(width) + rowGap.roundToPx() + value.maxIntrinsicHeight(width)
        }
    }
}
