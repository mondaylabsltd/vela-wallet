package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.material3.Text
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.contacts.ContactFormModel

/**
 * The add/edit contact form (spec 045 US5), drawn from 018's vocabulary: a
 * raised sheet, two labelled fields, the core's one error line, Save and
 * Cancel. Typing is echoed locally (the machine's view lags fast typing —
 * spec 043's device finding); the values still round-trip through the core,
 * which validates and gates Save.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ContactFormSheet(
    model: ContactFormModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    onName: (String) -> Unit = {},
    onAddress: (String) -> Unit = {},
    onSave: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
        modifier = modifier,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl)
                .navigationBarsPadding(),
        ) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            FormField(
                label = model.nameLabel,
                placeholder = model.namePlaceholder,
                value = model.name,
                mono = false,
                tag = "contact-name",
                onValueChange = onName,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            FormField(
                label = model.addressLabel,
                placeholder = model.addressPlaceholder,
                value = model.address,
                mono = true,
                tag = "contact-address",
                onValueChange = if (model.addressLocked) null else onAddress,
            )
            model.error?.let {
                Spacer(modifier = Modifier.height(VelaSpacing.sm))
                Text(
                    text = it,
                    color = colors.errorBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(
                        if (model.saveEnabled) colors.accentBase else colors.bgSunken,
                        RoundedCornerShape(VelaRadius.full),
                    )
                    .clickable(enabled = model.saveEnabled, onClick = onSave)
                    .padding(vertical = VelaSpacing.lg),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = model.save,
                    color = if (model.saveEnabled) colors.fgInverse else colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.base,
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = model.cancel,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onDismiss)
                    .padding(vertical = VelaSpacing.md),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
    }
}

@Composable
private fun FormField(
    label: String,
    placeholder: String,
    value: String,
    mono: Boolean,
    tag: String,
    onValueChange: ((String) -> Unit)?,
) {
    val colors = VelaTheme.colors
    val family: FontFamily = if (mono) VelaMonoFontFamily else VelaFontFamily
    val style = TextStyle(color = colors.fgBase, fontFamily = family, fontSize = VelaTextSize.base)
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = label,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.xs,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xs))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgSunken, RoundedCornerShape(VelaRadius.md))
                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
        ) {
            if (onValueChange == null) {
                Text(text = value, style = style.copy(color = colors.fgMuted), maxLines = 2)
            } else {
                var typed by remember { mutableStateOf(value) }
                val sent = remember { ArrayDeque<String>().apply { addLast(value) } }
                LaunchedEffect(value) { if (value !in sent) typed = value }
                BasicTextField(
                    value = typed,
                    onValueChange = { next ->
                        typed = next
                        sent.addLast(next)
                        if (sent.size > 256) sent.removeFirst()
                        onValueChange(next)
                    },
                    textStyle = style,
                    cursorBrush = SolidColor(colors.accentBase),
                    maxLines = 2,
                    modifier = Modifier
                        .fillMaxWidth()
                        .semantics { contentDescription = tag },
                    decorationBox = { inner ->
                        if (typed.isEmpty()) Text(text = placeholder, style = style.copy(color = colors.fgSubtle), maxLines = 1)
                        inner()
                    },
                )
            }
        }
    }
}
