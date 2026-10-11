package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.feature.settings.KeyPillModel
import app.getvela.wallet.feature.settings.KeyPillTone
import kotlinx.coroutines.delay
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.TextLayoutResult
import kotlin.math.abs
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.passkey.PasskeyProviderMark
import app.getvela.wallet.feature.settings.WalletKeyRowModel
import app.getvela.wallet.feature.settings.WalletKeysModel

/**
 * The keys that control this wallet, and their Ethereum backup (spec 062).
 *
 * A key row says what a person can act on: what it is called, WHO is holding it
 * (the vault's own mark and name when the core's catalog knows — a passkey
 * manager, a security key), a short fingerprint that tells two unnamed keys
 * apart, and whether it is synced. The backup is the block's last row and its
 * only button.
 *
 * De-containered and hairline-divided like the rest of settings: facts to read,
 * not cards to tap. Metrics are [VelaSettingsRow]'s, so names line up with the
 * page below.
 *
 * The block has its final height from its FIRST frame (device walk,
 * 2026-10-11: one skeleton row grew into the wallet's keys when the registry
 * answered and everything under the block dropped — 151 px for one key). While
 * the registry is being asked the rows are the device's own, each already as
 * tall as it can end up: what the registry has yet to say — what holds the
 * key, its mark, its badges — is a quiet shape in the room those will take,
 * and the answer fills it in place.
 */
@Composable
fun VelaWalletKeysBlock(model: WalletKeysModel, onRow: (String) -> Unit) {
    val colors = VelaTheme.colors
    Column(modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.xl2).testTag(WALLET_KEYS_BLOCK_TAG)) {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
            )
            if (model.count.isNotEmpty()) {
                Text(text = model.count, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
            }
        }
        // The caption under the title says what the list below is: any one
        // of these keys signs for the wallet — or, when the registry did not
        // answer, that these are what this device remembers. ONE place for
        // both, as tall as the taller: the sentence used to arrive under the
        // rows, seconds after the page opened, and push the copy-record row
        // and everything after it down by its height.
        Box(modifier = Modifier.padding(top = VelaSpacing.xs, bottom = VelaSpacing.md)) {
            val caption = model.note ?: model.subtitle
            listOf(model.subtitle, model.noteRoom).filter { it.isNotEmpty() && it != caption }.forEach { unseen ->
                Text(
                    text = unseen,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier.alpha(0f).clearAndSetSemantics {},
                )
            }
            Text(
                text = caption,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
        }
        // Spec 102: a wallet whose keys belong to its own domain says which.
        model.domain?.let {
            Text(
                text = it,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
        }

        if (model.loading && model.rows.isEmpty()) {
            // Nothing is known yet — not even this device's own record, which
            // is read as the account comes to the front, before anybody can
            // open Settings. The shape of one row, for that instant.
            Row(
                modifier = Modifier.fillMaxWidth().heightIn(min = VelaSizing.controlLg).padding(vertical = VelaSpacing.lg).clearAndSetSemantics {},
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                Box(Modifier.size(VelaIconSize.lg).clip(CircleShape).background(colors.borderBase))
                Box(Modifier.width(VelaSpacing.xl5 * 3).height(VelaSpacing.md).clip(CircleShape).background(colors.borderBase))
            }
            SettingsDivider()
        } else {
            // While the registry is still being asked these are the device's
            // own rows: every key the wallet has, in its place and at its
            // final height. The answer fills them in; it moves nothing.
            model.rows.forEach { row ->
                KeyRow(row, model)
                SettingsDivider()
            }
        }

        model.backup?.let {
            VelaSettingsRow(row = it, divider = false, onClick = onRow)
            // What the copy makes public and what it costs, in the core's
            // words — "back up keys" read as handing over the keys themselves.
            // Only under a state it applies to: where the core names no
            // paragraph the block ends on its row, with no empty slot kept.
            model.backupExplain?.let { explain ->
                Text(
                    text = explain,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier.padding(start = VelaIconSize.lg + VelaSpacing.lg, bottom = VelaSpacing.md),
                )
            }
        }
    }
}

@Composable
private fun KeyRow(row: WalletKeyRowModel, model: WalletKeysModel) {
    val colors = VelaTheme.colors
    val expandable = row.details.isNotEmpty()
    var open by rememberSaveable(row.fingerprint) { mutableStateOf(false) }
    Column(modifier = Modifier.fillMaxWidth().testTag(WALLET_KEY_ROW_TAG)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                // The row is the summary; opening it shows what the registry holds
                // about this key. Nothing to open while it is still being asked.
                .then(if (expandable) Modifier.clickable { open = !open } else Modifier)
                // One sentence for a screen reader, not five fragments — and
                // only what is known: a row still being asked about says its
                // name and what the device vouches for, nothing else. Outside
                // the padding, so the row is one place to a screen reader
                // whether or not it opens yet.
                .semantics(mergeDescendants = true) {
                    contentDescription = (listOf(row.name, row.holder).filter { it.isNotEmpty() } + row.pills.map { it.text }).joinToString(", ")
                }
                .heightIn(min = VelaSizing.controlLg)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Box(modifier = Modifier.size(VelaIconSize.lg), contentAlignment = Alignment.Center) {
                if (row.pending) {
                    // Whose mark it is, is the registry's to say: its place, no glyph.
                    Box(Modifier.size(VelaIconSize.lg).clip(CircleShape).background(colors.borderBase))
                } else {
                    val drawn = PasskeyProviderMark(key = row.key, label = "", size = VelaIconSize.lg)
                    if (!drawn) {
                        Icon(imageVector = VelaIcons.Lock, contentDescription = null, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.lg))
                    }
                }
            }
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                KeyName(row)
                KeyHolderLine(row, model.holderRoom)
                // Under the name, not beside it: a phone has no room for a name
                // AND two pills on one line, and the name is what loses. And
                // wrapping: three Russian pills are wider than the column.
                if (row.pills.isNotEmpty() || model.badgeRoom.isNotEmpty()) KeyPillLine(row, model.badgeRoom)
            }
            // The chevron's place is the row's from the start: it arrives with
            // the registry's facts, and a column that narrowed for it then
            // would wrap its lines again.
            Box(modifier = Modifier.size(VelaIconSize.sm), contentAlignment = Alignment.Center) {
                if (expandable) {
                    Icon(
                        imageVector = VelaIcons.ChevronDown,
                        contentDescription = null,
                        tint = colors.fgSubtle,
                        modifier = Modifier.size(VelaIconSize.sm).rotate(if (open) 180f else 0f),
                    )
                }
            }
        }
        if (expandable && open) KeyDetails(row, model.copyLabel, model.copiedLabel)
    }
}

/**
 * A key's name on its one line. A key nobody has named yet — the device's
 * record names only the first, and the registry has not answered — is a bar
 * where the name will be written, the size of the name it gets when nobody
 * names it at all ("Key 2").
 */
@Composable
private fun KeyName(row: WalletKeyRowModel) {
    val colors = VelaTheme.colors
    if (row.name.isNotEmpty()) {
        Text(
            text = row.name,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    } else {
        Text(
            text = row.nameRoom.ifEmpty { " " },
            color = Color.Transparent,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.skeletonBar(colors.borderBase).clearAndSetSemantics {},
        )
    }
}

/**
 * Who holds the key, then its fingerprint — in the room of every line this
 * can come to read ([room]: drawn unseen and unread, so the tallest sets the
 * height: one line, or two where the widest wraps at this text size).
 *
 * While the registry is being asked the words are a bar and only the
 * fingerprint — the device's own fact — is written. The bar is the size of
 * the longest line the room is kept for, so the fingerprint already stands
 * where the longest answer puts it (on its own line, where that one wraps);
 * a shorter answer draws it back along the line, and no answer pushes it on.
 */
@Composable
private fun KeyHolderLine(row: WalletKeyRowModel, room: List<String>) {
    val colors = VelaTheme.colors
    Box {
        room.forEach { words -> HolderText(words, row.fingerprint, Modifier.alpha(0f).clearAndSetSemantics {}) }
        if (row.pending) {
            HolderText(room.maxByOrNull { it.length }.orEmpty(), row.fingerprint, skeleton = colors.borderBase)
        } else {
            HolderText(row.holder, row.fingerprint)
        }
    }
}

/** [skeleton]: [words] are not written — a bar of that colour is drawn where they would be, line by line. */
@Composable
private fun HolderText(words: String, fingerprint: String, modifier: Modifier = Modifier, skeleton: Color? = null) {
    val colors = VelaTheme.colors
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val bar = with(LocalDensity.current) { VelaSpacing.md.toPx() }
    Text(
        text = buildAnnotatedString {
            if (skeleton != null) withStyle(SpanStyle(color = Color.Transparent)) { append(words) } else append(words)
            if (fingerprint.isNotEmpty()) {
                if (words.isNotEmpty()) append("  ·  ")
                // One unit: word joiners around the ellipsis, so a
                // wrapped line moves the whole fingerprint rather than
                // splitting it into `7802…` and `2eec`.
                withStyle(SpanStyle(fontFamily = VelaMonoFontFamily)) { append(fingerprint.replace("…", "\u2060…\u2060")) }
            }
        },
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        // Two lines, not an ellipsis: a long holder name (Russian,
        // German) must not cut off the fingerprint that tells keys apart.
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
        onTextLayout = { layout = it },
        modifier = if (skeleton == null) {
            modifier
        } else {
            modifier
                .drawBehind {
                    val drawn = layout ?: return@drawBehind
                    val end = minOf(words.length, drawn.layoutInput.text.length)
                    for (line in 0 until drawn.lineCount) {
                        val from = drawn.getLineStart(line)
                        if (from >= end) break
                        val lineEnd = drawn.getLineEnd(line, visibleEnd = true)
                        val to = minOf(lineEnd, end)
                        if (to <= from) continue
                        val a = drawn.getHorizontalPosition(from, usePrimaryDirection = true)
                        val b = if (to >= lineEnd) drawn.getLineRight(line) else drawn.getHorizontalPosition(to, usePrimaryDirection = true)
                        val middle = (drawn.getLineTop(line) + drawn.getLineBottom(line)) / 2
                        drawRoundRect(
                            color = skeleton,
                            topLeft = Offset(minOf(a, b), middle - bar / 2),
                            size = Size(abs(b - a), bar),
                            cornerRadius = CornerRadius(bar / 2),
                        )
                    }
                }
                // What is not written is not read out either.
                .clearAndSetSemantics { if (fingerprint.isNotEmpty()) text = AnnotatedString(fingerprint) }
        },
    )
}

/**
 * A row's badges, in the room they can ever take.
 *
 * The line is always there, and always as tall as the fullest set of badges
 * the row can end with ([room], after what the device itself vouches for —
 * three pills wrap to two lines in English at the largest text): a row is the
 * same height while its badges are being asked for, when they arrive, and
 * when nobody could vouch for any. So an answer — or a registry that never
 * gives one — moves nothing under the row.
 */
@Composable
private fun KeyPillLine(row: WalletKeyRowModel, room: List<List<String>>) {
    // What the device itself vouches for stays in every outcome.
    val own = row.pills.filter { it.tone == KeyPillTone.SignsHere }
    Box(modifier = Modifier.padding(top = VelaSpacing.xs)) {
        room.forEach { texts ->
            PillFlow(Modifier.alpha(0f).clearAndSetSemantics {}) {
                own.forEach { KeyPill(it) }
                texts.forEach { KeyPill(KeyPillModel(it, KeyPillTone.Local)) }
            }
        }
        PillFlow {
            row.pills.forEach { pill -> KeyPill(pill) }
            // Where a badge will stand while the registry is still being
            // asked: the badge's own size, nothing to read.
            if (row.pending) room.firstOrNull()?.forEach { PendingPill(it) }
        }
    }
}

@Composable
private fun PillFlow(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        modifier = modifier,
    ) { content() }
}

/** An outlined pill's size ([KeyPill]) with nothing written in it. */
@Composable
private fun PendingPill(text: String) {
    Text(
        text = text,
        color = Color.Transparent,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier
            .clip(CircleShape)
            .background(VelaTheme.colors.borderBase)
            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs)
            .clearAndSetSemantics {},
    )
}

/** A quiet bar across the element, on its middle: the house skeleton's weight, whatever the element's height. */
private fun Modifier.skeletonBar(color: Color): Modifier = drawBehind {
    val bar = VelaSpacing.md.toPx()
    drawRoundRect(color = color, topLeft = Offset(0f, (size.height - bar) / 2), size = Size(size.width, bar), cornerRadius = CornerRadius(bar / 2))
}

const val WALLET_KEYS_BLOCK_TAG = "wallet-keys-block"
const val WALLET_KEY_ROW_TAG = "wallet-key-row"

/**
 * One of a key's pills. The key this device signs with (founder, 2026-09-26)
 * is the only FILLED one — success-soft behind a check, medium weight — so it
 * stands out from the registry's facts, which stay outlined. Never the accent:
 * that colour means money moving. Both shapes carry the same hairline, so a
 * filled pill and an outlined one are the same height on one line.
 */
@Composable
private fun KeyPill(pill: KeyPillModel) {
    val colors = VelaTheme.colors
    when (pill.tone) {
        KeyPillTone.SignsHere -> Row(
            modifier = Modifier
                .clip(CircleShape)
                .background(colors.successSoft)
                .border(VelaBorder.hairline, colors.successSoft, CircleShape)
                .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Icon(imageVector = VelaIcons.Check, contentDescription = null, tint = colors.successBase, modifier = Modifier.size(VelaIconSize.xs))
            Text(
                text = pill.text,
                color = colors.successBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        KeyPillTone.Verified, KeyPillTone.Synced, KeyPillTone.Local -> {
            val tint = when (pill.tone) {
                KeyPillTone.Verified -> colors.infoBase
                KeyPillTone.Synced -> colors.successBase
                else -> colors.fgMuted
            }
            Text(
                text = pill.text,
                color = tint,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .border(VelaBorder.hairline, tint, CircleShape)
                    .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
            )
        }
    }
}

/** What a key row opens onto — the registry explorer's facts, the two a person pastes elsewhere copyable. */
@Composable
private fun KeyDetails(row: WalletKeyRowModel, copyLabel: String, copiedLabel: String) {
    val colors = VelaTheme.colors
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf("") }
    LaunchedEffect(copied) {
        if (copied.isNotEmpty()) {
            delay(1200)
            copied = ""
        }
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = VelaSpacing.lg)
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        row.details.forEach { detail ->
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(text = detail.label.uppercase(), color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
                Text(
                    text = detail.value,
                    color = colors.fgBase,
                    fontFamily = if (detail.mono) VelaMonoFontFamily else VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                if (detail.copy) {
                    Text(
                        text = if (copied == detail.label) copiedLabel else copyLabel,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier
                            .clip(RoundedCornerShape(VelaRadius.md))
                            .border(VelaBorder.hairline, colors.borderStrong, RoundedCornerShape(VelaRadius.md))
                            .clickable {
                                clipboard.setText(AnnotatedString(detail.value))
                                copied = detail.label
                            }
                            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                    )
                }
            }
        }
    }
}
