package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/**
 * Where an address (or any long hex) may break across lines — nowhere nobody
 * chose (r13: the create sheet wrapped a member's last five digits alone onto
 * a line, `e130b`).
 *
 * The desktop's rule (`app-desktop/.../ui/address.rs`), one for every shell:
 * a full address is two unbreakable halves — `0x` + half the hex | the other
 * half, 22 + 20 for an address — side by side where the line holds them, else
 * the second under the first; a short form (`0x88cCA0…266894`) is one
 * unbreakable line; anything else (a name, a link, calldata) wraps as text does.
 */
object AddressBreaks {
    /** Hex longer than this is split in two (a 6+6 short form never is). */
    private const val SPLIT_FROM = 24

    /** Longer hex (calldata) wraps as text does: two halves would not fit. */
    private const val SPLIT_UP_TO = 66

    /** The two halves [VelaAddressText] draws [text] as, or `null` when it is drawn whole. */
    fun halves(text: String): Pair<String, String>? {
        if (!(text.startsWith("0x") || text.startsWith("0X"))) return null
        if (text.length !in SPLIT_FROM..SPLIT_UP_TO) return null
        val hex = text.substring(2)
        if (!hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
        // The `0x` stays with the first half; both halves as near even as hex allows.
        val mid = 2 + (hex.length + 1) / 2
        return text.substring(0, mid) to text.substring(mid)
    }

    /** `0x777E98…61875a`: hex, an ellipsis, hex — and nothing that could wrap. */
    fun isShortForm(text: String): Boolean =
        text.length <= SPLIT_FROM + 4 &&
            (text.startsWith("0x") || text.startsWith("0X")) &&
            text.contains('…') &&
            text.none { it.isWhitespace() }

    /** Whether [text] is drawn by the address rule rather than as prose. */
    fun isAddressLike(text: String): Boolean = halves(text) != null || isShortForm(text)
}

/**
 * [text] (an address, a hash, its short form) with line breaks only where
 * [AddressBreaks] chose them. Screen readers and tests read it as one text.
 *
 * The receive card, the recipient field and a contact's address block keep
 * their own two fixed lines (the 018 canon every client draws, split 21 + 21);
 * this is for an address that would otherwise wrap where the line ran out.
 */
@Composable
fun VelaAddressText(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = VelaTheme.colors.fgBase,
    fontSize: TextUnit = VelaTextSize.base,
    fontWeight: FontWeight = VelaFontWeight.regular,
    fontFamily: FontFamily = VelaMonoFontFamily,
    lineHeight: TextUnit = TextUnit.Unspecified,
    textAlign: TextAlign? = null,
    maxLines: Int = Int.MAX_VALUE,
) {
    val halves = remember(text) { AddressBreaks.halves(text) }
    val piece: @Composable (String) -> Unit = { part ->
        Text(
            text = part,
            color = color,
            fontFamily = fontFamily,
            fontWeight = fontWeight,
            fontSize = fontSize,
            lineHeight = lineHeight,
            softWrap = false,
            maxLines = 1,
        )
    }
    when {
        halves != null -> FlowRow(
            modifier.clearAndSetSemantics { this.text = AnnotatedString(text) },
            horizontalArrangement = when (textAlign) {
                TextAlign.Center -> Arrangement.Center
                TextAlign.End, TextAlign.Right -> Arrangement.End
                else -> Arrangement.Start
            },
        ) {
            piece(halves.first)
            piece(halves.second)
        }
        // A short form is one line; anything else wraps as text does.
        else -> Text(
            text = text,
            modifier = modifier,
            color = color,
            fontFamily = fontFamily,
            fontWeight = fontWeight,
            fontSize = fontSize,
            lineHeight = lineHeight,
            textAlign = textAlign,
            softWrap = !AddressBreaks.isShortForm(text),
            maxLines = if (AddressBreaks.isShortForm(text)) 1 else maxLines,
            overflow = if (maxLines == Int.MAX_VALUE) TextOverflow.Clip else TextOverflow.Ellipsis,
        )
    }
}
