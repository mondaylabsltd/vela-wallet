package app.getvela.wallet.feature.signing

import androidx.compose.runtime.remember
import app.getvela.wallet.core.designsystem.components.KeptRoom
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.flows.components.FeeSpeedControl
import app.getvela.wallet.feature.flows.components.FeeSpeedNote
import app.getvela.wallet.feature.signing.components.SigningPositive
import app.getvela.wallet.feature.wallet.components.TokenIcon
import app.getvela.wallet.feature.wallet.components.WalletMetrics

/**
 * The fee row, and its expanded fee-token selector (mock CS33) — the last thing
 * between the request and the confirm. Under the row, inside the same card, the
 * speed control the send form draws (spec 069).
 */
@Composable
fun SigningFee(
    fee: FeeModel,
    modifier: Modifier = Modifier,
    /** The row's tap: retry a failed quote, or open / close the coin list. */
    onFee: () -> Unit = {},
    /** A coin from the list, by id (`SigningLive.NATIVE_FEE_ID` for the chain's own). */
    onPick: (String) -> Unit = {},
    /** Spec 069: the speed control under the fee — fold/unfold, and a speed by id. */
    onToggleSpeed: () -> Unit = {},
    onPickSpeed: (String) -> Unit = {},
    /** Spec 079: the refresh control (the send form's own). */
    onRefresh: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    when (fee) {
        is FeeModel.Hidden -> Unit
        is FeeModel.OffChain -> SigningPositive(fee.note, modifier, quiet = true)
        is FeeModel.OnChain -> Column(modifier = modifier.fillMaxWidth()) {
            SigningFeeBody(fee, onFee, onPick, onToggleSpeed, onPickSpeed, onRefresh)
            FeeLines(fee)
        }
    }
}

/** The room under the fee card: the line said there, and what the card may still say ([FeeRooms]). */
const val FEE_LINES_TAG = "signing-fee-lines"

/**
 * Under the fee card: the line it says there, and the room of everything it
 * may still come to say.
 *
 * Issue #262: the reason the confirm below is shut is said where the fix is.
 * While the fee is measured again the last words keep their line's height,
 * unsaid: invisible, and silent to TalkBack ([FeeModel.OnChain.heldWarning]).
 *
 * And ([FeeRooms], the 2026-10-11 walk) the place is as tall as the tallest
 * sentence named for it, plus — until the card has said it — the speed's
 * note: each drawn unseen and unsaid, at the width and in the face it would
 * be said in, so a line's arrival fills room that was already there. The
 * note is said INSIDE the card, which grows by exactly the room given up
 * here. A line taller than its own room (a failure's reason, in a language
 * that needs two lines for it) takes the note's unused room before it makes
 * the place any taller. With no rooms named (a drawn board) this is the line
 * alone, as before.
 */
@Composable
private fun FeeLines(fee: FeeModel.OnChain) {
    val line = fee.warning ?: fee.heldWarning
    val rooms = fee.rooms
    if (line == null && rooms == null) return
    val noteRoom = rooms?.speedNote?.takeIf { fee.speed?.freeNote == null }
    Layout(
        modifier = Modifier.fillMaxWidth().testTag(FEE_LINES_TAG),
        content = {
            // 0 — the line: said, or held unseen.
            if (line != null) FeeLine(line, seen = fee.warning != null) else Spacer(Modifier)
            // 1 — the speed's note, while the card has not said it: the card's own note, at the card's own inset.
            if (noteRoom != null) {
                Box(modifier = Modifier.padding(horizontal = VelaSpacing.sm).alpha(0f).clearAndSetSemantics {}) { FeeSpeedNote(noteRoom) }
            } else {
                Spacer(Modifier)
            }
            // 2… — every sentence the card may put here.
            rooms?.lines?.forEach { FeeLine(it, seen = false) }
        },
    ) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val placed = measurables.map { it.measure(loose) }
        val rooms = placed[1].height + (placed.drop(2).maxOfOrNull { it.height } ?: 0)
        layout(constraints.maxWidth, maxOf(placed[0].height, rooms)) {
            placed.forEach { it.place(0, 0) }
        }
    }
}

@Composable
private fun FeeLine(text: String, seen: Boolean) {
    Text(
        text = text,
        color = VelaTheme.colors.errorBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        modifier = Modifier
            .padding(top = VelaSpacing.sm, start = VelaSpacing.xl, end = VelaSpacing.xl)
            .then(if (seen) Modifier else Modifier.alpha(0f).clearAndSetSemantics {}),
    )
}

@Composable
private fun SigningFeeBody(
    fee: FeeModel.OnChain,
    onFee: () -> Unit,
    onPick: (String) -> Unit,
    onToggleSpeed: () -> Unit,
    onPickSpeed: (String) -> Unit,
    onRefresh: (() -> Unit)?,
) {
    val colors = VelaTheme.colors
    // The fee's worth is withheld until the display currency commits, and
    // the line it joins ("~0.000123 ETH · ≈CN¥2.24") wraps onto a second
    // line: the figure keeps two lines' room from the first frame, and keeps
    // it — the confirm under the card does not move when the worth lands.
    val worthRoom = remember { KeptRoom() }.keep(fee.worthRoom)
    run {
        if (fee.selectorTitle == null) Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(VelaRadius.lg))
                .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg)),
        ) {
            // Issue #438: a gap the label and the figure always keep (the iOS
            // row's 8). `SpaceBetween` had none to give once a long figure
            // filled the row: "Network fee~0.00783 ETH · ≈CN¥141.11". The
            // figure takes what the label leaves, and wraps at its end.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = fee.tappable, onClick = onFee)
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Text(
                    text = fee.label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Row(
                    modifier = Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md, Alignment.End),
                ) {
                    Box(modifier = Modifier.weight(1f, fill = false), contentAlignment = Alignment.CenterEnd) {
                        if (worthRoom) {
                            // Two lines of the figure's own face: the room, unseen and unsaid.
                            Text(
                                text = "\n",
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.base,
                                modifier = Modifier.alpha(0f).clearAndSetSemantics {},
                            )
                        }
                        Text(
                            text = fee.value,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                            textAlign = androidx.compose.ui.text.style.TextAlign.End,
                        )
                    }
                    if (fee.chevron) {
                        Icon(
                            VelaIcons.ChevronRight, null, tint = colors.fgMuted,
                            modifier = Modifier.size(VelaIconSize.sm),
                        )
                    } else if (fee.chevronRoom) {
                        // Its room, unmarked: the figure stays where it was.
                        Spacer(modifier = Modifier.size(VelaIconSize.sm))
                    }
                    fee.refreshLabel?.let { label ->
                        app.getvela.wallet.feature.flows.components.FeeRefreshButton(label = label, refreshing = fee.refreshing, onRefresh = onRefresh)
                    }
                }
            }
            fee.speed?.let { speed ->
                // The control pads its rows by `lg`; the row above by `xl`.
                FeeSpeedControl(
                    speed = speed,
                    modifier = Modifier.padding(horizontal = VelaSpacing.sm).padding(bottom = VelaSpacing.sm),
                    onToggle = onToggleSpeed,
                    onPick = onPickSpeed,
                )
            }
        } else {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(onClick = onFee)
                        .padding(vertical = VelaSpacing.md),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = fee.selectorTitle,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    Icon(
                        VelaIcons.ChevronDown, null, tint = colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                }
                fee.options.forEach { option ->
                    Column(Modifier.fillMaxWidth()) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(VelaRadius.lg))
                                .background(
                                    if (option.selected) colors.bgRaised else Color.Transparent,
                                    RoundedCornerShape(VelaRadius.lg),
                                )
                                // A coin that cannot pay is shown for context, never picked.
                                .clickable(enabled = !option.disabled) { onPick(option.id) }
                                .alpha(if (option.disabled) 0.45f else 1f)
                                .padding(VelaSpacing.md),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                        ) {
                            TokenIcon(mark = option.mark)
                            Column(
                                Modifier.weight(1f),
                                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
                            ) {
                                Text(
                                    text = option.name,
                                    color = colors.fgBase,
                                    fontFamily = VelaFontFamily,
                                    fontWeight = VelaFontWeight.semibold,
                                    fontSize = VelaTextSize.xl,
                                )
                                Text(
                                    text = option.balance,
                                    color = colors.fgMuted,
                                    fontFamily = VelaFontFamily,
                                    fontSize = VelaTextSize.base,
                                )
                            }
                            Text(
                                text = option.fee,
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.base,
                            )
                            if (option.selected) {
                                Icon(
                                    VelaIcons.Check, null, tint = colors.accentBase,
                                    modifier = Modifier.size(VelaIconSize.sm),
                                )
                            }
                        }
                        // Issue #408: why a greyed coin cannot pay, under its row and
                        // at full strength — the row's dimming is not a reason. Set
                        // in from the row's edge past the coin's mark, under the name.
                        option.reason?.let { reason ->
                            Text(
                                text = reason,
                                color = colors.errorBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.medium,
                                fontSize = VelaTextSize.sm,
                                modifier = Modifier.padding(
                                    start = VelaSpacing.md + WalletMetrics.avatarSize + VelaSpacing.lg,
                                    end = VelaSpacing.md,
                                    bottom = VelaSpacing.sm,
                                ),
                            )
                        }
                    }
                }
            }
        }
    }
}
