package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/** The annotation tag a link phrase inside an ack sentence carries. */
const val ACK_LINK_TAG: String = "ack-link"

/**
 * Acknowledgment checkbox row (spec 014, A1–A3): hairline square when
 * unchecked, accent fill + white ✓ when checked; muted wrapping text that may
 * carry inline link phrases (row 3). The whole row toggles (Role.Checkbox) —
 * INCLUDING the sentence, as it does on web — and a tap that lands on a link
 * phrase opens that document instead of toggling.
 *
 * The hit test is done HERE, against the text layout, rather than left to
 * Compose's `LinkAnnotation`. Device-found 2026-08-25 (Galaxy S22): with two
 * links in one wrapped sentence, the second link's tap box was laid out over
 * the START of the first line, so tapping the words "I have read" opened the
 * terms. Mapping the tap to a character offset and reading the annotation
 * there cannot drift from what is drawn — the same layout answers both.
 */
@Composable
fun VelaAckRow(
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    text: AnnotatedString,
    modifier: Modifier = Modifier,
    onLink: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .toggleable(
                value = checked,
                role = Role.Checkbox,
                onValueChange = onCheckedChange,
            )
            // Vertical padding keeps the touch target comfortable while the
            // row itself hugs its wrapped text.
            .padding(vertical = VelaSpacing.sm),
        verticalAlignment = Alignment.Top,
    ) {
        val boxShape = RoundedCornerShape(VelaRadius.sm)
        Box(
            modifier = Modifier
                .padding(top = VelaSpacing.xs)
                .size(VelaSizing.checkboxBox)
                .clip(boxShape)
                // A check is a selected state: neutral ink, never the
                // accent (web `.ack input { accent-color: fg.base }`).
                .background(if (checked) colors.fgBase else colors.bgRaised)
                .border(
                    width = VelaBorder.hairline,
                    color = if (checked) colors.fgBase else colors.borderStrong,
                    shape = boxShape,
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (checked) {
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.fgInverse,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Text(
            text = text,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.regular,
            fontSize = VelaTextSize.base,
            lineHeight = VelaLeading.normal * VelaTextSize.base,
            onTextLayout = { layout = it },
            modifier = Modifier
                .weight(1f)
                .pointerInput(text) {
                    detectTapGestures { position ->
                        val hit = layout?.let { result ->
                            val line = result.getLineForVerticalPosition(position.y)
                            // Past the end of a line is not the last glyph on
                            // it: `getOffsetForPosition` would snap there, and
                            // the empty run after "Privacy Policy" would open
                            // the privacy policy.
                            val inside = position.x >= result.getLineLeft(line) &&
                                position.x <= result.getLineRight(line)
                            if (inside) {
                                val offset = result.getOffsetForPosition(position)
                                text.getStringAnnotations(ACK_LINK_TAG, offset, offset)
                                    .firstOrNull()
                                    ?.item
                            } else {
                                null
                            }
                        }
                        if (hit != null) onLink(hit) else onCheckedChange(!checked)
                    }
                },
        )
    }
}
