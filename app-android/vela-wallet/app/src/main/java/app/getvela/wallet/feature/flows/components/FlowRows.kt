package app.getvela.wallet.feature.flows.components

import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.marks.RemoteLogo
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.flows.ContactEntryModel
import app.getvela.wallet.feature.flows.FactLead
import app.getvela.wallet.feature.flows.FactRowModel
import app.getvela.wallet.feature.flows.FeeTokenRowModel
import app.getvela.wallet.feature.flows.NetworkRowModel
import app.getvela.wallet.feature.flows.RecipientCardModel
import app.getvela.wallet.feature.flows.StatusChipModel
import app.getvela.wallet.feature.flows.StatusTone
import app.getvela.wallet.feature.wallet.components.TokenIcon

/** The rows of the wallet flows (spec 021 components 9, 12–15, 23). */

/**
 * R1's network row (component 9): the chain, the address on it, and the two
 * things a person does with an address — copy it, or show it.
 *
 * Both actions sit on the row rather than behind it. The point of R1 is that
 * ONE address serves every network, so the fastest path is to copy it from
 * whichever line you happened to look at, without opening anything.
 */
@Composable
fun NetworkRow(
    row: NetworkRowModel,
    modifier: Modifier = Modifier,
    copied: Boolean = false,
    onCopy: () -> Unit = {},
    onQr: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Spec 047: the network's logo from the chain-data endpoint; the code
        // over the brand colour is the fallback (the web's RemoteLogo).
        RemoteLogo(urls = listOfNotNull(row.logoUrl), size = VelaSizing.chainBadge) {
            Box(
                modifier = Modifier
                    .size(VelaSizing.chainBadge)
                    .background(row.badgeColor, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = row.code,
                    // The chain colours are brand fills, dark enough for white in
                    // both appearances — so the mode-invariant white, not fgInverse.
                    color = VelaOnAccent,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xs,
                    maxLines = 1,
                )
            }
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = row.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = row.addressDisplay,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        FlowIconButton(
            icon = if (copied) VelaIcons.Check else VelaIcons.Copy,
            label = row.copyLabel,
            tint = if (copied) colors.successBase else colors.fgMuted,
            onClick = onCopy,
        )
        FlowIconButton(icon = VelaIcons.QrCode, label = row.qrLabel, onClick = onQr)
    }
}

/** A circular tap target for a single glyph. */
@Composable
internal fun FlowIconButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    modifier: Modifier = Modifier,
    tint: androidx.compose.ui.graphics.Color = VelaTheme.colors.fgMuted,
    onClick: () -> Unit = {},
) {
    Box(
        modifier = modifier
            .size(VelaSizing.controlSm)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = tint,
            modifier = Modifier.size(VelaIconSize.md),
        )
    }
}

/**
 * The label-value row (component 15).
 *
 * One component for A2's transaction facts, SD3's confirmation summary, T2's
 * token facts and T3b's chain facts. They differ only in what art the value
 * carries — a chain dot, a token mark, an identicon, or nothing — and in
 * whether the value is copyable, so those are parameters rather than four
 * near-identical rows.
 */
@Composable
fun FactRow(
    fact: FactRowModel,
    modifier: Modifier = Modifier,
    copied: Boolean = false,
    onCopy: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.lg, bottom = if (fact.note != null) VelaSpacing.xs else VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = fact.label,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            maxLines = 1,
        )
        Spacer(modifier = Modifier.weight(1f))
        when (val lead = fact.lead) {
            is FactLead.Dot -> {
                Box(
                    modifier = Modifier
                        .size(VelaIconSize.base)
                        .background(lead.color, CircleShape),
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
            }
            is FactLead.Token -> {
                TokenIcon(mark = lead.mark,
                    inline = true,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
            }
            is FactLead.Identicon -> {
                IdenticonImage(seed = lead.seed, size = VelaIconSize.lg, name = lead.name)
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
            }
            null -> Unit
        }
        Text(
            text = fact.value,
            color = colors.fgBase,
            fontFamily = if (fact.mono) VelaMonoFontFamily else VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.base,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.End,
        )
        fact.copy?.let { label ->
            Spacer(modifier = Modifier.width(VelaSpacing.sm))
            Icon(
                imageVector = if (copied) VelaIcons.Check else VelaIcons.Copy,
                contentDescription = label,
                tint = if (copied) colors.successBase else colors.fgSubtle,
                modifier = Modifier
                    .size(VelaIconSize.sm)
                    .clickable(onClick = onCopy),
            )
        }
    }
    // The reason under its row, in the row's own quiet voice (issue 686): a
    // fact about the value, not a warning about it.
    fact.note?.let { note ->
        Text(
            text = note,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(bottom = VelaSpacing.lg),
        )
    }
    }
}

/**
 * The small status pill (component 23): A2's 已确认, T3's 已添加, T3b's
 * 兼容 / 不兼容.
 *
 * Four tones off the semantic colour pairs, so a chip never invents a colour —
 * and never uses the accent, which in this product means "this moves money",
 * not "this is fine".
 */
@Composable
fun StatusChip(chip: StatusChipModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val (bg, fg) = when (chip.tone) {
        StatusTone.Success -> colors.successSoft to colors.successBase
        StatusTone.Warning -> colors.warningSoft to colors.warningBase
        StatusTone.Error -> colors.errorSoft to colors.errorBase
        StatusTone.Info -> colors.infoSoft to colors.infoBase
    }
    Box(
        modifier = modifier
            .background(bg, CircleShape)
            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
    ) {
        Text(
            text = chip.text,
            color = fg,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.xs,
            maxLines = 1,
        )
    }
}

/**
 * SD2b's split row (component 13): one of N people, what they get, and the way
 * to drop them.
 *
 * The ordinal ("Recipient 2") is a label above the name rather than a number
 * beside it, because in a split the ROW is the person and the number is only
 * there to keep three otherwise-similar cards apart.
 */
@Composable
fun RecipientCard(
    recipient: RecipientCardModel,
    modifier: Modifier = Modifier,
    onRemove: () -> Unit = {},
    onAmountChange: ((String) -> Unit)? = null,
    onAddressChange: ((String) -> Unit)? = null,
    /** Spec 048: this row's own 通讯录 pick (the web's `pickContactFor(i)`). */
    onPick: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IdenticonImage(seed = recipient.identiconSeed, size = VelaIconSize.xl2, name = recipient.name)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        if (onPick != null) {
            Icon(
                imageVector = VelaIcons.NavContactsOutline,
                contentDescription = "pick-contact-" + recipient.ordinal,
                tint = colors.fgSubtle,
                modifier = Modifier
                    .size(VelaIconSize.md)
                    .clickable(onClick = onPick),
            )
            Spacer(modifier = Modifier.width(VelaSpacing.md))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = recipient.ordinal,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.xs,
                maxLines = 1,
            )
            if (onAddressChange != null) {
                // A live row: the address is typed (or picked) in place. Local
                // echo as the amount hero does — the machine's view lags fast typing.
                val addressStyle = TextStyle(color = colors.fgBase, fontFamily = VelaMonoFontFamily, fontSize = VelaTextSize.base)
                var typedAddress by remember(recipient.id) { mutableStateOf(recipient.address) }
                val sentAddress = remember(recipient.id) { ArrayDeque<String>().apply { addLast(recipient.address) } }
                LaunchedEffect(recipient.address) { if (recipient.address !in sentAddress) typedAddress = recipient.address }
                BasicTextField(
                    value = typedAddress,
                    onValueChange = { next ->
                        typedAddress = next
                        sentAddress.addLast(next)
                        if (sentAddress.size > 256) sentAddress.removeFirst()
                        onAddressChange(next)
                    },
                    singleLine = true,
                    textStyle = addressStyle,
                    cursorBrush = SolidColor(colors.accentBase),
                    modifier = Modifier.fillMaxWidth().semantics { contentDescription = "recipient-address-${recipient.ordinal}" },
                    decorationBox = { inner ->
                        if (typedAddress.isEmpty()) {
                            Text(text = recipient.addressPlaceholder, style = addressStyle.copy(color = colors.fgSubtle), maxLines = 1)
                        }
                        inner()
                    },
                )
                if (recipient.name.isNotEmpty() && recipient.name != recipient.address) {
                    Text(text = recipient.name, color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.xs, maxLines = 1)
                }
            } else {
                Text(
                    text = recipient.name,
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        val amountStyle = TextStyle(
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            textAlign = TextAlign.End,
        )
        if (onAmountChange != null && recipient.amountValue != null) {
            var typed by remember(recipient.id) { mutableStateOf(recipient.amountValue) }
            val sent = remember(recipient.id) { ArrayDeque<String>().apply { addLast(recipient.amountValue) } }
            LaunchedEffect(recipient.amountValue) { if (recipient.amountValue !in sent) typed = recipient.amountValue }
            BasicTextField(
                value = typed,
                onValueChange = { next ->
                    typed = next
                    sent.addLast(next)
                    if (sent.size > 256) sent.removeFirst()
                    onAmountChange(next)
                },
                singleLine = true,
                textStyle = amountStyle,
                cursorBrush = SolidColor(colors.accentBase),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.width(96.dp).semantics { contentDescription = "recipient-amount-${recipient.ordinal}" },
                decorationBox = { inner ->
                    Box(contentAlignment = Alignment.CenterEnd) {
                        if (typed.isEmpty()) Text(text = "0", style = amountStyle.copy(color = colors.fgSubtle))
                        inner()
                    }
                },
            )
        } else {
            Text(text = recipient.amount, style = amountStyle, maxLines = 1)
        }
        Spacer(modifier = Modifier.width(VelaSpacing.md))
        Icon(
            imageVector = VelaIcons.Close,
            contentDescription = recipient.removeLabel,
            tint = colors.fgSubtle,
            modifier = Modifier
                .size(VelaIconSize.md)
                .clickable(onClick = onRemove),
        )
    }
}

/**
 * SD2f's fee-token row (component 14): a coin that could pay this transfer's
 * fee, what you hold of it, and what the fee would come to.
 *
 * The estimate is per row and not per screen because that is the whole
 * decision: the same transfer costs a different number in each coin, and one
 * figure with a token switcher would hide the comparison.
 */
@Composable
fun FeeTokenRow(
    row: FeeTokenRowModel,
    estimateLabel: String,
    modifier: Modifier = Modifier,
    onSelect: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(
                if (row.selected) colors.bgRaised else androidx.compose.ui.graphics.Color.Transparent,
                RoundedCornerShape(VelaRadius.lg),
            )
            // A coin that cannot cover the fee is drawn for context and
            // answers to nothing (issue 211; the core refuses it anyway).
            .clickable(enabled = !row.insufficient, onClick = onSelect)
            .alpha(if (row.insufficient) VelaOpacity.disabled else 1f)
            .padding(VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TokenIcon(mark = row.mark)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = row.symbol,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
            )
            Text(
                text = row.insufficientNote?.takeIf { row.insufficient } ?: row.balanceLabel,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
            )
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(
                text = row.fee,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
            )
            Text(
                text = estimateLabel,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.xs,
                maxLines = 1,
            )
        }
        // Always laid out, so choosing a row does not shift the ones under it.
        Box(
            modifier = Modifier
                .width(VelaIconSize.lg)
                .padding(start = VelaSpacing.sm),
            contentAlignment = Alignment.Center,
        ) {
            if (row.selected) {
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.accentBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
        }
    }
}

/**
 * SD2e's contact row (component 12).
 *
 * Close cousin of spec 018's `ContactRow`, and deliberately not it: that row
 * MANAGES a contact (swipe to reveal edit and delete, a favourite star, a send
 * count). This one PICKS one, so it carries a chevron and nothing else — every
 * affordance it does not have is one that cannot fire by accident while someone
 * is halfway through a transfer.
 */
@Composable
fun ContactPickRow(
    contact: ContactEntryModel,
    modifier: Modifier = Modifier,
    onSelect: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onSelect)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IdenticonImage(seed = contact.identiconSeed, size = VelaIconSize.xl2, name = contact.name)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = contact.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                contact.group?.let { group ->
                    Spacer(modifier = Modifier.width(VelaSpacing.sm))
                    Box(
                        modifier = Modifier
                            .background(colors.bgRaised, RoundedCornerShape(VelaRadius.sm))
                            .padding(horizontal = VelaSpacing.sm),
                    ) {
                        Text(
                            text = group,
                            color = colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.xs,
                            maxLines = 1,
                        )
                    }
                }
            }
            Text(
                text = contact.addressDisplay,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Icon(
            imageVector = VelaIcons.ChevronRight,
            contentDescription = null,
            tint = colors.fgSubtle,
            modifier = Modifier.size(VelaIconSize.sm),
        )
    }
}
