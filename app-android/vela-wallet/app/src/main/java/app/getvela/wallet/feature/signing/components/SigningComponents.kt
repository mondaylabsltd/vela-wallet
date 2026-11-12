package app.getvela.wallet.feature.signing.components

import androidx.compose.runtime.remember
import app.getvela.wallet.core.designsystem.components.KeptRoom
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.Box
import app.getvela.wallet.core.format.amountFieldOf
import app.getvela.wallet.core.format.cleanAmountFieldEdit
import app.getvela.wallet.core.marks.RemoteLogo
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.wrapContentHeight
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaAddressText
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaColors
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
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
import app.getvela.wallet.feature.wallet.components.TokenIcon
import app.getvela.wallet.feature.flows.FactLead

/**
 * The signing sheet's small parts (spec 022 §3). They are grouped in one file
 * because none of them is meaningful alone — they exist to be assembled in the
 * order a scenario dictates.
 */

internal fun SigningTone.color(colors: VelaColors): Color = when (this) {
    SigningTone.Neutral -> colors.fgBase
    SigningTone.Accent -> colors.fgBase
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
    iconUrls: List<String> = emptyList(),
    networkLogoUrl: String? = null,
    /**
     * Spec 079: the sheet's one explicit close. It no longer closes on a swipe,
     * a tap outside or Back (owner ruling), so the close has to be drawn.
     */
    onClose: (() -> Unit)? = null,
    closeLabel: String = "",
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        // The site's own icon; its initial until one lands, and when it has none.
        RemoteLogo(urls = iconUrls, size = ExploreMetrics.signingAvatar) {
            LetterAvatar(letter, tint, size = ExploreMetrics.signingAvatar)
        }
        // Issue #438: the chain's chip sits under the name, as on iOS since
        // spec 082 RE13. Inline, the chip and the ✕ left the name ~100 dp, and
        // a long name broke over two lines at the smallest system font; here
        // the column has the whole width between the avatar and the ✕.
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            // Spec 082 RE13 (G12): two lines each and never an ellipsis — the
            // end of a host is the part a spoofer controls, so it is never the
            // part cut off. Whether the host is said at all is the core's
            // (`browserSiteLabel`): an empty [host] means the name IS it.
            Text(
                text = name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
                maxLines = 2,
                overflow = TextOverflow.Clip,
            )
            if (host.isNotEmpty()) {
                Text(
                    text = host,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 2,
                    overflow = TextOverflow.Clip,
                )
            }
            Row(
                modifier = Modifier
                    .padding(top = VelaSpacing.sm)
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
                    maxLines = 1,
                )
            }
        }
        onClose?.let { SigningClose(it, closeLabel) }
    }
}

/**
 * The header of the wallet's OWN request (the core's `first_party` — the key
 * backup). Nobody else is asking, so there is no mark, no "Vela Wallet" and no
 * network chip (the rows say the network): ONE row, what the request does as
 * the sheet's title, and the ✕ — the sheet's only exit (spec 079), drawn in
 * every mode that draws a header: the form, the receipt, the aftercare. The
 * title wraps beside the ✕, never under it.
 */
@Composable
fun SigningOwnHeader(
    headline: String,
    modifier: Modifier = Modifier,
    onClose: (() -> Unit)? = null,
    closeLabel: String = "",
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(
            text = headline,
            color = VelaTheme.colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
            modifier = Modifier
                .weight(1f)
                // One line sits centred on the ✕; more lines grow the row.
                .heightIn(min = VelaSizing.hitTarget)
                .wrapContentHeight(Alignment.CenterVertically)
                .semantics { heading() },
        )
        onClose?.let { SigningClose(it, closeLabel) }
    }
}

/** The header's ✕, in a hit-target box: the same place on a site's sheet and the wallet's own. */
@Composable
private fun SigningClose(onClose: () -> Unit, label: String) {
    Box(
        modifier = Modifier
            .size(VelaSizing.hitTarget)
            .testTag("signing-close")
            .clickable(onClick = onClose),
        contentAlignment = Alignment.Center,
    ) { Icon(VelaIcons.Close, label, tint = VelaTheme.colors.fgMuted) }
}

/**
 * The intent: the eyebrow over a hero figure. (The wallet's own request says
 * its intent as the header's title instead — [SigningOwnHeader].)
 */
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
                line.token?.let { TokenIcon(mark = it, inline = true) }
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
                if (row.mono) {
                    VelaAddressText(
                        text = row.value,
                        color = row.valueTone.color(colors),
                        fontSize = VelaTextSize.base,
                        textAlign = TextAlign.End,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                } else {
                    Text(
                        text = row.value,
                        color = row.valueTone.color(colors),
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                        textAlign = TextAlign.End,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                }
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
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            // The name takes the room the badge leaves, and wraps in it: an
            // unweighted name ("1inch Aggregation Router · 1inch Network")
            // took the whole row and squeezed 已验证 into a column of one
            // character per line.
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                )
                address?.let {
                    VelaAddressText(
                        text = it,
                        color = colors.fgMuted,
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
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier
                        .background(fill, RoundedCornerShape(VelaRadius.sm))
                        .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                )
            }
        }
    }
}

@Composable
fun SigningWarning(
    tone: SigningTone,
    text: String,
    modifier: Modifier = Modifier,
    /** dapp-15: "Check again" (or, while it runs, "Checking again…") under the line. */
    recheck: app.getvela.wallet.feature.signing.SigningBlock.Recheck? = null,
    onRecheck: () -> Unit = {},
    /**
     * R22: under a check one node answered alone — how to check with a node
     * the person trusts (`componentsUi.signing.ownNodeHint`); a tap opens
     * this network's RPC settings, where "Only use my node" is ([onHint]).
     */
    hint: String? = null,
    onHint: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val danger = tone == SigningTone.Danger
    if (hint != null) {
        Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            SigningWarning(tone, text, recheck = recheck, onRecheck = onRecheck)
            Text(
                text = hint,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline,
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(VelaRadius.sm))
                    .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onHint)
                    .padding(vertical = VelaSpacing.xs)
                    .testTag(app.getvela.wallet.feature.signing.OWN_NODE_HINT_TAG),
            )
        }
        return
    }
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
        if (recheck == null) {
            Text(text = text, color = ink, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
                // While a new check is out, the line it will replace is stale.
                Text(
                    text = text, color = ink, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base,
                    modifier = if (recheck.checking) Modifier.alpha(VelaOpacity.disabled) else Modifier,
                )
                if (recheck.checking) {
                    Text(text = recheck.label, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
                } else {
                    Text(
                        text = recheck.label,
                        color = ink,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        fontWeight = VelaFontWeight.semibold,
                        modifier = Modifier
                            .clip(RoundedCornerShape(VelaRadius.sm))
                            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onRecheck)
                            .padding(vertical = VelaSpacing.xs),
                    )
                }
            }
        }
    }
}

/**
 * cs-14: where a reading came from — one quiet line in its tone's ink, no
 * card (the web's `source` block): neutral muted, a caution or a danger in
 * theirs.
 */
@Composable
fun SigningSource(text: String, tone: SigningTone, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Text(
        text = text,
        color = when (tone) {
            SigningTone.Danger -> colors.errorBase
            SigningTone.Caution -> colors.warningBase
            else -> colors.fgMuted
        },
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * The person's AI agent's opinion (spec 103, `vela_advise`) — the web's
 * `AgentAdviceCard`: a card of its own under Vela's lines and over the fee,
 * labelled as the agent's, never one of the sheet's words. It colours
 * nothing but its chip, and never green: a reassurance is the wallet's to
 * give. Plain text only — the core already stripped what could hide or
 * reorder it; the sources are hosts, never links. Given for an earlier
 * version of the request, or at a fee that has since moved, it is greyed
 * and says which.
 */
@Composable
fun AgentAdviceCard(card: app.getvela.wallet.feature.signing.AdviceModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val shape = RoundedCornerShape(VelaRadius.xl)
    val dashColor = colors.borderStrong
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, shape)
            .drawBehind {
                val stroke = VelaBorder.emphasis.toPx()
                val dash = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))
                drawRoundRect(
                    color = dashColor,
                    cornerRadius = androidx.compose.ui.geometry.CornerRadius(VelaRadius.xl.toPx()),
                    style = androidx.compose.ui.graphics.drawscope.Stroke(width = stroke, pathEffect = dash),
                )
            }
            .semantics(mergeDescendants = false) { contentDescription = card.title }
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Text(
                text = card.title,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                fontWeight = VelaFontWeight.semibold,
                modifier = Modifier.weight(1f).semantics { heading() },
            )
            val (chipBg, chipBorder, chipInk) = when (card.verdict) {
                app.getvela.wallet.feature.signing.SigningAdvice.Verdict.Caution -> Triple(colors.warningSoft, colors.warningBorder, colors.warningBase)
                app.getvela.wallet.feature.signing.SigningAdvice.Verdict.Danger -> Triple(colors.errorSoft, colors.errorBase, colors.errorBase)
                app.getvela.wallet.feature.signing.SigningAdvice.Verdict.Unsure -> Triple(Color.Transparent, colors.borderStrong, colors.fgMuted)
                app.getvela.wallet.feature.signing.SigningAdvice.Verdict.LooksFine -> Triple(Color.Transparent, colors.borderStrong, colors.fgBase)
            }
            Text(
                text = card.verdictWord,
                color = chipInk,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                fontWeight = VelaFontWeight.medium,
                modifier = Modifier
                    .then(if (card.stale) Modifier.alpha(VelaOpacity.dim) else Modifier)
                    .background(chipBg, RoundedCornerShape(VelaRadius.full))
                    .border(VelaBorder.hairline, chipBorder, RoundedCornerShape(VelaRadius.full))
                    .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
            )
        }
        if (card.stale) {
            Text(text = card.staleLine, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
        }
        card.feeLine?.let { Text(text = it, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm) }
        Column(
            modifier = if (card.stale) Modifier.alpha(VelaOpacity.dim) else Modifier,
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            Text(text = card.summary, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
            card.points.forEach { point ->
                Text(text = "•  $point", color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
            }
            if (card.hosts.isNotEmpty()) {
                Text(
                    text = "${card.sourcesLabel}: ${card.hosts.joinToString(", ")}",
                    color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm,
                )
            }
        }
        Text(text = card.footer, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.xs)
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
fun SigningCode(
    lines: List<String>,
    note: String?,
    modifier: Modifier = Modifier,
    runs: List<List<app.getvela.wallet.feature.signing.CodeRun>>? = null,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
    ) {
        if (runs != null) {
            // The message as the core drew it: a hidden character's marker and
            // a folded run of empty lines muted, the text in the base ink.
            runs.forEach { line ->
                Text(
                    text = androidx.compose.ui.text.buildAnnotatedString {
                        line.forEach { run ->
                            if (run.muted) {
                                pushStyle(androidx.compose.ui.text.SpanStyle(color = colors.fgMuted))
                                append(run.text)
                                pop()
                            } else {
                                append(run.text)
                            }
                        }
                    },
                    color = colors.fgBase,
                    fontFamily = FontFamily.Monospace,
                    fontSize = VelaTextSize.base,
                )
            }
        } else {
            lines.forEach {
                Text(
                    text = it,
                    color = colors.fgBase,
                    fontFamily = FontFamily.Monospace,
                    fontSize = VelaTextSize.base,
                )
            }
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
    /** R22: the core's `source_key` line ("Checked by your chosen node only"), or `null`. */
    source: String? = null,
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
                    // §41: a row with no figure (a grant, a change Vela can't
                    // read) says its tone in its words — there is no figure to.
                    color = if (row.delta.isEmpty() && row.tone != SigningTone.Neutral) row.tone.color(colors) else colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
                // The amount takes the rest of the row and keeps to its end,
                // however long it is. (An unverified token's row has no
                // figure at all since PR 3: its sign alone, or nothing.)
                Text(
                    text = row.delta,
                    color = row.tone.color(colors),
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    textAlign = TextAlign.End,
                    modifier = Modifier.weight(1f).padding(start = VelaSpacing.lg),
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
        // R22: who stood behind the check, when it was the person's own node
        // alone ("Checked by your chosen node only") — quiet, under the rows.
        source?.let {
            Text(
                text = it,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier
                    .padding(top = VelaSpacing.xs, bottom = VelaSpacing.md)
                    .testTag(app.getvela.wallet.feature.signing.SIM_SOURCE_TAG),
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
                                if (selected) colors.fgBase else colors.borderStrong,
                                CircleShape,
                            )
                            .alpha(if (disabled) VelaOpacity.disabled else 1f)
                            .clickable(enabled = !disabled) { onChip(chip.id) }
                            .padding(horizontal = VelaSpacing.lg),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = chip.label,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
            }
            custom?.let { input ->
                // Spec 044: the guard's custom amount — local echo, the core's
                // text when it differs (the send form's rule).
                var typed by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(amountFieldOf(input.value)) }
                androidx.compose.runtime.LaunchedEffect(input.value) { if (input.value != typed.text && input.value.isEmpty()) typed = amountFieldOf(input.value) }
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
                        // raw "4,5" allowed 45 — cleaned by the core's rule
                        // first, caret and all (issue #421: "08" is 8).
                        onValueChange = { next ->
                            cleanAmountFieldEdit(next, typed)?.let { edit ->
                                val changed = edit.text != typed.text
                                typed = edit
                                if (changed) onCustomAmount(edit.text)
                            }
                        },
                        singleLine = true,
                        textStyle = androidx.compose.ui.text.TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg, fontFeatureSettings = app.getvela.wallet.core.designsystem.tokens.VelaFontFeatures),
                        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = androidx.compose.ui.text.input.KeyboardType.Decimal),
                        modifier = Modifier.weight(1f),
                        decorationBox = { inner ->
                            if (typed.text.isEmpty()) Text(text = input.placeholder, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg)
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
                        when (val lead = identity.lead) {
                            // On the grey card: ringed, or the disc is the card's colour.
                            is FactLead.Token -> TokenIcon(mark = lead.mark, inline = true, ring = true)
                            is FactLead.Identicon -> IdenticonAvatar(seed = lead.seed, size = VelaIconSize.xl, tappable = false)
                            is FactLead.Dot, null -> Unit
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
                            VelaAddressText(
                                text = identity.address,
                                color = colors.fgBase,
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
 * The Trusted Signer's page is open (spec 071): what to do there, a way back to
 * it (a tab closed by mistake), and a way out. It stands where the confirm was —
 * the signature is being made on the page, not here.
 */
@Composable
fun TrustedSignerWaiting(
    model: app.getvela.wallet.feature.signing.TrustedSignerWaitModel,
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
        // Spec 102: a key ceremony names its key, as the page does.
        model.key?.let { key -> KeyRow(key) }
        if (model.hint.isNotBlank()) {
            Text(model.hint, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
        }
        app.getvela.wallet.core.designsystem.components.VelaPrimaryButton(model.reopen, onReopen, Modifier.fillMaxWidth())
        app.getvela.wallet.core.designsystem.components.VelaSecondaryButton(model.cancel, onCancel, Modifier.fillMaxWidth())
    }
}

/**
 * Spec 102 D4: the hand-off card. The account reviews and signs on a page, so
 * this card says only what Vela knows and the page cannot: where it goes (the
 * page's address), which key the person will confirm with (the "Confirm with
 * | …" row), what this operation was priced at only when the screen shows no
 * fee of its own (one quiet row, no control: the fee was chosen before the
 * hand-off; a fee is on screen once), and what is trusted about the page —
 * its integrity line, or why it will not open, with "Trust this version"
 * under a self-hosted page's question. Open is the consent that goes there,
 * enabled only when [enabled] (the line opens and the request may be
 * confirmed). D7: the sheet's own palette and rows, one accent — the button.
 */
@Composable
fun HandoffCard(
    model: app.getvela.wallet.feature.signing.HandoffModel,
    enabled: Boolean,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
    onTrust: () -> Unit = {},
    /** R22: the sheet's confirm friction ([app.getvela.wallet.feature.signing.SigningScreenModel.confirmHoldMs]). */
    holdMs: Int = 0,
    danger: Boolean = false,
    holdHint: String = "",
    confirmAgain: String = "",
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            Icon(
                imageVector = app.getvela.wallet.core.designsystem.components.VelaIcons.Eye,
                contentDescription = null,
                tint = colors.fgMuted,
                modifier = Modifier.padding(top = VelaSpacing.xs).size(VelaIconSize.base),
            )
            Text(
                model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
        }
        model.key?.let { key -> KeyRow(key) }
        model.fee?.let { fee -> HandoffFeeRow(fee) }
        // The page and what was checked about it, together (as iOS draws
        // them): its NAME — 「Vela 官方签名页」, the person's label,
        // "Self-hosted · domain" — its address where the name does not already
        // say it, and its integrity line. The card used to name the page by
        // its host alone, under the title.
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                model.pageName,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.base,
            )
            if (!model.pageName.contains(model.page)) {
                Text(
                    model.page,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            app.getvela.wallet.feature.settings.components.IntegrityLine(model.integrity)
            model.trust?.let { label ->
                app.getvela.wallet.feature.settings.components.TrustAnswer(
                    label,
                    modifier = Modifier.padding(top = VelaSpacing.sm),
                    indent = false,
                    onClick = onTrust,
                )
            }
        }
        // Opening the page is where this signature is given (web HandoffCard `value`).
        // R22: Open IS the sheet's confirm (spec 102 D4) — the same friction:
        // "… anyway" / danger in the danger style, held for `hold_ms`.
        app.getvela.wallet.core.designsystem.components.VelaHoldToConfirmButton(
            text = model.open,
            onConfirm = onOpen,
            holdMs = holdMs,
            holdHint = holdHint,
            confirmAgain = confirmAgain,
            danger = danger,
            enabled = enabled,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * Spec 102: a key row — "Confirm with | Phone or tablet", "New key on | This
 * device" — drawn as the sheet's [SignerRow] ("Signing account | name") is:
 * the label muted, the value in the body colour at its end, the same size.
 * A long value wraps at the end rather than pushing the label away.
 */
@Composable
fun KeyRow(model: app.getvela.wallet.feature.signing.KeyRowModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth().semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(
            text = model.label,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Text(
            text = model.value,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = androidx.compose.ui.text.style.TextAlign.End,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * The card's fee + speed row: the label, and what this operation was priced
 * at with its speed's name — the sheet's fee row, folded to one line with no
 * control on it, set like the key row above it (label | value).
 */
@Composable
private fun HandoffFeeRow(model: app.getvela.wallet.feature.signing.HandoffFeeModel) {
    val colors = VelaTheme.colors
    // The fee's worth joins this line when the display currency commits:
    // two lines' room from the first frame, kept, so the card's Open does
    // not move when it lands.
    val worthRoom = remember { KeptRoom() }.keep(model.worthRoom)
    Row(
        modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(model.label, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
        Text(
            text = androidx.compose.ui.text.buildAnnotatedString {
                append(model.value)
                model.tier?.let { tier ->
                    withStyle(androidx.compose.ui.text.SpanStyle(color = colors.fgMuted)) { append(" · $tier") }
                }
            },
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = androidx.compose.ui.text.style.TextAlign.End,
            minLines = if (worthRoom) 2 else 1,
            modifier = Modifier.weight(1f),
        )
    }
}
