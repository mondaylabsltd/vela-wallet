package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.SearchFieldModel

/**
 * Contacts search field (spec vocabulary #6, mock C1): full-width sunken well,
 * leading search glyph, placeholder 搜索名字、ENS 或地址, and — once a query is
 * present (c1f) — the typed text plus a clear affordance.
 *
 * **Pass [onQueryChange] and it accepts typing; omit it and it renders exactly
 * as it always has.** Spec 023 drew it as an action sink because every query
 * came from a fixture — and spec 040 gave `ContactsLive.home` a working filter
 * with nothing on the phone able to produce a query for it. A search box that
 * cannot be searched with is the same bug as a settings field that cannot be
 * typed into, one screen over.
 *
 * The default keeps every gallery state pixel-identical.
 */
@Composable
fun ContactsSearchField(
    model: SearchFieldModel,
    modifier: Modifier = Modifier,
    onClick: () -> Unit = {},
    onClear: () -> Unit = {},
    onQueryChange: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .clickable(onClick = onClick)
            .heightIn(min = VelaSizing.controlMd)
            .padding(horizontal = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = VelaIcons.Search,
            contentDescription = null,
            tint = colors.fgSubtle,
            modifier = Modifier.size(VelaIconSize.base),
        )
        Spacer(modifier = Modifier.width(VelaSpacing.md))
        if (onQueryChange == null) {
            Text(
                text = if (model.filtering) model.query else model.placeholder,
                color = if (model.filtering) colors.fgBase else colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
        } else {
            BasicTextField(
                value = model.query,
                onValueChange = onQueryChange,
                singleLine = true,
                textStyle = TextStyle(
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                ),
                cursorBrush = SolidColor(colors.accentBase),
                keyboardOptions = KeyboardOptions(
                    // A name, an ENS label or an address — none of which want a
                    // capital first letter supplied by the keyboard.
                    autoCorrectEnabled = false,
                    capitalization = KeyboardCapitalization.None,
                    imeAction = ImeAction.Search,
                ),
                modifier = Modifier.weight(1f),
                decorationBox = { field ->
                    if (model.query.isEmpty()) {
                        Text(
                            text = model.placeholder,
                            color = colors.fgSubtle,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.regular,
                            fontSize = VelaTextSize.base,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    field()
                },
            )
        }
        if (model.filtering) {
            Box(
                modifier = Modifier
                    .size(VelaIconSize.lg)
                    .clickable(onClick = onClear),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = VelaIcons.Close,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.base),
                )
            }
        }
    }
}
