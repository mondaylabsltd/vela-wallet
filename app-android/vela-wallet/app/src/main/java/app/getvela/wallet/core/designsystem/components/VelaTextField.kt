package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontFeatures
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/**
 * Single-line input (spec 014, A1–A3): optional label → sunken well → optional
 * error hint (error.base, A3) → helper caption. The error line appears between
 * the field and the caption without displacing the field (spec edge case).
 */
@Composable
fun VelaTextField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    placeholder: String,
    modifier: Modifier = Modifier,
    errorText: String? = null,
    helperText: String? = null,
) {
    val colors = VelaTheme.colors
    val interactionSource = remember { MutableInteractionSource() }
    val focused by interactionSource.collectIsFocusedAsState()
    val isError = errorText != null
    val borderColor = when {
        isError -> colors.errorBase
        // Focus ring per house convention: the accent outer-ring color.
        focused -> colors.fixed.focusRingOuter
        else -> colors.borderBase
    }
    val borderWidth = if (isError || focused) VelaBorder.emphasis else VelaBorder.hairline
    Column(modifier = modifier.fillMaxWidth()) {
        // An empty label renders NOTHING — not an empty Text with its own line
        // box, which would leave a band of dead space above the field. The
        // create screen passes "" because its heading already names the field
        // (spec 019); the placeholder still names it to assistive technology.
        if (label.isNotEmpty()) {
            Text(
                text = label,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = VelaSizing.controlLg)
                .clip(RoundedCornerShape(VelaRadius.lg))
                .background(colors.bgSunken)
                .border(
                    width = borderWidth,
                    color = borderColor,
                    shape = RoundedCornerShape(VelaRadius.lg),
                )
                .padding(horizontal = VelaSpacing.xl),
            contentAlignment = Alignment.CenterStart,
        ) {
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = true,
                // BasicTextField has no inline typography params; this TextStyle
                // is assembled purely from tokens (house Text rule equivalent).
                textStyle = TextStyle(
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.lg,
                    // An address typed or pasted reads "0x", not "0×".
                    fontFeatureSettings = VelaFontFeatures,
                ),
                cursorBrush = SolidColor(colors.accentBase),
                interactionSource = interactionSource,
                modifier = Modifier.fillMaxWidth(),
            )
            if (value.isEmpty()) {
                Text(
                    text = placeholder,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                )
            }
        }
        if (errorText != null) {
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = errorText,
                color = colors.errorBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
        }
        if (helperText != null) {
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = helperText,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
        }
    }
}
