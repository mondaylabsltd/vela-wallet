package app.getvela.wallet.feature.signing.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.Box
import androidx.compose.ui.draw.rotate
import app.getvela.wallet.core.format.cleanAmountEdit
import app.getvela.wallet.feature.signing.SignWithModel
import app.getvela.wallet.core.designsystem.components.VelaLogo
import app.getvela.wallet.core.marks.RemoteLogo
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaColors
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.components.ExploreMetrics
import app.getvela.wallet.feature.explore.components.LetterAvatar
import app.getvela.wallet.feature.signing.AllowanceChip
import app.getvela.wallet.feature.signing.AmountLine
import app.getvela.wallet.feature.signing.BalanceDeltaRow
import app.getvela.wallet.feature.signing.PartyBadge
import app.getvela.wallet.feature.signing.SigningRow
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.TechModel
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/**
 * The signing sheet's small parts (spec 022 §3). They are grouped in one file
 * because none of them is meaningful alone — they exist to be assembled in the
 * order a scenario dictates.
 */

internal fun SigningTone.color(colors: VelaColors): Color = when (this) {
    SigningTone.Neutral -> colors.fgBase
    SigningTone.Accent -> colors.accentBase
    SigningTone.Success -> colors.successBase
    SigningTone.Caution -> colors.warningBase
    SigningTone.Danger -> colors.errorBase
}

@Composable
fun SigningHeader(
    name: String,
    host: String,
    letter: String,
    tint: Color,
    networkName: String,
    networkDot: Color,
    modifier: Modifier = Modifier,
    own: Boolean = false,
    iconUrls: List<String> = emptyList(),
    networkLogoUrl: String? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        if (own) {
            // The wallet asking itself: its own mark, never a letter on a disc.
            Box(
                Modifier.size(ExploreMetrics.signingAvatar).background(colors.bgSunken, CircleShape),
                contentAlignment = Alignment.Center,
            ) { VelaLogo(darkTheme = VelaTheme.isDark, modifier = Modifier.size(ExploreMetrics.signingAvatar * 0.6f)) }
        } else {
            // The site's own icon; its initial until one lands, and when it has none.
            RemoteLogo(urls = iconUrls, size = ExploreMetrics.signingAvatar) {
                LetterAvatar(letter, tint, size = ExploreMetrics.signingAvatar)
            }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (host.isNotEmpty()) {
                Text(
                    text = host,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
            }
        }
        Row(
            modifier = Modifier
                .height(ExploreMetrics.networkChip)
                .background(colors.bgSunken, CircleShape)
                .padding(horizontal = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            // The chain's logo; the drawn dot until it lands, and when there is none.
            RemoteLogo(urls = listOfNotNull(networkLogoUrl), size = VelaSpacing.xl) {
                Box(Modifier.size(VelaSpacing.xl), contentAlignment = Alignment.Center) {
                    Box(Modifier.size(VelaSpacing.md).background(networkDot, CircleShape))
                }
            }
            Text(
                text = networkName,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
    }
}

@Composable
fun SigningIntent(text: String, tone: SigningTone, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Text(
        text = text,
        color = if (tone == SigningTone.Neutral) colors.fgMuted else tone.color(colors),
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = modifier,
    )
}

/**
 * The plain-language line is the one thing a hurried person reads, so it
 * carries the tone of the transaction rather than staying neutral.
 */
@Composable
fun SigningSentence(text: String, tone: SigningTone, modifier: Modifier = Modifier) {
    Text(
        text = text,
        color = tone.color(VelaTheme.colors),
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.lg,
        modifier = modifier.fillMaxWidth(),
    )
}

@Composable
fun SigningAmount(
    line: AmountLine,
    modifier: Modifier = Modifier,
    card: Boolean = false,
    note: String? = null,
    compact: Boolean = false,
) {
    val colors = VelaTheme.colors
    val ink = if (line.tone == SigningTone.Neutral) colors.fgBase else line.tone.color(colors)
    val body = @Composable {
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            if (line.caption != null && !card) {
                Text(
                    text = line.caption,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Text(
                    text = "${line.sign}${line.value}",
                    color = ink,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = when {
                        card -> VelaTextSize.xl2
                        compact -> VelaTextSize.xl3
                        else -> VelaTextSize.xl4
                    },
                    maxLines = 1,
                )
                line.token?.let {
                    LetterAvatar(it.letter, it.tint, size = ExploreMetrics.tokenMark)
                }
                Text(
                    text = line.symbol,
                    color = if (line.tone == SigningTone.Neutral) colors.fgMuted else ink,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                )
            }
            (note ?: line.fiat)?.let {
                Text(
                    text = it,
                    color = if (note != null) colors.fgMuted else colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }

    if (card) {
        Box(
            modifier = modifier
                .fillMaxWidth()
                .background(
                    if (line.tone == SigningTone.Danger) colors.errorSoft else colors.bgSunken,
                    RoundedCornerShape(VelaRadius.xl),
                )
                .border(
                    VelaBorder.hairline,
                    if (line.tone == SigningTone.Danger) colors.errorBase else Color.Transparent,
                    RoundedCornerShape(VelaRadius.xl),
                )
                .padding(VelaSpacing.xl),
        ) { body() }
    } else {
        Box(modifier.fillMaxWidth()) { body() }
    }
}

@Composable
fun SigningSwapPair(pay: AmountLine, receive: AmountLine, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
        SigningAmount(pay, compact = true)
        Box(
            modifier = Modifier
                .size(VelaSpacing.xl4)
                .background(colors.bgSunken, CircleShape),
            contentAlignment = Alignment.Center,
        ) { Icon(VelaIcons.ArrowDown, null, tint = colors.fgMuted) }
        SigningAmount(receive, compact = true)
    }
}

@Composable
fun SigningNftHero(id: String, collection: String, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
        Text(
            text = id,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl4,
        )
        Text(
            text = collection,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
    }
}

@Composable
fun SigningRows(rows: List<SigningRow>, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier.fillMaxWidth()) {
        rows.forEach { row ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.lg),
                verticalAlignment = Alignment.Top,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = row.label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Spacer(Modifier.width(VelaSpacing.lg))
                Text(
                    text = row.value,
                    color = row.valueTone.color(colors),
                    fontFamily = if (row.mono) FontFamily.Monospace else VelaFontFamily,
                    fontWeight = if (row.mono) VelaFontWeight.regular else VelaFontWeight.semibold,
                    fontSize = VelaTextSize.base,
                    textAlign = TextAlign.End,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
        }
    }
}

@Composable
fun SigningParty(
    label: String,
    name: String,
    address: String?,
    badge: PartyBadge?,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
        Text(
            text = label,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                )
                address?.let {
                    Text(
                        text = it,
                        color = colors.fgMuted,
                        fontFamily = FontFamily.Monospace,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            badge?.let {
                val ink = if (it.tone == SigningTone.Neutral) colors.fgMuted
                else it.tone.color(colors)
                val fill = when (it.tone) {
                    SigningTone.Success -> colors.successSoft
                    SigningTone.Caution -> colors.warningSoft
                    SigningTone.Danger -> colors.errorSoft
                    else -> colors.bgSunken
                }
                Text(
                    text = it.text,
                    color = ink,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    modifier = Modifier
                        .background(fill, RoundedCornerShape(VelaRadius.sm))
                        .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                )
            }
        }
    }
}

@Composable
fun SigningWarning(tone: SigningTone, text: String, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val danger = tone == SigningTone.Danger
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(
                if (danger) colors.errorSoft else colors.warningSoft,
                RoundedCornerShape(VelaRadius.lg),
            )
            .border(
                VelaBorder.hairline,
                if (danger) colors.errorBase else colors.warningBorder,
                RoundedCornerShape(VelaRadius.lg),
            )
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        val ink = if (danger) colors.errorBase else colors.warningBase
        Icon(VelaIcons.TriangleAlert, null, tint = ink, modifier = Modifier.size(VelaIconSize.base))
        Text(text = text, color = ink, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
    }
}

@Composable
fun SigningPositive(text: String, modifier: Modifier = Modifier, quiet: Boolean = false) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(
                if (quiet) {
                    Modifier
                } else {
                    Modifier
                        .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                        .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg)
                },
            ),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        val ink = if (quiet) colors.successBase else colors.fgBase
        Icon(VelaIcons.Check, null, tint = ink, modifier = Modifier.size(VelaIconSize.sm))
        Text(text = text, color = ink, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
    }
}

@Composable
fun SigningCode(lines: List<String>, note: String?, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
    ) {
        lines.forEach {
            Text(
                text = it,
                color = colors.fgBase,
                fontFamily = FontFamily.Monospace,
                fontSize = VelaTextSize.base,
            )
        }
        note?.let {
            Text(
                text = it,
                color = colors.fgMuted,
                fontFamily = FontFamily.Monospace,
                fontSize = VelaTextSize.base,
            )
        }
    }
}

@Composable
fun SigningCard(
    title: String?,
    rows: List<SigningRow>,
    tone: SigningTone,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(
                if (tone == SigningTone.Danger) colors.errorSoft else Color.Transparent,
                RoundedCornerShape(VelaRadius.xl),
            )
            .border(
                VelaBorder.hairline,
                if (tone == SigningTone.Danger) colors.errorBase else colors.borderBase,
                RoundedCornerShape(VelaRadius.xl),
            )
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.sm),
    ) {
        title?.let {
            Text(
                text = it,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(vertical = VelaSpacing.md),
            )
        }
        SigningRows(rows)
    }
}

/**
 * The simulation's own account of what moves. It is the ONE part of a signing
 * sheet a malicious site cannot author, which is why the deeper degradation
 * rungs promote it from footnote to protagonist.
 */
@Composable
fun SigningBalances(
    title: String,
    rows: List<BalanceDeltaRow>,
    note: String?,
    noteTone: SigningTone,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .border(
                VelaBorder.hairline, colors.borderBase,
                RoundedCornerShape(VelaRadius.xl),
            )
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
    ) {
        Text(
            text = title,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(vertical = VelaSpacing.md),
        )
        rows.forEach { row ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.sm),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = row.symbol,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = row.delta,
                    color = row.tone.color(colors),
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
            }
        }
        note?.let {
            Text(
                text = it,
                color = if (noteTone == SigningTone.Neutral) colors.fgSubtle
                else noteTone.color(colors),
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(top = VelaSpacing.md, bottom = VelaSpacing.md),
            )
        }
    }
}

/**
 * The approval editor (spec 022 §4, never-unlimited mandate). The `requested`
 * chip is DISABLED whenever the request is unlimited — not merely unselected.
 * A wallet that renders "unlimited" as one tap among four has made the
 * dangerous choice the easy one.
 */
@Composable
fun AllowanceEditor(
    label: String,
    value: String,
    valueTone: SigningTone,
    chips: List<AllowanceChip>,
    note: String?,
    resultingTotal: SigningRow?,
    modifier: Modifier = Modifier,
    custom: app.getvela.wallet.feature.signing.AllowanceInput? = null,
    onChip: (String) -> Unit = {},
    onCustomAmount: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .border(
                    VelaBorder.hairline, colors.borderBase,
                    RoundedCornerShape(VelaRadius.xl),
                )
                .padding(VelaSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Text(
                    text = value,
                    color = if (valueTone == SigningTone.Neutral) colors.fgBase
                    else valueTone.color(colors),
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl2,
                )
            }
            androidx.compose.foundation.layout.FlowRow(
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                chips.forEach { chip ->
                    val selected = chip.state == AllowanceChip.ChipState.Selected
                    val disabled = chip.state == AllowanceChip.ChipState.Disabled
                    Box(
                        modifier = Modifier
                            .height(VelaSizing.controlSm)
                            .border(
                                VelaBorder.hairline,
                                if (selected) colors.accentBase else colors.borderStrong,
                                CircleShape,
                            )
                            .alpha(if (disabled) VelaOpacity.disabled else 1f)
                            .clickable(enabled = !disabled) { onChip(chip.id) }
                            .padding(horizontal = VelaSpacing.lg),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = chip.label,
                            color = if (selected) colors.accentBase else colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
            }
            custom?.let { input ->
                // Spec 044: the guard's custom amount — local echo, the core's
                // text when it differs (the send form's rule).
                var typed by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(input.value) }
                androidx.compose.runtime.LaunchedEffect(input.value) { if (input.value != typed && input.value.isEmpty()) typed = input.value }
                androidx.compose.foundation.layout.Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(VelaBorder.hairline, if (input.error != null) colors.errorBase else colors.borderStrong, RoundedCornerShape(VelaRadius.md))
                        .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    androidx.compose.foundation.text.BasicTextField(
                        value = typed,
                        // Spec 073: the cap's parser drops every comma, so a
                        // raw "4,5" allowed 45 — cleaned by the core's rule first.
                        onValueChange = { next ->
                            cleanAmountEdit(next, typed)?.let { clean ->
                                typed = clean
                                onCustomAmount(clean)
                            }
                        },
                        singleLine = true,
                        textStyle = androidx.compose.ui.text.TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg),
                        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = androidx.compose.ui.text.input.KeyboardType.Decimal),
                        modifier = Modifier.weight(1f),
                        decorationBox = { inner ->
                            if (typed.isEmpty()) Text(text = input.placeholder, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg)
                            inner()
                        },
                    )
                    Text(text = input.symbol, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
                }
                input.error?.let { Text(text = it, color = colors.errorBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm) }
            }
            note?.let {
                Text(
                    text = it,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
        resultingTotal?.let { SigningRows(listOf(it)) }
    }
}

/**
 * The universal fallback renderer (SPEC 签名 · 技术细节): function → parameters
 * → address identities → simulation → raw data. Five fixed layers that can
 * render ANY request, folded away by default and never removed.
 */
@Composable
fun TechDetails(
    tech: TechModel,
    open: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(
                if (open) colors.bgSunken else Color.Transparent,
                RoundedCornerShape(VelaRadius.xl),
            ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onToggle)
                .padding(horizontal = if (open) VelaSpacing.xl else VelaSpacing.none)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Icon(
                if (open) VelaIcons.ChevronDown else VelaIcons.ChevronRight,
                null,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.sm),
            )
            Text(
                text = tech.summary?.let { "${tech.title} · $it" } ?: tech.title,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }

        if (open) {
            Column(
                modifier = Modifier.padding(
                    horizontal = VelaSpacing.xl,
                    vertical = VelaSpacing.md,
                ),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                tech.functionLabel?.let { label ->
                    Text(
                        text = label,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                    tech.signature?.let {
                        Text(
                            text = it,
                            color = colors.fgBase,
                            fontFamily = FontFamily.Monospace,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
                if (tech.params.isNotEmpty()) SigningRows(tech.params)
                tech.identities.forEach { identity ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                    ) {
                        identity.mark?.let {
                            LetterAvatar(it.letter, it.tint, size = VelaSpacing.xl3)
                        }
                        Column(
                            Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
                        ) {
                            Text(
                                text = "${identity.role} · ${identity.name}",
                                color = colors.fgSubtle,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.sm,
                            )
                            Text(
                                text = identity.address,
                                color = colors.fgBase,
                                fontFamily = FontFamily.Monospace,
                                fontSize = VelaTextSize.base,
                            )
                        }
                        Icon(
                            VelaIcons.Copy, tech.copyLabel, tint = colors.fgMuted,
                            modifier = Modifier.size(VelaIconSize.sm),
                        )
                        Icon(
                            VelaIcons.ExternalLink, tech.explorerLabel, tint = colors.fgMuted,
                            modifier = Modifier.size(VelaIconSize.sm),
                        )
                    }
                }
                tech.simResult?.let { SigningRows(listOf(it)) }
                tech.rawLabel?.let { label ->
                    Text(
                        text = label,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                    tech.rawHex?.let {
                        Text(
                            text = it,
                            color = colors.fgMuted,
                            fontFamily = FontFamily.Monospace,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun SignerRow(label: String, name: String, seed: String, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = label,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            IdenticonAvatar(seed = seed, size = VelaIconSize.base)
            Text(
                text = name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
    }
}


/**
 * "Sign with · Automatic ›" — where the passkey that signs this request is.
 *
 * Creating a wallet and signing in both let a person say whether their key is
 * on this phone, on another device, or on a security key. Signing did not: it
 * took the first key's stored route (founder, 2026-09-19). Per request; the
 * core decides which key the ceremony is pinned to (`sign_route`). Opens in
 * place — a sheet over the signing sheet is a modal under a modal.
 */
@Composable
fun SignWithRow(model: SignWithModel, onSelect: (String?) -> Unit, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        Row(
            modifier = Modifier.fillMaxWidth().clickable { onSelect(null) },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Text(model.label, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base, modifier = Modifier.weight(1f))
            Text(model.value, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
            Icon(
                imageVector = VelaIcons.ChevronDown,
                contentDescription = null,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.sm).rotate(if (model.open) 180f else 0f),
            )
        }
        if (model.open) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .padding(VelaSpacing.sm),
            ) {
                model.options.forEach { option ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(option.id) }
                            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                            Text(
                                option.title,
                                color = if (option.selected) colors.fgBase else colors.fgMuted,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.base,
                            )
                            option.line?.let {
                                Text(it, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
                            }
                        }
                        if (option.selected) {
                            Icon(VelaIcons.Check, contentDescription = null, tint = colors.accentBase, modifier = Modifier.size(VelaIconSize.sm))
                        }
                    }
                }
            }
        }
    }
}

/**
 * The Clear Signer's page is open (spec 071): what to do there, a way back to
 * it (a tab closed by mistake), and a way out. It stands where the slide was —
 * the signature is being made on the page, not here.
 */
@Composable
fun ClearSignerWaiting(
    model: app.getvela.wallet.feature.signing.ClearSignerWaitModel,
    onReopen: () -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(model.title, color = colors.fgBase, fontFamily = VelaFontFamily, fontWeight = VelaFontWeight.semibold, fontSize = VelaTextSize.base)
        Text(model.hint, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
        app.getvela.wallet.core.designsystem.components.VelaPrimaryButton(model.reopen, onReopen, Modifier.fillMaxWidth())
        app.getvela.wallet.core.designsystem.components.VelaSecondaryButton(model.cancel, onCancel, Modifier.fillMaxWidth())
    }
}
