package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import app.getvela.wallet.core.designsystem.components.AddressBreaks
import app.getvela.wallet.core.designsystem.components.VelaAddressText
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaTextField
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic

/*
 * The multisig screens' small parts (iOS `MultisigComponents.swift`, the web's
 * multisig ui): quiet, typographic, de-containered (docs/DESIGN-LANGUAGE.md).
 * Accent only on the action that signs or moves value; everything else ink.
 */

/** The text roles the multisig screens draw in. */
enum class MsType(val size: TextUnit, val weight: FontWeight, val mono: Boolean = false) {
    Title(VelaTextSize.xl2, VelaFontWeight.bold),
    RowTitle(VelaTextSize.lg, VelaFontWeight.semibold),
    Field(VelaTextSize.lg, VelaFontWeight.medium),
    Body(VelaTextSize.lg, VelaFontWeight.regular),
    Sub(VelaTextSize.base, VelaFontWeight.regular),
    Label(VelaTextSize.sm, VelaFontWeight.semibold),
    Mono(VelaTextSize.base, VelaFontWeight.regular, mono = true),
    Action(VelaTextSize.lg, VelaFontWeight.semibold),
}

@Composable
fun MsText(
    text: String,
    type: MsType,
    modifier: Modifier = Modifier,
    color: Color = VelaTheme.colors.fgBase,
    maxLines: Int = Int.MAX_VALUE,
) {
    // An address (full, short, a hash) never breaks where the line ran out:
    // the shared rule draws it (r13: a lone `e130b` under a member).
    if (AddressBreaks.isAddressLike(text)) {
        VelaAddressText(
            text = text,
            modifier = modifier,
            color = color,
            fontSize = type.size,
            fontWeight = type.weight,
            fontFamily = if (type.mono) VelaMonoFontFamily else VelaFontFamily,
        )
        return
    }
    Text(
        text = text,
        modifier = modifier,
        color = color,
        fontFamily = if (type.mono) VelaMonoFontFamily else VelaFontFamily,
        fontWeight = type.weight,
        fontSize = type.size,
        maxLines = maxLines,
        overflow = if (maxLines == Int.MAX_VALUE) TextOverflow.Clip else TextOverflow.Ellipsis,
    )
}

/** How a note reads: neutral, a caution, a danger, a good thing. */
enum class MsTone { Neutral, Warning, Danger, Positive }

/** One sentence the core's verdict is said in. */
@Composable
fun MsNote(text: String, modifier: Modifier = Modifier, tone: MsTone = MsTone.Neutral, tag: String? = null) {
    val colors = VelaTheme.colors
    val (ink, wash) = when (tone) {
        MsTone.Neutral -> colors.fgMuted to null
        MsTone.Warning -> colors.fgBase to colors.warningSoft
        MsTone.Danger -> colors.errorBase to colors.errorSoft
        MsTone.Positive -> colors.fgBase to colors.successSoft
    }
    val base = modifier.fillMaxWidth().let { m -> tag?.let { m.testTag(it) } ?: m }
    if (wash == null) {
        MsText(text, MsType.Sub, base, color = ink)
    } else {
        Row(
            base.background(wash, RoundedCornerShape(VelaRadius.md)).padding(VelaSpacing.md),
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            if (tone != MsTone.Positive) {
                Icon(VelaIcons.TriangleAlert, null, tint = if (tone == MsTone.Danger) colors.errorBase else colors.warningBase, modifier = Modifier.size(VelaIconSize.base))
            }
            MsText(text, MsType.Sub, color = ink)
        }
    }
}

/** A short tag: a rule, a state. */
@Composable
fun MsChip(text: String, modifier: Modifier = Modifier, tone: MsTone = MsTone.Neutral) {
    val colors = VelaTheme.colors
    val (ink, border) = when (tone) {
        MsTone.Neutral -> colors.fgMuted to colors.borderBase
        MsTone.Warning -> colors.fgBase to colors.warningBorder
        MsTone.Danger -> colors.errorBase to colors.errorBase
        MsTone.Positive -> colors.fgBase to colors.borderBase
    }
    Box(
        modifier
            .border(VelaBorder.hairline, border, RoundedCornerShape(VelaRadius.full))
            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
    ) {
        MsText(text, MsType.Label, color = ink)
    }
}

/** A section: its small heading, then its rows. */
@Composable
fun MsSection(title: String, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        MsText(title.uppercase(), MsType.Label, Modifier.semantics { heading() }, color = VelaTheme.colors.fgSubtle)
        content()
    }
}

/** A label and its value on one line (a value that is an address is mono, and may be copied). */
@Composable
fun MsRow(label: String, value: String, modifier: Modifier = Modifier, mono: Boolean = false, copyable: Boolean = false, tag: String? = null) {
    val clipboard = LocalClipboardManager.current
    val haptic = rememberVelaHaptic()
    Column(
        modifier
            .fillMaxWidth()
            .let { m -> tag?.let { m.testTag(it) } ?: m }
            .then(
                if (copyable) Modifier.clickable(onClickLabel = value) {
                    clipboard.setText(AnnotatedString(value))
                    haptic(VelaHaptic.Press)
                } else Modifier,
            )
            .padding(vertical = VelaSpacing.xs),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
    ) {
        MsText(label, MsType.Sub, color = VelaTheme.colors.fgSubtle)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            MsText(value, if (mono) MsType.Mono else MsType.Field, Modifier.weight(1f, fill = false))
            if (copyable) Icon(VelaIcons.Copy, null, tint = VelaTheme.colors.fgSubtle, modifier = Modifier.size(VelaIconSize.sm))
        }
    }
}

/** A row that opens something: icon, title, its line under it, a chevron. */
@Composable
fun MsNavRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: ImageVector? = null,
    tag: String? = null,
    trailing: @Composable (RowScope.() -> Unit)? = null,
    onClick: () -> Unit,
) {
    val colors = VelaTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .defaultMinSize(minHeight = VelaSizing.hitTarget)
            .let { m -> tag?.let { m.testTag(it) } ?: m }
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        icon?.let { Icon(it, null, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg)) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            MsText(title, MsType.Field)
            subtitle?.let { MsText(it, MsType.Sub, color = colors.fgSubtle) }
        }
        trailing?.invoke(this)
        Icon(VelaIcons.ChevronRight, null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.sm))
    }
}

/** A text action (ink, never the accent). */
@Composable
fun MsTextButton(text: String, modifier: Modifier = Modifier, enabled: Boolean = true, tag: String? = null, danger: Boolean = false, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier
            .defaultMinSize(minHeight = VelaSizing.hitTarget)
            .let { m -> tag?.let { m.testTag(it) } ?: m }
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.CenterStart,
    ) {
        MsText(text, MsType.Action, color = if (!enabled) colors.fgSubtle else if (danger) colors.errorBase else colors.fgBase)
    }
}

/** A thin rule between rows. */
@Composable
fun MsDivider(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(VelaBorder.hairline).background(VelaTheme.colors.borderBase))
}

/** A field: the design system's, tagged. */
@Composable
fun MsField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
    error: String? = null,
    tag: String? = null,
) {
    VelaTextField(
        value = value,
        onValueChange = onValueChange,
        label = label,
        placeholder = placeholder,
        modifier = modifier.let { m -> tag?.let { m.testTag(it) } ?: m },
        errorText = error,
    )
}

/**
 * A multisig sheet: the app's one bottom sheet, its title and close, its
 * content scrolling inside. [dismissible] `false` while something is being
 * signed (a stray swipe must not throw away a ceremony in flight).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MsSheet(
    title: String,
    closeLabel: String,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    dismissible: Boolean = true,
    tag: String? = null,
    /** A new key starts the content at its top (a sheet's next step). */
    scrollKey: Any? = null,
    /**
     * r10 S4: a form with something in it. A swipe or a tap outside does not
     * close it, and Back or ✕ go to [onClose] — which asks first — instead
     * of the form vanishing under a gesture meant for the keyboard.
     */
    guarded: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = VelaTheme.colors
    val scroll = androidx.compose.runtime.key(scrollKey) { rememberScrollState() }
    val guard = androidx.compose.runtime.rememberUpdatedState(guarded)
    @OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
    val sheetState = androidx.compose.material3.rememberModalBottomSheetState(
        skipPartiallyExpanded = true,
        confirmValueChange = { it != androidx.compose.material3.SheetValue.Hidden || !guard.value },
    )
    VelaModalSheet(onDismissRequest = onClose, sheetState = sheetState, containerColor = colors.bgBase, dismissible = dismissible, followAppTextSize = true) {
        // Registered inside the sheet's own window, after its own: Back asks.
        androidx.activity.compose.BackHandler(enabled = guarded && dismissible) { onClose() }
        Column(
            modifier
                .fillMaxWidth()
                .let { m -> tag?.let { m.testTag(it) } ?: m }
                .verticalScroll(scroll)
                .padding(horizontal = VelaSpacing.xl)
                .padding(bottom = VelaSpacing.xl)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                MsText(title, MsType.Title, Modifier.weight(1f).semantics { heading() })
                Box(
                    Modifier
                        .size(VelaSizing.sheetClose)
                        .clickable(role = Role.Button, onClick = onClose)
                        .semantics { contentDescription = closeLabel }
                        .testTag("multisig.sheet.close"),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(VelaIcons.Close, null, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.lg))
                }
            }
            content()
            Spacer(Modifier.width(VelaSpacing.none))
        }
    }
}
