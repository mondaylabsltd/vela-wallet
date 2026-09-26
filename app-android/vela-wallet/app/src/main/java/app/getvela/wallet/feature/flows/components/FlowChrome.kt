package app.getvela.wallet.feature.flows.components

import app.getvela.wallet.core.designsystem.components.VelaLabelBesideValue
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.windowInsetsBottomHeight
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.flows.FilterChipModel
import app.getvela.wallet.feature.flows.FlowHeaderModel
import app.getvela.wallet.feature.wallet.components.WalletMetrics

/**
 * The chrome and the inputs of the wallet flows (spec 021 components 1, 4–7).
 *
 * Grouped in one file the way `WalletPrimitives.kt` and `ContactsPrimitives.kt`
 * are: these are small, they are only ever used together, and a file each would
 * be more import blocks than code.
 */

/**
 * The phone page frame for every non-sheet screen (component 1).
 *
 * Back chevron on its own line, then a large title that may carry a trailing
 * text action, a network pill, or neither. R1, A1, T1, SD1, SD2, SD3 and SD4
 * are all this frame with a different body — the mocks differ in what sits
 * under the title, not in how the title sits.
 */
@Composable
fun FlowScaffold(
    header: FlowHeaderModel,
    modifier: Modifier = Modifier,
    onBack: () -> Unit = {},
    onAction: () -> Unit = {},
    onPill: () -> Unit = {},
    footer: (@Composable () -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .statusBarsPadding(),
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSizing.screenPaddingX),
        ) {
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        // The chevron's own glyph inset already reads as
                        // padding; pulling the button back by it puts the
                        // STROKE on the screen margin, where the title starts.
                        .offset(x = -VelaSpacing.md)
                        .size(VelaSizing.controlSm)
                        .clickable(onClick = onBack),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = VelaIcons.ChevronLeft,
                        contentDescription = header.backLabel,
                        tint = colors.fgBase,
                        modifier = Modifier.size(VelaIconSize.lg),
                    )
                }
                Spacer(modifier = Modifier.weight(1f))
                header.action?.let { action ->
                    Text(
                        text = action,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.medium,
                        fontSize = VelaTextSize.lg,
                        modifier = Modifier
                            .clickable(onClick = onAction)
                            .padding(VelaSpacing.sm),
                    )
                }
            }
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            // The title is never cut (design review, 078 round 3: "Token
            // auswä…" at the largest size): beside the network pill when both
            // fit whole, otherwise the pill takes the line under it.
            val title: @Composable () -> Unit = {
                Text(
                    text = header.title,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl4,
                    lineHeight = VelaLeading.hero * VelaTextSize.xl4,
                )
            }
            val pill = header.pill
            if (pill == null) {
                title()
            } else {
                VelaLabelBesideValue(
                    modifier = Modifier.fillMaxWidth(),
                    gap = VelaSpacing.lg,
                    rowGap = VelaSpacing.md,
                    stackedValue = Alignment.Start,
                    label = title,
                    value = {
                        Row(
                            modifier = Modifier
                                .background(colors.bgRaised, CircleShape)
                                .clickable(onClick = onPill)
                                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            pill.dots.forEachIndexed { index, dot ->
                                Box(
                                    modifier = Modifier
                                        // Overlapped, not spaced: the cluster stands
                                        // for "several networks", and three separate
                                        // dots read as three separate controls.
                                        .offset(x = -VelaSpacing.sm * index)
                                        .size(WalletMetrics.pillDotSize)
                                        .background(colors.bgRaised, CircleShape)
                                        .padding(VelaBorder.emphasis)
                                        .background(dot, CircleShape),
                                )
                            }
                            Spacer(modifier = Modifier.width(VelaSpacing.sm))
                            Text(
                                text = pill.label,
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.semibold,
                                fontSize = VelaTextSize.base,
                            )
                            Icon(
                                imageVector = VelaIcons.ChevronDown,
                                contentDescription = null,
                                tint = colors.fgMuted,
                                modifier = Modifier.size(VelaIconSize.sm),
                            )
                        }
                    },
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            content()
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            // Only when nothing is pinned below: with a footer the inset is
            // the footer's, and taking it twice leaves a gap the mocks do not
            // draw. Without one the last row runs under the navigation bar —
            // R1's eighth network did exactly that on the first device run.
            if (footer == null) {
                Spacer(modifier = Modifier.windowInsetsBottomHeight(WindowInsets.navigationBars))
            }
        }
        footer?.let {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .navigationBarsPadding()
                    .padding(
                        horizontal = VelaSizing.screenPaddingX,
                        vertical = VelaSpacing.lg,
                    ),
            ) { it() }
        }
    }
}

/**
 * The filled search field (component 4) — R1's network search, T1's and SD1's
 * token search, SD2e's contact search.
 *
 * Filtering is live and animation-free by design (SPEC 动效 · 收款): rows leave
 * as the query narrows, and a transition on a list that changes every keystroke
 * reads as lag rather than polish.
 */
@Composable
fun FlowSearchField(
    placeholder: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(VelaSizing.controlLg)
            .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
            .padding(horizontal = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = VelaIcons.Search,
            contentDescription = null,
            tint = colors.fgSubtle,
            modifier = Modifier.size(VelaIconSize.md),
        )
        Spacer(modifier = Modifier.width(VelaSpacing.md))
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (value.isEmpty()) {
                Text(
                    text = placeholder,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
            }
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = true,
                textStyle = TextStyle(
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                ),
                cursorBrush = SolidColor(colors.accentBase),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * The two-segment toggle (component 5) — T3's ERC-20 / native tabs and SD2c's
 * fiat / token pricing switch.
 *
 * The design review made this the ONE segmented control in the product, so it
 * takes its segments as data rather than growing a variant per caller.
 */
@Composable
fun FlowSegmentedToggle(
    options: List<Pair<String, String>>,
    selectedId: String,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xs),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
    ) {
        options.forEach { (id, label) ->
            val on = id == selectedId
            Box(
                modifier = Modifier
                    .weight(1f)
                    .background(
                        if (on) colors.bgRaised else Color.Transparent,
                        RoundedCornerShape(VelaRadius.md),
                    )
                    .clickable { onSelect(id) }
                    .padding(vertical = VelaSpacing.md),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = label,
                    color = if (on) colors.fgBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = if (on) VelaFontWeight.semibold else VelaFontWeight.medium,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
            }
        }
    }
}

/**
 * SD1's token-class filter chips (component 6).
 *
 * Distinct from [FlowSegmentedToggle] on purpose. That control divides ONE
 * space into named halves and always fills its width; this is a row of
 * independent narrowings that hugs its labels and scrolls past the screen edge
 * when a locale needs the room.
 */
@Composable
fun FlowFilterChips(
    options: List<FilterChipModel>,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        options.forEach { option ->
            Box(
                modifier = Modifier
                    .background(
                        // The selected chip inverts rather than taking the
                        // accent: accent means "moves money" in this product,
                        // and narrowing a list does not.
                        if (option.selected) colors.fgBase else colors.bgRaised,
                        CircleShape,
                    )
                    .clickable { onSelect(option.id) }
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm),
            ) {
                Text(
                    text = option.label,
                    color = if (option.selected) colors.bgBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = if (option.selected) {
                        VelaFontWeight.semibold
                    } else {
                        VelaFontWeight.medium
                    },
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                )
            }
        }
    }
}

/**
 * The monospace field (component 7): T3's contract address, T3b's network
 * query, SD2c's pasted recipient list.
 *
 * Addresses are compared character by character by the people pasting them,
 * which is the reason for the mono face — and the reason the error state
 * colours the BORDER and prints underneath rather than tinting the text, which
 * would make the characters harder to read exactly when they most need reading.
 */
@Composable
fun FlowMonoField(
    value: String,
    modifier: Modifier = Modifier,
    label: String? = null,
    placeholder: String? = null,
    error: String? = null,
    minLines: Int = 1,
    onValueChange: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        label?.let {
            Text(
                text = it,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                .then(
                    if (error != null) {
                        Modifier.border(
                            VelaBorder.hairline,
                            colors.errorBase,
                            RoundedCornerShape(VelaRadius.lg),
                        )
                    } else {
                        Modifier
                    }
                )
                .heightIn(min = VelaSizing.controlLg)
                .padding(VelaSpacing.lg),
            contentAlignment = Alignment.CenterStart,
        ) {
            if (value.isEmpty() && placeholder != null) {
                Text(
                    text = placeholder,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = minLines,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = minLines == 1,
                minLines = minLines,
                textStyle = TextStyle(
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                ),
                cursorBrush = SolidColor(colors.accentBase),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        error?.let {
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = it,
                color = colors.errorBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
    }
}
