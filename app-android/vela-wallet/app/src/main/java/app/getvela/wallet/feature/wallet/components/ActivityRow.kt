package app.getvela.wallet.feature.wallet.components

import app.getvela.wallet.core.designsystem.tokens.VelaFontFeatures
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
import androidx.compose.ui.semantics.SemanticsPropertyKey
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
 * A dApp row's title: it wraps only BETWEEN its parts, and only the place is
 * ever cut.
 *
 * The title is the core's sentence about a place — 「在 {place} {intent}」,
 * "{intent} on {place}" — and a second line was added so an English one need
 * not cut its place down to "s…" (the 102 device run). But the two lines were
 * one wrapping text, and a wrapping text breaks where the width runs out:
 * 「在 arcorbs.game3.app 合 / 约交互」 on the owner's phone (2026-10-11), the
 * action split inside its word.
 *
 * So the lines are decided, not left to the text ([TitleLines]): whole on one
 * line when it fits; else two lines broken between the action and the place
 * — 「在 arcorbs.game3.app」 over 「合约交互」, "Contract interaction" over
 * "on arcorbs.game3.app". Every part is drawn on ONE line and cannot wrap;
 * the place (a site's host) takes what its line has left and is cut in its
 * middle when that is not enough. Said as the one title it is.
 */
@Composable
private fun PlaceTitle(parts: TitlePlace, title: String) {
    val colors = VelaTheme.colors
    val style = TextStyle(
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.lg,
        // A `style` handed to Text replaces the theme's, and this title names
        // a place — a host or an ADDRESS: "0x12ab…" was drawn "0×12ab…".
        fontFeatureSettings = VelaFontFeatures,
    )
    val measurer = rememberTextMeasurer()
    BoxWithConstraints {
        val wide = constraints.maxWidth
        // Measured before it is drawn, so the row never shows one title for a frame and another after.
        val lines = remember(parts, wide, style) {
            TitleLines.of(parts, wide) { text -> measurer.measure(text, style, maxLines = 1, softWrap = false).size.width }
        }
        Column(
            modifier = Modifier.clearAndSetSemantics {
                contentDescription = title
                this[ActivityTitleLinesKey] = lines
            },
        ) {
            lines.forEach { line ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (line.before.isNotEmpty()) TitlePart(line.before, style, wraps = line.wraps)
                    line.place?.let { place ->
                        Text(
                            text = place,
                            style = style,
                            maxLines = 1,
                            softWrap = false,
                            overflow = TextOverflow.MiddleEllipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                    }
                    if (line.after.isNotEmpty()) TitlePart(line.after, style, wraps = false)
                }
            }
        }
    }
}

/** The words either side of the place: one line, never wrapped — unless they alone are wider than the row. */
@Composable
private fun TitlePart(text: String, style: TextStyle, wraps: Boolean) = Text(
    text = text,
    style = style,
    maxLines = if (wraps) 2 else 1,
    softWrap = wraps,
    overflow = if (wraps) TextOverflow.Ellipsis else TextOverflow.Clip,
)

/** What a dApp row's title put on each of its lines, for a test: the row says its one title, the lines are here. */
internal val ActivityTitleLinesKey = SemanticsPropertyKey<List<TitleLine>>("ActivityTitleLines")

/**
 * One line of a dApp row's title: the words before the place, the place (when
 * this is its line), the words after. [wraps]: a line of words alone that is
 * wider than the row — the one case a part may take a second line, between
 * its own words.
 */
internal data class TitleLine(val before: String, val place: String?, val after: String, val wraps: Boolean = false) {
    val text: String get() = before + place.orEmpty() + after
}

/**
 * Where a dApp row's title breaks ([PlaceTitle]) — a rule over widths, so it
 * can be read without a screen.
 *
 * - It fits one line: one line.
 * - Else it breaks between the ACTION and the place, and nowhere else. What
 *   joins the two — 「在 」, " on ", 「で」, " bei " — stays on the place's
 *   line, before or after it as the language has it: 「在 arcorbs.game3.app」
 *   over 「合约交互」; "Contract interaction" over "on arcorbs.game3.app".
 * - The place is the only part ever cut (in its middle, by its line): the
 *   words beside it keep their width.
 * - The action's line is one line. Only when the action ALONE is wider than
 *   the row (two English words at the largest text size, beside a figure)
 *   does it take a second — between its own words ([TitleLine.wraps]).
 * - A title whose action is not known ([TitlePlace.intent] empty) breaks at
 *   an edge of the place: the words on one side take their own line; with
 *   words on both sides the place stays with the shorter side.
 */
internal object TitleLines {
    fun of(parts: TitlePlace, wide: Int, width: (String) -> Int): List<TitleLine> {
        val lead = if (parts.lead.isNotEmpty() && parts.gapBefore) parts.lead + " " else parts.lead
        val trail = if (parts.trail.isNotEmpty() && parts.gapAfter) " " + parts.trail else parts.trail
        if (width(lead + parts.place + trail) <= wide) return listOf(TitleLine(lead, parts.place, trail))
        fun words(text: String) = TitleLine(text, null, "", wraps = width(text) > wide)
        val action = parts.intent.takeIf { it.isNotBlank() }
        val inLead = action?.let { lead.lastIndexOf(it) }?.takeIf { it >= 0 }
        val inTrail = action?.let { trail.indexOf(it) }?.takeIf { it >= 0 }
        return when {
            // The action first: it ends its line, and what follows it goes down with the place.
            inLead != null -> {
                val cut = inLead + action.length
                listOf(words(lead.substring(0, cut).trimEnd()), TitleLine(lead.substring(cut).trimStart(), parts.place, trail))
            }
            // The place first: what leads up to the action stays with the place.
            inTrail != null -> listOf(TitleLine(lead, parts.place, trail.substring(0, inTrail).trimEnd()), words(trail.substring(inTrail).trimStart()))
            parts.lead.isEmpty() && parts.trail.isEmpty() -> listOf(TitleLine("", parts.place, ""))
            parts.trail.isEmpty() -> listOf(words(parts.lead), TitleLine("", parts.place, ""))
            parts.lead.isEmpty() -> listOf(TitleLine("", parts.place, ""), words(parts.trail))
            width(lead) <= width(trail) -> listOf(TitleLine(lead, parts.place, ""), words(parts.trail))
            else -> listOf(words(parts.lead), TitleLine("", parts.place, trail))
        }
    }
}
