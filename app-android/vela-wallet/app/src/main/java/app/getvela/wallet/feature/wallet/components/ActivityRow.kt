package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.ActivityKind
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.TitlePlace

/**
 * Day-group label above activity rows (今天 / 昨天; spec vocabulary #8).
 */
@Composable
fun DayLabel(label: String, modifier: Modifier = Modifier) {
    Text(
        text = label,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        modifier = modifier.padding(top = VelaSpacing.lg, bottom = VelaSpacing.sm),
    )
}

/**
 * Activity row (spec vocabulary #8): direction circle with a chain-dot badge,
 * title + counterparty subtitle, trailing signed amount (+success / −fg.base)
 * with a small unit. Masked variant renders the fixture's dot glyphs while the
 * unit stays visible (mock H5).
 */
@Composable
fun ActivityRow(model: ActivityRowModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    // 087 F11: a row with no figure — a dApp call that moved no coin of
    // ours — draws no amount cell at all. Spec 097 N5: what came back is
    // drawn on its own when nothing left.
    val figure = model.hasFigure || model.received != null
    ActivityRowLayout(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        gap = VelaSpacing.lg,
        mark = {
            Box(contentAlignment = Alignment.BottomEnd) {
                Box(
                    modifier = Modifier
                        .size(WalletMetrics.avatarSize)
                        .background(colors.bgSunken, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = when (model.kind) {
                            ActivityKind.Sent -> VelaIcons.ArrowUpRight
                            ActivityKind.Received -> VelaIcons.ArrowDownLeft
                            ActivityKind.Dapp -> VelaIcons.Link2
                        },
                        contentDescription = null,
                        tint = when (model.kind) {
                            ActivityKind.Received -> colors.successBase
                            else -> colors.fgMuted
                        },
                        modifier = Modifier.size(VelaIconSize.md),
                    )
                }
                ChainBadge(color = model.badgeColor, logoUrl = model.badgeLogoUrl)
            }
        },
        words = {
            Column {
                val parts = model.titlePlace
                if (parts != null) {
                    PlaceTitle(parts, model.title)
                } else {
                    Text(
                        text = model.title,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                        // Two lines before a word is cut: "Contract interaction"
                        // is the whole of what the row says happened.
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    text = model.subtitle,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        },
        amount = if (figure) {
            { AmountText(model) }
        } else {
            null
        },
    )
}

/**
 * The row's three parts on one line: the mark, the words, the figure.
 *
 * **The figure takes the width it needs, and the words take the rest.** They
 * used to split the row in half whatever they held, so beside "−0.05 BNB" an
 * English title lost half its room to empty space and read "Contract
 * interacti…" (the 102 device run). A figure may still take AT MOST the half
 * it always had — an extreme one wraps its unit below itself rather than
 * push the title out (the H7 board) — and a row with no figure gives the
 * words all of it.
 */
@Composable
private fun ActivityRowLayout(
    modifier: Modifier,
    gap: Dp,
    mark: @Composable () -> Unit,
    words: @Composable () -> Unit,
    amount: (@Composable () -> Unit)?,
) {
    Layout(
        content = {
            Box { mark() }
            Box { words() }
            if (amount != null) Box { amount() }
        },
        modifier = modifier,
    ) { measurables, constraints ->
        val space = gap.roundToPx()
        val wide = constraints.maxWidth
        val markP = measurables[0].measure(Constraints())
        val shared = (wide - markP.width - space).coerceAtLeast(0)
        // The figure first: its own width, up to half of what it shares with the words.
        val amountP = measurables.getOrNull(2)?.measure(Constraints(maxWidth = ((shared - space) / 2).coerceAtLeast(0)))
        val wordsWide = (shared - (amountP?.let { it.width + space } ?: 0)).coerceAtLeast(0)
        val wordsP = measurables[1].measure(Constraints(minWidth = wordsWide, maxWidth = wordsWide))
        val tall = maxOf(markP.height, wordsP.height, amountP?.height ?: 0).coerceAtLeast(constraints.minHeight)
        layout(wide, tall) {
            markP.placeRelative(0, (tall - markP.height) / 2)
            wordsP.placeRelative(markP.width + space, (tall - wordsP.height) / 2)
            amountP?.placeRelative(wide - amountP.width, (tall - amountP.height) / 2)
        }
    }
}

@Composable
private fun AmountText(model: ActivityRowModel) {
    val colors = VelaTheme.colors
    val amountColor = when {
        // Spec 093: an unlimited allowance is the risk to see.
        model.danger -> colors.errorBase
        model.positive -> colors.successBase
        else -> colors.fgBase
    }
    Column(horizontalAlignment = Alignment.End) {
        if (model.hasFigure) AmountLine(model, amountColor)
        // A swap's coin back, beside what left (083 F1): what was expected.
        model.received?.let { back ->
            Text(
                text = back,
                color = colors.successBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.End,
            )
        }
    }
}

@Composable
private fun AmountLine(model: ActivityRowModel, amountColor: Color) {
    val colors = VelaTheme.colors
    Text(
        text = buildAnnotatedString {
            withStyle(
                SpanStyle(
                    color = amountColor,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                ),
            ) {
                append(model.amount)
            }
            if (model.amount.isNotBlank() && model.unit.isNotBlank()) append(" ")
            withStyle(
                SpanStyle(
                    // "Unlimited USDC" reads as one warning, unit and all.
                    color = if (model.danger) amountColor else colors.fgSubtle,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.sm,
                ),
            ) {
                append(model.unit)
            }
        },
        fontFamily = VelaFontFamily,
        textAlign = TextAlign.End,
    )
}

/**
 * A dApp row's title, the verb never the part cut.
 *
 * Whole when it fits — on one line, or on two before anything is cut: beside
 * a figure an English title ("Contract interaction on swap.example") does
 * not fit one line of a phone, and cutting its place down to "s…" told
 * nobody where (the 102 device run). Only a title two lines cannot hold goes
 * back to the one-line rule: the words either side of the place keep their
 * width, and the place — a site's host — takes what is left, cut in its
 * middle (「在 127.0…8137 合约交互」). A tail ellipsis would cut the verb
 * instead: 「在 127.0.0.1:8137 合约…」. Said as the one title it is.
 */
@Composable
private fun PlaceTitle(parts: TitlePlace, title: String) {
    val colors = VelaTheme.colors
    val style = TextStyle(
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.lg,
    )
    val measurer = rememberTextMeasurer()
    BoxWithConstraints {
        val wide = constraints.maxWidth
        // Measured before it is drawn, so the row never shows one title for a frame and another after.
        val whole = remember(title, wide, style) {
            !measurer.measure(title, style, overflow = TextOverflow.Ellipsis, maxLines = 2, constraints = Constraints(maxWidth = wide)).hasVisualOverflow
        }
        if (whole) {
            Text(text = title, style = style, maxLines = 2)
            return@BoxWithConstraints
        }
        @Composable
        fun part(text: String, modifier: Modifier = Modifier, overflow: TextOverflow = TextOverflow.Clip) = Text(
            text = text,
            style = style,
            maxLines = 1,
            softWrap = false,
            overflow = overflow,
            modifier = modifier,
        )
        Row(
            modifier = Modifier.clearAndSetSemantics { contentDescription = title },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (parts.lead.isNotEmpty()) part(if (parts.gapBefore) parts.lead + " " else parts.lead)
            part(parts.place, Modifier.weight(1f, fill = false), TextOverflow.MiddleEllipsis)
            if (parts.trail.isNotEmpty()) part(if (parts.gapAfter) " " + parts.trail else parts.trail)
        }
    }
}
