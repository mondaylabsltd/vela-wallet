package app.getvela.wallet.feature.flows

import androidx.compose.foundation.text.BasicTextField
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
import app.getvela.wallet.core.platform.Clipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.flows.components.AddressCard
import app.getvela.wallet.feature.flows.components.AmountHero
import app.getvela.wallet.feature.flows.components.AmountInput
import app.getvela.wallet.feature.flows.components.ContactPickRow
import app.getvela.wallet.feature.flows.components.FactRow
import app.getvela.wallet.feature.flows.components.FeeRow
import app.getvela.wallet.feature.flows.components.FeeTokenRow
import app.getvela.wallet.feature.flows.components.FlowFilterChips
import app.getvela.wallet.feature.flows.components.FlowMonoField
import app.getvela.wallet.feature.flows.components.FlowSearchField
import app.getvela.wallet.feature.flows.components.FlowSegmentedToggle
import app.getvela.wallet.feature.flows.components.GhostPillRow
import app.getvela.wallet.feature.flows.components.HintCard
import app.getvela.wallet.feature.flows.components.NetworkRow
import app.getvela.wallet.feature.flows.components.NoticeBanner
import app.getvela.wallet.feature.flows.components.QrCard
import app.getvela.wallet.feature.flows.components.RecipientCard
import app.getvela.wallet.feature.flows.components.RecipientField
import app.getvela.wallet.feature.flows.components.StatusChip
import app.getvela.wallet.feature.flows.components.StatusHero
import app.getvela.wallet.feature.flows.components.SummaryLine
import app.getvela.wallet.feature.flows.components.TokenHeaderCard
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.AssetRowModel
import app.getvela.wallet.feature.wallet.EmptyStateModel
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.components.AssetRow
import app.getvela.wallet.feature.wallet.components.DayLabel
import app.getvela.wallet.feature.wallet.components.EmptyState
import app.getvela.wallet.feature.wallet.components.TokenIcon

/**
 * The bodies of the wallet flows (spec 021).
 *
 * Every one takes its model and nothing else, so the same body serves the
 * phone screen, the sheet, and the gallery. Chrome — the scaffold, the sheet —
 * is the caller's business.
 */

/** The 150 ms copy tick the SPEC sheet specifies, as reusable screen state. */
@Composable
private fun rememberCopyTick(): Pair<Int, (Int) -> Unit> {
    var copied by remember { mutableStateOf(-1) }
    return copied to { index -> copied = index }
}

// ------------------------------------------------------------------ receive

/**
 * R1 — the receive network list.
 *
 * The subtitle is the whole idea: one address, every network. The list under it
 * is not eight addresses, it is eight ways of saying the same one — which is
 * why every row shows the same characters, and why the copy button is on each
 * row rather than once at the top.
 */
@Composable
fun ReceiveListBody(
    model: ReceiveListModel,
    modifier: Modifier = Modifier,
    onQr: (Int) -> Unit = {},
) {
    val colors = VelaTheme.colors
    var query by remember { mutableStateOf("") }
    val (copied, setCopied) = rememberCopyTick()
    val context = LocalContext.current
    val haptic = rememberVelaHaptic()
    val shown = remember(query, model.rows) {
        if (query.isBlank()) {
            model.rows.withIndex().toList()
        } else {
            model.rows.withIndex().filter { it.value.name.contains(query.trim(), true) }
        }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        // R1 prints this directly under the title: the list below is not eight
        // addresses, and the sentence is the only thing that says so.
        Text(
            text = model.subtitle,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FlowSearchField(
            placeholder = model.searchPlaceholder,
            value = query,
            onValueChange = { query = it },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        if (shown.isEmpty()) {
            Text(
                text = model.emptyText.replace("{{query}}", query.trim()),
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.xl4),
            )
        } else {
            shown.forEachIndexed { position, entry ->
                if (position > 0) HairlineDivider()
                NetworkRow(
                    row = entry.value,
                    copied = copied == entry.index,
                    onCopy = { if (Clipboard.copy(context, entry.value.copyLabel, model.address)) { haptic(VelaHaptic.Select); setCopied(entry.index) } },
                    onQr = { onQr(entry.index) },
                )
            }
        }
    }
}

/** R2 / R3 — the address, as a code. */
@Composable
fun ReceiveQrBody(
    model: ReceiveQrModel,
    modifier: Modifier = Modifier,
    onSave: () -> Unit = {},
    onExplorer: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val (copied, setCopied) = rememberCopyTick()
    val context = LocalContext.current
    val haptic = rememberVelaHaptic()

    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = model.title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
        )
        model.contract?.let { contract ->
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = contract.label,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Text(
                    text = contract.value,
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    // The same copy affordance the address row carries, one size
                    // down: a contract is a detail ABOUT the code below, not the
                    // thing being received.
                    imageVector = if (copied == 1) VelaIcons.Check else VelaIcons.Copy,
                    contentDescription = contract.copyLabel,
                    tint = if (copied == 1) colors.successBase else colors.fgSubtle,
                    modifier = Modifier
                        .size(VelaIconSize.sm)
                        .clickable { if (Clipboard.copy(context, contract.copyLabel, contract.copyValue ?: contract.value)) { haptic(VelaHaptic.Select); setCopied(1) } },
                )
            }
        }
        AddressCard(
            account = model.account,
            copied = copied == 0,
            onCopy = { if (Clipboard.copy(context, model.account.copyLabel, model.account.lines.first + model.account.lines.second)) { haptic(VelaHaptic.Select); setCopied(0) } },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Box(modifier = Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            QrCard(
                label = model.title,
                // The address the card is showing, rejoined — a scanner reads
                // this, so it must be the same string the two lines above spell
                // out and never a shortened one.
                payload = (model.account.lines.first + model.account.lines.second)
                    .takeIf { it.isNotEmpty() },
            ) {
                Box(
                    modifier = Modifier
                        .size(VelaIconSize.xl3)
                        .background(model.centre.badgeColor, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = model.centre.ticker,
                        color = app.getvela.wallet.core.designsystem.tokens.VelaOnAccent,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = VelaTextSize.xs,
                        maxLines = 1,
                    )
                }
            }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = model.warning,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        FlowCta(
            label = model.saveImage,
            onClick = onSave,
            accent = false,
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        FlowCta(
            label = model.viewOnExplorer,
            onClick = onExplorer,
            accent = false,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

// ----------------------------------------------------------------- activity

/**
 * A1 — the full history.
 *
 * The wallet home shows the last three; this shows all of them, grouped by day.
 * Same `ActivityRow` as the home, same day headings — the difference is the
 * network filter in the header and that the list does not stop.
 */
@Composable
fun HistoryBody(
    model: HistoryModel,
    modifier: Modifier = Modifier,
    onSelect: (Int, Int) -> Unit = { _, _ -> },
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        when (model.mode) {
            HistoryMode.Empty -> Text(
                // A filtered-empty history is a narrowing, not a problem: one
                // quiet line rather than the illustrated empty state the home
                // uses for a wallet that has genuinely never done anything.
                text = model.emptyText,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.xl5),
            )
            HistoryMode.Loading, HistoryMode.Rows ->
                model.groups.forEachIndexed { groupIndex, group ->
                    DayLabel(label = group.label)
                    group.rows.forEachIndexed { rowIndex, row ->
                        Box(modifier = Modifier.clickable { onSelect(groupIndex, rowIndex) }) {
                            ActivityRow(model = row)
                        }
                    }
                }
        }
    }
}

/**
 * A2 / A3 — one transaction.
 *
 * A2 is a received ERC-20 and A3 a sent native coin; the difference between
 * them is entirely in the fact list (a native coin has no contract row), so
 * this takes the facts as data rather than branching on a kind.
 */
@Composable
fun TxDetailBody(
    model: TxDetailModel,
    modifier: Modifier = Modifier,
    onExplorer: () -> Unit = {},
    /** 删除记录 (spec 058). Absent in the gallery, where nothing is real. */
    onDelete: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    val (copied, setCopied) = rememberCopyTick()
    val context = LocalContext.current
    val haptic = rememberVelaHaptic()

    Column(modifier = modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Spacer(modifier = Modifier.width(VelaSpacing.md))
            StatusChip(chip = model.status)
        }
        AmountHero(amount = model.amount, fiat = model.fiat, positive = model.positive)
        HairlineDivider()
        model.facts.forEachIndexed { index, fact ->
            if (index > 0) HairlineDivider()
            FactRow(fact = fact, copied = copied == index, onCopy = { if (Clipboard.copy(context, fact.copy ?: fact.label, fact.copyValue ?: fact.value)) { haptic(VelaHaptic.Select); setCopied(index) } })
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        FlowCta(
            label = model.viewOnExplorer,
            onClick = onExplorer,
            accent = false,
            modifier = Modifier.fillMaxWidth(),
        )
        if (model.deleteLabel != null && onDelete != null) {
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            VelaDangerButton(
                text = model.deleteLabel,
                onClick = onDelete,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

// ------------------------------------------------------------------- assets

/**
 * T1 / T4 — everything the wallet holds.
 *
 * T4 is the same screen with nothing in it, and it does more than say so: an
 * empty asset list usually means either "you haven't received anything yet" or
 * "you have, and we can't see it". The guidance card answers the second,
 * because the person in that case is the one who needs help.
 */
@Composable
fun AssetsBody(
    model: AssetsModel,
    modifier: Modifier = Modifier,
    onSelect: (Int) -> Unit = {},
    onAdd: () -> Unit = {},
    onReceive: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    var query by remember { mutableStateOf("") }
    val shown = remember(query, model.rows) {
        if (query.isBlank()) {
            model.rows.withIndex().toList()
        } else {
            model.rows.withIndex().filter {
                "${it.value.ticker} ${it.value.chain}".contains(query.trim(), true)
            }
        }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        FlowSearchField(
            placeholder = model.searchPlaceholder,
            value = query,
            onValueChange = { query = it },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        val empty = model.empty
        if (empty != null) {
            // The empty state is tappable: its caption says "tap here to see
            // your address", so it had better be the thing that does.
            Box(modifier = Modifier.clickable(onClick = onReceive)) {
                EmptyState(
                    icon = VelaIcons.CreditCard,
                    model = EmptyStateModel(empty.title, empty.caption),
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            HintCard(title = empty.hintTitle, body = empty.hintBody) {
                FlowCta(
            label = empty.cta,
            onClick = onAdd,
            accent = false,
            modifier = Modifier.fillMaxWidth(),
        )
            }
        } else {
            shown.forEachIndexed { position, entry ->
                if (position > 0) HairlineDivider()
                AssetRow(model = entry.value, onClick = { onSelect(entry.index) })
            }
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
                // T1 sets this quiet and centred, not as a link: it is the way
                // out of "my token is missing", not an action competing with
                // the rows above it.
            Text(
                // A link, not a button: adding a token by hand is the rare path
                // out of a list that normally fills itself.
                text = model.addByAddress,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onAdd)
                    .padding(VelaSpacing.md),
            )
        }
    }
}

/** T2 — one token: what you hold, what it is, and what it has done. */
@Composable
fun TokenDetailBody(
    model: TokenDetailModel,
    modifier: Modifier = Modifier,
    onReceive: () -> Unit = {},
    onSend: () -> Unit = {},
    onExplorer: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val (copied, setCopied) = rememberCopyTick()
    val context = LocalContext.current
    val haptic = rememberVelaHaptic()

    Column(modifier = modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TokenIcon(mark = model.mark)
            Spacer(modifier = Modifier.width(VelaSpacing.lg))
            Column {
                Text(
                    text = model.symbol,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = model.chain,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = model.balance,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl4,
            lineHeight = VelaLeading.amountHero * VelaTextSize.xl4,
            maxLines = 1,
        )
        Text(
            text = model.fiat,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        // Receive and Send sit under the balance because they are the two
        // reasons anyone opens this sheet. Everything below is reference.
        Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            FlowCta(
            label = model.receive,
            onClick = onReceive,
            accent = false,
            modifier = Modifier.weight(1f),
        )
            FlowCta(
            label = model.send,
            onClick = onSend,
            accent = false,
            modifier = Modifier.weight(1f),
        )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        HairlineDivider()
        model.facts.forEachIndexed { index, fact ->
            if (index > 0) HairlineDivider()
            FactRow(fact = fact, copied = copied == index, onCopy = { if (Clipboard.copy(context, fact.copy ?: fact.label, fact.copyValue ?: fact.value)) { haptic(VelaHaptic.Select); setCopied(index) } })
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        Text(
            text = model.transactionsTitle,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.lg,
        )
        model.rows.forEach { ActivityRow(model = it) }
        Text(
            text = model.viewOnExplorer,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onExplorer)
                .padding(VelaSpacing.lg),
        )
    }
}

/**
 * T3 / T3b / T5 / T5b — adding a token, or the network one lives on.
 *
 * Two tabs over one shape: a field, a result card, a CTA. Every failure state
 * in T5 and T5b is a variant of the same two elements — the field's error, and
 * what the result card holds — so they are model states, not separate screens.
 */
@Composable
fun AddTokenBody(
    model: AddTokenModel,
    modifier: Modifier = Modifier,
    onTab: (String) -> Unit = {},
    onNetwork: () -> Unit = {},
    onSubmit: () -> Unit = {},
    /** Spec 043: present ⇒ the address is typed or pasted here. */
    onValueChange: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        FlowSegmentedToggle(
            options = listOf("erc20" to model.tabErc20, "native" to model.tabNative),
            selectedId = if (model.tab == AddTokenTab.Erc20) "erc20" else "native",
            onSelect = onTab,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        model.network?.let { network ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                    .clickable(onClick = onNetwork)
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TokenIcon(mark = network.mark,
                    inline = true,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.md))
                Text(
                    text = network.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.lg,
                    modifier = Modifier.weight(1f),
                )
                Icon(
                    imageVector = VelaIcons.ChevronDown,
                    contentDescription = network.pickLabel,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        if (onValueChange != null) {
            // Local echo: the field shows what was typed the instant it was
            // typed, and the core's echo of it is ignored — only a value the
            // field never sent (a paste the core normalised, a reset) replaces
            // the text. Round-tripping every keystroke dropped characters on
            // the device (spec 043 phase 3).
            var typed by remember { mutableStateOf(model.fieldValue) }
            val sent = remember { ArrayDeque<String>().apply { addLast(model.fieldValue) } }
            LaunchedEffect(model.fieldValue) { if (model.fieldValue !in sent) typed = model.fieldValue }
            FlowMonoField(
                value = typed,
                label = model.fieldLabel,
                placeholder = model.fieldPlaceholder,
                error = model.fieldError,
                onValueChange = { next ->
                    typed = next
                    sent.addLast(next)
                    while (sent.size > 8) sent.removeFirst()
                    onValueChange(next)
                },
            )
        } else {
            FlowMonoField(
                value = model.fieldValue,
                label = model.fieldLabel,
                placeholder = model.fieldPlaceholder,
                error = model.fieldError,
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        when (val result = model.result) {
            AddTokenResult.None -> Unit
            is AddTokenResult.Searching -> ResultNote(result.text)
            is AddTokenResult.NotFound -> ResultNote(result.text)
            is AddTokenResult.Token -> Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(
                        VelaBorder.hairline,
                        colors.borderBase,
                        RoundedCornerShape(VelaRadius.lg),
                    )
                    .padding(VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TokenIcon(mark = result.mark)
                Spacer(modifier = Modifier.width(VelaSpacing.lg))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = result.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = result.detail,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                result.chip?.let {
                    Spacer(modifier = Modifier.width(VelaSpacing.md))
                    StatusChip(chip = it)
                }
            }
            is AddTokenResult.Network -> Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(
                        VelaBorder.hairline,
                        colors.borderBase,
                        RoundedCornerShape(VelaRadius.lg),
                    )
                    .padding(VelaSpacing.lg),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TokenIcon(mark = result.mark)
                    Spacer(modifier = Modifier.width(VelaSpacing.lg))
                    Text(
                        text = result.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    StatusChip(chip = result.chip)
                }
                result.link?.let {
                    Spacer(modifier = Modifier.height(VelaSpacing.md))
                    Text(
                        text = it,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                }
                result.facts.forEach { FactRow(fact = it) }
            }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FlowCta(
            label = model.cta,
            onClick = onSubmit,
            accent = true,
            enabled = !model.ctaDisabled,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun ResultNote(text: String) {
    Text(
        text = text,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
    )
}

// --------------------------------------------------------------------- send

/**
 * SD1 / SD1b — which token (or tokens) to send.
 *
 * SD1b is the same list in multi-select. Once the first token is chosen the
 * network is decided, and rows on other chains grey out rather than
 * disappearing — the person still owns them, and a list that silently shortened
 * would read as a bug.
 */
@Composable
fun SendPickBody(
    model: SendPickModel,
    modifier: Modifier = Modifier,
    onFilter: (String) -> Unit = {},
    onSelect: (Int) -> Unit = {},
    onSelectAll: () -> Unit = {},
    onCta: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    var query by remember { mutableStateOf("") }
    val shown = remember(query, model.rows) {
        if (query.isBlank()) {
            model.rows.withIndex().toList()
        } else {
            model.rows.withIndex().filter {
                "${it.value.ticker} ${it.value.chain}".contains(query.trim(), true)
            }
        }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        FlowSearchField(
            placeholder = model.searchPlaceholder,
            value = query,
            onValueChange = { query = it },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FlowFilterChips(options = model.filters, onSelect = onFilter)
        model.notice?.let {
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            NoticeBanner(text = it.text, mark = it.mark)
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        shown.forEachIndexed { position, entry ->
            if (position > 0) HairlineDivider()
            AssetRow(
                model = entry.value,
                selected = model.selection?.selected?.getOrNull(entry.index) ?: false,
                dimmed = model.selection?.dimmed?.getOrNull(entry.index) ?: false,
                onClick = { onSelect(entry.index) },
            )
        }
        model.selection?.let {
            Text(
                text = it.selectAll,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier
                    .clickable(onClick = onSelectAll)
                    .padding(vertical = VelaSpacing.md),
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FlowCta(
            label = model.cta.label,
            onClick = onCta,
            accent = model.cta.accent,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * SD2 / SD2b / SD2d — the send form, in its three modes.
 *
 * One component, because the three ARE one form: single is a token, an amount
 * and a person; split is the same token to several people; sweep is several
 * tokens to one person. The SPEC sheet makes them mutually exclusive, so they
 * share a mode rather than living in three screens that would each need their
 * own fee row, summary line and CTA.
 */
@Composable
fun SendFormBody(
    model: SendFormModel,
    modifier: Modifier = Modifier,
    onPickRecipient: () -> Unit = {},
    onScan: () -> Unit = {},
    onRecipientAction: (RecipientAction) -> Unit = {},
    onRemoveRecipient: (Int) -> Unit = {},
    onFee: () -> Unit = {},
    onDenom: () -> Unit = {},
    onMax: (Int) -> Unit = {},
    onAddRecipient: () -> Unit = {},
    onContinue: () -> Unit = {},
    onAmountChange: ((String) -> Unit)? = null,
    onRecipientChange: ((String) -> Unit)? = null,
    onRecipientAmount: ((Int, String) -> Unit)? = null,
    onRecipientAddress: ((Int, String) -> Unit)? = null,
    onRecipientPick: ((Int) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        model.token?.let {
            TokenHeaderCard(token = it, onMax = { onMax(0) })
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        model.sweepSummary?.let {
            Text(
                text = it,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        model.sweepRows.forEachIndexed { index, row ->
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.lg),
            ) {
                AssetRow(
                    model = AssetRowModel(
                        ticker = row.symbol,
                        chain = row.balanceLabel,
                        badgeColor = row.mark.badgeColor,
                        logoUrls = row.mark.logoUrls,
                        badgeLogoUrl = row.mark.badgeLogoUrl,
                        badgeHidden = row.mark.badgeHidden,
                        balance = row.amount,
                        fiat = AssetFiatModel.None,
                        masked = false,
                    ),
                    trailing = {
                        Box(
                            modifier = Modifier
                                .background(colors.bgSunken, CircleShape)
                                .clickable { onMax(index) }
                                .padding(
                                    horizontal = VelaSpacing.md,
                                    vertical = VelaSpacing.xs,
                                ),
                        ) {
                            Text(
                                text = row.max,
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.semibold,
                                fontSize = VelaTextSize.xs,
                            )
                        }
                    },
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
        }
        model.amount?.let {
            AmountInput(amount = it, onDenom = onDenom, onValueChange = onAmountChange)
        }
        model.warning?.let {
            Text(
                text = it,
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaTextSize.sm * VelaLeading.normal,
                modifier = Modifier.padding(bottom = VelaSpacing.lg),
            )
        }
        model.recipient?.let {
            RecipientField(
                field = it,
                onPick = onPickRecipient,
                onScan = onScan,
                onValueChange = onRecipientChange,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        model.addRecipient?.let {
            // The door from a single send into a split. Quiet on purpose: most
            // sends have one recipient, and this is for the ones that don't.
            Text(
                text = "+  $it",
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier
                    .clickable(onClick = onAddRecipient)
                    .padding(vertical = VelaSpacing.sm),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        model.recipients.forEachIndexed { index, recipient ->
            RecipientCard(
                recipient = recipient,
                onRemove = { onRemoveRecipient(index) },
                onAmountChange = onRecipientAmount?.let { edit -> { text -> edit(index, text) } },
                onAddressChange = onRecipientAddress?.let { edit -> { text -> edit(index, text) } },
                onPick = onRecipientPick?.let { pick -> { pick(index) } },
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
        }
        if (model.recipientActions.isNotEmpty()) {
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            GhostPillRow(items = model.recipientActions, onSelect = { onRecipientAction(it.id) })
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        model.summary?.let {
            SummaryLine(summary = it)
            Spacer(modifier = Modifier.height(VelaSpacing.md))
        }
        FeeRow(fee = model.fee, onOpen = onFee)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        FlowCta(
            label = model.cta,
            onClick = onContinue,
            accent = true,
            modifier = Modifier.fillMaxWidth(),
            enabled = model.ctaEnabled,
        )
    }
}

/**
 * SD2e — choosing who gets the money.
 *
 * Scan sits at the top, above the saved people. Most sends go to someone
 * already in the book, but the ones that don't are the ones where a person is
 * holding a phone in one hand and an address in the other — so the escape hatch
 * is the first thing, not the last.
 */
@Composable
fun ContactPickBody(
    model: ContactPickModel,
    modifier: Modifier = Modifier,
    onScan: () -> Unit = {},
    onGroup: (Int) -> Unit = {},
    onSelect: (Int) -> Unit = {},
) {
    val colors = VelaTheme.colors
    var query by remember { mutableStateOf("") }
    val shown = remember(query, model.contacts) {
        if (query.isBlank()) {
            model.contacts.withIndex().toList()
        } else {
            model.contacts.withIndex().filter {
                "${it.value.name} ${it.value.addressDisplay}".contains(query.trim(), true)
            }
        }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        FlowSearchField(
            placeholder = model.searchPlaceholder,
            value = query,
            onValueChange = { query = it },
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                .clickable(onClick = onScan)
                .padding(VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = VelaIcons.QrCode,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.md),
            )
            Spacer(modifier = Modifier.width(VelaSpacing.md))
            Text(
                text = model.scanRow,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.base,
                modifier = Modifier.weight(1f),
            )
            Icon(
                imageVector = VelaIcons.ChevronRight,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
        if (model.groups.isNotEmpty() && query.isBlank()) {
            SectionCaption(model.groupsTitle)
            model.groups.forEachIndexed { index, group ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onGroup(index) }
                        .padding(vertical = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Two overlapping discs stand for "several people" without
                    // drawing any of them — a group has no single face to show.
                    Box(
                        modifier = Modifier
                            .size(VelaIconSize.xl2)
                            .background(group.colors.first, CircleShape),
                    )
                    Box(
                        modifier = Modifier
                            .size(VelaIconSize.xl2)
                            .background(group.colors.second, CircleShape),
                    )
                    Spacer(modifier = Modifier.width(VelaSpacing.lg))
                    Text(
                        text = group.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                        modifier = Modifier.weight(1f),
                    )
                    Text(
                        text = group.count,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                    Spacer(modifier = Modifier.width(VelaSpacing.sm))
                    Icon(
                        imageVector = VelaIcons.ChevronRight,
                        contentDescription = null,
                        tint = colors.fgSubtle,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                }
            }
        }
        SectionCaption(model.contactsTitle)
        shown.forEach { entry ->
            ContactPickRow(contact = entry.value, onSelect = { onSelect(entry.index) })
        }
    }
}

/**
 * SD2f — which coin pays the network fee.
 *
 * The hint above the list is doing real work: paying gas in a stablecoin is
 * unusual enough that a person seeing USDC offered as a fee token will wonder
 * whether they are being asked to send it. Saying what the choice is for, once,
 * above the rows, is cheaper than a tooltip on each.
 */
@Composable
fun FeeTokenBody(
    model: FeeTokenPickModel,
    modifier: Modifier = Modifier,
    onSelect: (Int) -> Unit = {},
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = model.hint,
            color = VelaTheme.colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaTextSize.sm * VelaLeading.normal,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        model.rows.forEachIndexed { index, row ->
            FeeTokenRow(
                row = row,
                estimateLabel = model.estimateLabel,
                onSelect = { onSelect(index) },
            )
        }
    }
}

/**
 * SD2c — pasting or importing a list of recipients.
 *
 * The screen's real subject is the rate, not the paste box. Someone importing a
 * payroll sheet has amounts in their own currency, and the question that decides
 * whether the transfer is right is what those become in the token.
 *
 * Bad rows are marked and skipped, never silently dropped, and the CTA counts
 * only the good ones — a button that says "Import 3" and imports 2 is how
 * someone underpays a contractor.
 */
@Composable
fun BatchImportBody(
    model: BatchImportModel,
    modifier: Modifier = Modifier,
    onUnit: (String) -> Unit = {},
    onFile: () -> Unit = {},
    onTemplate: () -> Unit = {},
    onApply: () -> Unit = {},
    onPaste: ((String) -> Unit)? = null,
    onRate: ((String) -> Unit)? = null,
    onRateReset: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        FlowSegmentedToggle(
            options = listOf("fiat" to model.unitFiat, "token" to model.unitToken),
            selectedId = if (model.unit == BatchUnit.Fiat) "fiat" else "token",
            onSelect = onUnit,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        if (onPaste != null) {
            // Local echo, as the amount hero: the machine's view lags fast typing.
            var typed by remember { mutableStateOf(model.pasteValue) }
            val sent = remember { ArrayDeque<String>().apply { addLast(model.pasteValue) } }
            LaunchedEffect(model.pasteValue) { if (model.pasteValue !in sent) typed = model.pasteValue }
            FlowMonoField(
                value = typed,
                placeholder = model.pastePlaceholder,
                minLines = 4,
                modifier = Modifier.semantics { contentDescription = "batch-paste" },
                onValueChange = { next ->
                    typed = next
                    sent.addLast(next)
                    if (sent.size > 256) sent.removeFirst()
                    onPaste(next)
                },
            )
        } else {
            FlowMonoField(
                value = model.pasteValue,
                placeholder = model.pastePlaceholder,
                minLines = 4,
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = VelaIcons.FileText,
                contentDescription = null,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.sm),
            )
            Spacer(modifier = Modifier.width(VelaSpacing.xs))
            Text(
                text = model.importFile,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.clickable(onClick = onFile),
            )
            Text(
                text = "  ·  ",
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
            Text(
                text = model.template,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.clickable(onClick = onTemplate),
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        HairlineDivider()
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.md),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = model.rateSection,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.weight(1f),
            )
            if (onRate != null && model.rateInput != null) {
                // The rate is the core's number; edited here it goes back as text.
                val rateStyle = TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base, textAlign = TextAlign.End)
                var typedRate by remember { mutableStateOf(model.rateInput) }
                val sentRate = remember { ArrayDeque<String>().apply { addLast(model.rateInput) } }
                LaunchedEffect(model.rateInput) { if (model.rateInput !in sentRate) typedRate = model.rateInput }
                Text(text = model.rateLabel, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
                Spacer(modifier = Modifier.width(VelaSpacing.xs))
                BasicTextField(
                    value = typedRate,
                    onValueChange = { next ->
                        typedRate = next
                        sentRate.addLast(next)
                        if (sentRate.size > 256) sentRate.removeFirst()
                        onRate(next)
                    },
                    singleLine = true,
                    textStyle = rateStyle,
                    cursorBrush = SolidColor(colors.accentBase),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    modifier = Modifier.width(88.dp).semantics { contentDescription = "batch-rate" },
                )
                if (model.rateEdited && model.rateReset != null) {
                    Spacer(modifier = Modifier.width(VelaSpacing.xs))
                    Text(
                        text = model.rateReset,
                        color = colors.accentBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.clickable(onClick = onRateReset),
                    )
                }
            } else {
                Text(
                    text = "${model.rateLabel} ${model.rateValue}",
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Spacer(modifier = Modifier.width(VelaSpacing.sm))
            Icon(
                imageVector = VelaIcons.Pencil,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
        Text(
            text = model.rateHint,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Text(
            text = model.parsedLabel,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
        model.rows.forEach { row ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = if (row.ok) VelaIcons.Check else VelaIcons.Close,
                    contentDescription = null,
                    tint = if (row.ok) colors.successBase else colors.errorBase,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                Spacer(modifier = Modifier.width(VelaSpacing.md))
                Text(
                    text = row.address,
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = row.conversion,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
        }
        model.rejectedText?.let {
            Text(
                text = it,
                color = colors.errorBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        model.note?.let {
            Text(
                text = it,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
        }
        FlowCta(
            label = model.cta,
            onClick = onApply,
            accent = true,
            enabled = !model.ctaDisabled,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * SD3 — the last screen before the money moves.
 *
 * Two blocks and nothing else: what is being sent, and the four facts that
 * decide whether that is right. A split or a sweep adds a second card listing
 * the parts. Per the SPEC sheet this is the ONE accent CTA in the whole send
 * journey — every other button on the way here is an outline.
 */
@Composable
fun SendConfirmBody(
    model: SendConfirmModel,
    modifier: Modifier = Modifier,
    onConfirm: () -> Unit = {},
    onNoticeAction: () -> Unit = {},
    onNoticeSecondary: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = VelaSpacing.lg, bottom = VelaSpacing.xl),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = model.amount,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl4,
                lineHeight = VelaLeading.amountHero * VelaTextSize.xl4,
            )
            Text(
                text = model.subline,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                .padding(horizontal = VelaSpacing.lg),
        ) {
            model.facts.forEachIndexed { index, fact ->
                if (index > 0) HairlineDivider()
                FactRow(fact = fact)
            }
        }
        if (model.breakdown.isNotEmpty()) {
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.xs),
            ) {
                model.breakdown.forEach { item ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = VelaSpacing.md),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        item.lead?.let {
                            TokenIcon(mark = it,
                                inline = true,
                            )
                            Spacer(modifier = Modifier.width(VelaSpacing.md))
                        }
                        item.identiconSeed?.let {
                            IdenticonImage(seed = it, size = VelaIconSize.lg)
                            Spacer(modifier = Modifier.width(VelaSpacing.md))
                        }
                        Text(
                            text = item.label,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                            modifier = Modifier.weight(1f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = item.value,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
            }
        }
        model.notice?.let { notice ->
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                    .padding(VelaSpacing.lg),
            ) {
                Text(
                    text = notice,
                    color = colors.warningBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    lineHeight = VelaTextSize.sm * VelaLeading.normal,
                )
                model.noticeAction?.let { action ->
                    Spacer(modifier = Modifier.height(VelaSpacing.md))
                    FlowCta(label = action, onClick = onNoticeAction, accent = false, modifier = Modifier.fillMaxWidth())
                }
                model.noticeSecondary?.let { secondary ->
                    Spacer(modifier = Modifier.height(VelaSpacing.sm))
                    Text(
                        text = secondary,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(onClick = onNoticeSecondary)
                            .padding(vertical = VelaSpacing.sm),
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl4))
        FlowCta(
            label = model.cta,
            onClick = onConfirm,
            accent = true,
            modifier = Modifier.fillMaxWidth(),
            enabled = model.ctaEnabled,
        )
    }
}

/**
 * SD4 — the receipt, in whichever state it is in.
 *
 * The SPEC sheet calls these "三态" and means it: submitting, submitted,
 * confirmed. One screen that changes, not three that replace each other — which
 * is why the disc, the title and the button keep their positions and only their
 * contents move.
 *
 * "Close · keep running" is load-bearing copy. The transaction does not depend
 * on this screen staying open, and a person who thinks it does will sit here
 * watching a spinner.
 */
@Composable
fun SendReceiptBody(
    model: SendReceiptModel,
    modifier: Modifier = Modifier,
    onExplorer: () -> Unit = {},
    onCta: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    var copied by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val haptic = rememberVelaHaptic()

    Column(modifier = modifier.fillMaxWidth()) {
        StatusHero(stage = model.stage, title = model.title, captions = model.captions)
        // The buttons live at the bottom while the status sits near the top:
        // the gap between them is where the waiting happens, and filling it
        // would make the screen look busier than the moment is.
        Spacer(modifier = Modifier.height(VelaSpacing.xl6))
        model.hash?.let { hash ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = hash.label,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Text(
                    text = hash.value,
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = if (copied) VelaIcons.Check else VelaIcons.Copy,
                    contentDescription = hash.copyLabel,
                    tint = if (copied) colors.successBase else colors.fgSubtle,
                    modifier = Modifier
                        .size(VelaIconSize.sm)
                        .clickable { if (Clipboard.copy(context, hash.copyLabel, hash.copyValue ?: hash.value)) { haptic(VelaHaptic.Select); copied = true } },
                )
            }
            Spacer(modifier = Modifier.height(VelaSpacing.md))
        }
        model.viewOnExplorer?.let {
            FlowCta(
            label = it,
            onClick = onExplorer,
            accent = false,
            modifier = Modifier.fillMaxWidth(),
        )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
        }
        FlowCta(
            label = model.cta,
            onClick = onCta,
            accent = model.ctaAccent,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

// ------------------------------------------------------------------ shared

/**
 * The one CTA in the flows.
 *
 * `accent` is not styling: in this product the accent means "this moves the
 * money", so exactly one button per journey carries it (SD3's confirm) and
 * every other is an outline. Routing both through one helper is what keeps that
 * rule checkable at a glance.
 */
@Composable
private fun FlowCta(
    label: String,
    onClick: () -> Unit,
    accent: Boolean,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    if (accent) {
        VelaPrimaryButton(text = label, onClick = onClick, modifier = modifier, enabled = enabled)
    } else {
        VelaSecondaryButton(text = label, onClick = onClick, modifier = modifier, enabled = enabled)
    }
}

/** The hairline that separates rows in every list here. */
@Composable
internal fun HairlineDivider(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(VelaBorder.hairline)
            .background(VelaTheme.colors.borderBase),
    )
}

@Composable
private fun SectionCaption(text: String) {
    Text(
        text = text,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        modifier = Modifier.padding(top = VelaSpacing.lg, bottom = VelaSpacing.sm),
    )
}
