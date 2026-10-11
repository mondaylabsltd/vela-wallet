package app.getvela.wallet.feature.signing

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.platform.testTag
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.feature.wallet.components.SkeletonBlock
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.flows.SendReceiptBody
import app.getvela.wallet.feature.signing.components.AllowanceEditor
import app.getvela.wallet.feature.signing.components.TrustedSignerWaiting
import app.getvela.wallet.feature.signing.components.HandoffCard
import app.getvela.wallet.feature.signing.components.SigningAmount
import app.getvela.wallet.feature.signing.components.SigningBalances
import app.getvela.wallet.feature.signing.components.SigningCard
import app.getvela.wallet.feature.signing.components.SigningCode
import app.getvela.wallet.feature.signing.components.SigningHeader
import app.getvela.wallet.feature.signing.components.SigningIntent
import app.getvela.wallet.feature.signing.components.SigningNftHero
import app.getvela.wallet.feature.signing.components.SigningOwnHeader
import app.getvela.wallet.feature.signing.components.SigningParty
import app.getvela.wallet.feature.signing.components.SigningPositive
import app.getvela.wallet.feature.signing.components.SigningRows
import app.getvela.wallet.feature.signing.components.SigningSentence
import app.getvela.wallet.feature.signing.components.SigningSwapPair
import app.getvela.wallet.feature.signing.components.SigningWarning
import app.getvela.wallet.feature.signing.components.SignerRow
import app.getvela.wallet.feature.signing.components.TechDetails

/** The signing sheet's confirm (issue #461) — the stable hook UI tests tap. */
const val CONFIRM_TAG = "signing-confirm"

/** The place the simulation's verdict lands in ([SigningBlock.Held]). */
const val VERDICT_PLACE_TAG = "signing-verdict-place"

/** The verdict as it stands in its place, at its own height: a UI test measures it against the place. */
const val VERDICT_SHOWN_TAG = "signing-verdict-shown"

/** The header over the body: who is asking, on which network, and the ✕ — outside the scroll. */
const val HEADER_TAG = "signing-header"

/** The sheet's body — everything between the header and the confirm's footer, and the one thing in the sheet that scrolls. */
const val BODY_TAG = "signing-body"

/** The footer under the body: the confirm and the line under a shut one, outside the scroll. */
const val FOOTER_TAG = "signing-footer"

/** The Trusted Signer account's "continue to the signing page" in the confirm's place (spec 079). */
const val OPEN_SIGNER_TAG = "signing-open-signer"

/**
 * The signing sheet (spec 022): the universal block renderer plus a fixed
 * footer — technical details → fee → signer → confirm — over the page that
 * asked for the signature, so the site you are dealing with never leaves the
 * screen.
 *
 * Three parts (the device round, item 1): the header pinned at the top, a
 * BODY that scrolls when the sheet is taller than the screen, and under it
 * the confirm with its one line, pinned — see [SigningSheetContent].
 *
 * The header's ✕ is the one way to refuse (spec 079, owner ruling: "除非用户
 * 明确关掉，不应该很容易误操作，比如下滑就关掉了" — a stray swipe used to throw
 * the dApp's request away). No swipe, scrim tap or Back closes it. There is
 * still no big "Reject" button, because a wallet with one teaches people to
 * reach for it without reading.
 *
 * After the approval the form gives way to the send receipt's own body
 * ([SigningScreenModel.receipt]); closing then refuses nothing — the core
 * routes the same close to a plain dismiss once the commitment point passed.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SigningSheet(
    model: SigningScreenModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    onConfirm: () -> Unit = onDismiss,
    /** Spec 044: the guard's chips and custom amount reach the machine. */
    onChip: (String) -> Unit = {},
    onCustomAmount: (String) -> Unit = {},
    /** A batch leg's own chips and field — the leg index travels with them. */
    onLegChip: (Int, String) -> Unit = { _, _ -> },
    onLegCustomAmount: (Int, String) -> Unit = { _, _ -> },
    onFee: () -> Unit = {},
    onFeePick: (String) -> Unit = {},
    /** Spec 069: the speed control under the fee. */
    onToggleSpeed: () -> Unit = {},
    onPickSpeed: (String) -> Unit = {},
    /** Spec 071: the Trusted Signer's waiting card. */
    onTrustedSignerReopen: () -> Unit = {},
    onTrustedSignerCancel: () -> Unit = {},
    /** Spec 102: "Trust this version" on the hand-off card's question. */
    onHandoffTrust: () -> Unit = {},
    /** Spec 079: the receipt's "view on explorer". */
    onExplorer: () -> Unit = {},
    /** Spec 079: the fee row's refresh. */
    onRefreshFee: (() -> Unit)? = null,
    /** Spec 096 F8: the failed receipt's Try again. */
    onRetry: (() -> Unit)? = null,
) {
    VelaModalSheet(
        onDismissRequest = onDismiss,
        containerColor = VelaTheme.colors.bgRaised,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        modifier = modifier,
        dismissible = false,
    ) {
        SigningSheetContent(
            model = model,
            onClose = onDismiss,
            onExplorer = onExplorer,
            onRefreshFee = onRefreshFee,
            onConfirm = onConfirm,
            onChip = onChip,
            onCustomAmount = onCustomAmount,
            onLegChip = onLegChip,
            onLegCustomAmount = onLegCustomAmount,
            onFee = onFee,
            onFeePick = onFeePick,
            onToggleSpeed = onToggleSpeed,
            onPickSpeed = onPickSpeed,
            onTrustedSignerReopen = onTrustedSignerReopen,
            onTrustedSignerCancel = onTrustedSignerCancel,
            onHandoffTrust = onHandoffTrust,
            onRetry = onRetry,
        )
    }
}

/**
 * The sheet's content, hostable anywhere (the preview gallery mounts it bare).
 *
 * **A body that scrolls, between a header and a footer that do not** (the
 * device round, item 1 — security). The simulation's verdict is the one part
 * of the sheet a site cannot write, so nothing of it may be under a fold:
 * its place is a MINIMUM height and grows to a taller verdict
 * ([VerdictPlace]). What gives when the sheet no longer fits the screen is
 * the body — it scrolls — and never the confirm: the confirm and the line
 * the core says under a shut one sit in a footer OUTSIDE the scroll, at the
 * bottom of the sheet, always whole and never moved by what the body holds.
 * The whole sheet used to be one scroll with the confirm at its end; a
 * taller verdict could only push the confirm down, which is why the place
 * was a fixed height with a scroll of its own — a third balance row, or the
 * warning under an unverified token, behind a fold.
 *
 * **And the header does not scroll either**: who is asking, on which
 * network, and the ✕. The ✕ is the ONE way to refuse — no swipe, no scrim
 * tap, no Back — and the body brings a tall verdict into view by itself: on
 * a short screen that left the confirm in sight and the refusal scrolled out
 * of it, the wrong way round. So every form of the sheet keeps its header at
 * the top: the request, the receipt (whose close is the same control), a
 * refusal, the hand-off and the wait on the signing page.
 *
 * A sheet that fits is drawn exactly as before: the body follows the header
 * and the footer the body, at the form's own gap. The host bounds the height
 * (the modal sheet's cap, the gallery's frame).
 */
@Composable
fun SigningSheetContent(
    model: SigningScreenModel,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
    /** Spec 079: the header's ✕ — `null` in the gallery, which has nothing to close. */
    onClose: (() -> Unit)? = null,
    onExplorer: () -> Unit = {},
    onRefreshFee: (() -> Unit)? = null,
    onChip: (String) -> Unit = {},
    onCustomAmount: (String) -> Unit = {},
    onLegChip: (Int, String) -> Unit = { _, _ -> },
    onLegCustomAmount: (Int, String) -> Unit = { _, _ -> },
    /** Issue #262: the fee row's tap and its coin list's pick. */
    onFee: () -> Unit = {},
    onFeePick: (String) -> Unit = {},
    onToggleSpeed: () -> Unit = {},
    onPickSpeed: (String) -> Unit = {},
    onTrustedSignerReopen: () -> Unit = {},
    onTrustedSignerCancel: () -> Unit = {},
    /** Spec 102: "Trust this version" on the hand-off card's question. */
    onHandoffTrust: () -> Unit = {},
    /** Spec 096 F8: the failed receipt's Try again. */
    onRetry: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    var techOverride by remember(model.state) { mutableStateOf<Boolean?>(null) }
    val techOpen = techOverride ?: model.techOpen

    // Spec 079: approved — the receipt replaces the form, unless the
    // trusted signer's page is open, whose waiting card says more.
    val receipt = model.receipt?.takeIf { model.trustedSignerWait == null }
    // Three states, in order of precedence: waiting on the Trusted Signer's
    // page (071), a request that can be confirmed, and a request that
    // cannot. The last draws NOTHING — spec 081: a refused request offers
    // no confirm control at all, not a disabled one, because the wallet
    // never offered it.
    val waiting = model.trustedSignerWait
    val action = model.confirmAction
    val handoff = model.handoff
    // The confirm is the footer's. The two cards that can stand in its place
    // — waiting on the signer's page, and the hand-off to it (spec 102 D4,
    // whose sheet repeats no preview, so holds no verdict) — are cards of the
    // body, as tall as what they say; and a receipt or a refusal has no
    // confirm at all.
    val pinned = action?.takeIf { receipt == null && waiting == null && handoff == null }

    // Spec 099 R7: a shut confirm never sits there without a reason. The
    // fee measured again shuts the confirm for a moment ("Working out the
    // network fee…") and opens it again, and a line coming and going under
    // a bottom-anchored sheet moved the whole form each time. Once a note
    // has been said its line stays (the web's rule), holding the last
    // words invisible and silent while the gate is open. A live sheet
    // opens with one (reading, measuring), so the line is there from the
    // first frame; a board that never shut gains no blank line.
    // (A board of a moment after a line was said starts with that line's
    // room: [SigningScreenModel.confirmBlockRoom].)
    val confirmNote = remember(model.requestKey) { HeldLine().apply { room(model.confirmBlockRoom) } }
    val note = model.confirmBlockLine?.takeIf { !model.confirmEnabled }
    val noteRoom = if (receipt != null) null else confirmNote.room(note)
    val noteLine: (@Composable () -> Unit)? = noteRoom?.takeIf { waiting == null && action != null }?.let { room ->
        {
            Text(
                text = note ?: room,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .then(if (note == null) Modifier.alpha(0f).clearAndSetSemantics {} else Modifier),
            )
        }
    }

    // The body's frame, as laid out: what the verdict's place brings itself
    // into view of.
    val bodyFrame = remember { Laid() }
    // A request's own: the next one opens at its top, wherever the body of
    // the one before it had been brought to.
    val bodyScroll = key(model.requestKey) { rememberScrollState() }

    Column(modifier = modifier.fillMaxWidth()) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .testTag(HEADER_TAG)
                // While the body is scrolled under the header, a hairline
                // says where the body starts: drawn, so it moves nothing.
                // (Nothing shows through: the body is cut at its own frame.)
                .drawBehind {
                    if (bodyScroll.canScrollBackward) {
                        val line = VelaBorder.hairline.toPx()
                        drawRect(colors.borderBase, topLeft = Offset(0f, size.height - line), size = Size(size.width, line))
                    }
                }
                .padding(horizontal = VelaSizing.screenPaddingX)
                // The form's own gap under the header: where it always was.
                .padding(bottom = VelaSpacing.xl),
        ) { SigningSheetHeader(model, onClose) }

        Column(
            modifier = Modifier
                // What the header and the footer leave, and no more: a sheet
                // that fits stays as tall as its content.
                .weight(1f, fill = false)
                .fillMaxWidth()
                .onGloballyPositioned { bodyFrame.at = it }
                .testTag(BODY_TAG)
                .verticalScroll(bodyScroll)
                .padding(horizontal = VelaSizing.screenPaddingX)
                // The sheet's bottom margin is under its last thing: the
                // footer when there is one.
                .then(if (pinned == null) Modifier.padding(bottom = VelaSpacing.xl3) else Modifier),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
        ) {
            if (receipt != null) {
                SendReceiptBody(
                    model = receipt,
                    onExplorer = onExplorer,
                    onCta = { onClose?.invoke() },
                    onRetry = onRetry,
                )
                return@Column
            }

            // The universal renderer: blocks in mock order, out. Nothing here knows
            // what a swap or a permit IS — which is what lets all 33 scenarios, and
            // the ones nobody has drawn yet, come out of one code path. (Issue
            // #314: the wallet's own request leads with its intent — that is the
            // header's title, so it is not among these blocks.)
            // One block, drawn. A local function so a held place can draw its two
            // blocks — the room and what took it — with the same renderer.
            @Composable
            fun Draw(block: SigningBlock) {
                when (block) {
                    is SigningBlock.Intent -> SigningIntent(block.text, block.tone)
                    is SigningBlock.Amount ->
                        SigningAmount(block.line, card = block.card, note = block.note)

                    is SigningBlock.Swap -> SigningSwapPair(block.pay, block.receive)
                    is SigningBlock.Nft -> SigningNftHero(block.id, block.collection)
                    is SigningBlock.Sentence -> SigningSentence(block.text, block.tone)
                    is SigningBlock.Allowance -> {
                        // A batch leg's card talks to its OWN leg: the single
                        // approval's events are ignored by the core on a batch,
                        // which is how these chips were drawn and dead.
                        val leg = block.leg
                        AllowanceEditor(
                            block.label, block.value, block.valueTone, block.chips,
                            block.note, block.resultingTotal,
                            custom = block.custom,
                            onChip = if (leg == null) onChip else { id -> onLegChip(leg, id) },
                            onCustomAmount = if (leg == null) onCustomAmount else { text -> onLegCustomAmount(leg, text) },
                        )
                    }

                    is SigningBlock.Party ->
                        SigningParty(block.label, block.name, block.address, block.badge)

                    is SigningBlock.Rows -> SigningRows(block.rows)
                    is SigningBlock.Warning -> SigningWarning(block.tone, block.text)
                    is SigningBlock.Positive -> SigningPositive(block.text)
                    is SigningBlock.Code -> SigningCode(block.lines, block.note)
                    is SigningBlock.Card -> SigningCard(block.title, block.rows, block.tone)
                    is SigningBlock.Balances ->
                        SigningBalances(block.title, block.rows, block.note, block.noteTone)

                    // A place kept for a verdict that lands late: every verdict
                    // the sheet usually ends on is drawn unseen and unsaid, one
                    // over the other, so the place is as tall as the tallest
                    // from the first frame and whichever arrives — centred in
                    // it — moves nothing above it and nothing below. Until one
                    // does, a skeleton stands in it; and one taller than the
                    // place is shown WHOLE — the place grows to it, and the
                    // body brings it into view.
                    is SigningBlock.Held -> VerdictPlace(
                        waiting = block.waiting,
                        rooms = { block.rooms.forEach { room -> Draw(room) } },
                        verdict = block.shown,
                        shown = block.shown?.let { shown -> { Draw(shown) } },
                        bodyFrame = bodyFrame,
                        bodyScroll = bodyScroll,
                    )
                }
            }
            model.blocks.forEach { block -> Draw(block) }

            Box(
                Modifier
                    .fillMaxWidth()
                    .height(VelaBorder.hairline)
                    .background(colors.borderBase),
            )

            if (!model.tech.isEmpty) {
                TechDetails(model.tech, techOpen, onToggle = { techOverride = !techOpen })
            }
            // Nothing moves while the fee is measured again (device-found: the
            // form jumped on every 30 s re-quote, speed pick and refresh). The
            // line under the fee keeps its last words' height — unsaid — until a
            // fee lands; the web's rule.
            val feeLine = remember(model.requestKey) { HeldLine() }
            model.fee?.let { drawn ->
                val fee = if (drawn is FeeModel.OnChain) {
                    val line = feeLine.next(drawn.warning, drawn.measuring, drawn.reserve)
                    drawn.copy(heldWarning = line.takeIf { drawn.warning == null })
                } else {
                    drawn
                }
                SigningFee(
                    fee,
                    onFee = onFee,
                    onPick = onFeePick,
                    onToggleSpeed = onToggleSpeed,
                    onPickSpeed = onPickSpeed,
                    onRefresh = onRefreshFee,
                )
            }
            SignerRow(model.signerLabel, model.signerName, model.signerSeed)
            model.trustedSignerNotice?.let { SigningWarning(SigningTone.Caution, it) }
            if (waiting != null) {
                TrustedSignerWaiting(waiting, onReopen = onTrustedSignerReopen, onCancel = onTrustedSignerCancel)
            } else if (handoff != null && action != null) {
                // Spec 102 D4: the account reviews and signs on its page — the
                // card says where, with which key and what is trusted about the
                // page, and its Open is the consent that goes there.
                HandoffCard(
                    handoff,
                    enabled = model.confirmEnabled,
                    onOpen = onConfirm,
                    modifier = Modifier.testTag(OPEN_SIGNER_TAG),
                    onTrust = onHandoffTrust,
                )
                noteLine?.invoke()
            }
        }

        if (pinned != null) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag(FOOTER_TAG)
                    // While part of the body is under the footer, a hairline
                    // says where the body ends: drawn, so it moves nothing.
                    .drawBehind {
                        if (bodyScroll.canScrollForward) {
                            drawRect(colors.borderBase, size = Size(size.width, VelaBorder.hairline.toPx()))
                        }
                    }
                    .padding(horizontal = VelaSizing.screenPaddingX)
                    // The form's own gap above the confirm, and the sheet's
                    // bottom margin under its note: where both always were.
                    .padding(top = VelaSpacing.xl, bottom = VelaSpacing.xl3),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
            ) {
                // Issue #461: a tap, like the Send screen's Confirm — the same
                // button, saying the action alone ("确认兑换", "签名", "复制钱包记录").
                // Shut (dimmed) only while the core says so; once approved, the
                // receipt takes the form's place, so busy never looks shut.
                VelaPrimaryButton(
                    text = pinned,
                    onClick = onConfirm,
                    enabled = model.confirmEnabled,
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag(CONFIRM_TAG),
                )
                noteLine?.invoke()
            }
        }
    }
}

/**
 * Where something was laid out, kept for whoever measures against it later.
 * A plain holder, not state: a scroll re-places everything in the body on
 * every frame, and none of that is a reason to recompose.
 */
private class Laid {
    var at: LayoutCoordinates? = null
}

/**
 * The simulation's verdict's place ([SigningBlock.Held]).
 *
 * - **Its height is the rooms' at least**: every verdict the sheet usually
 *   ends on, drawn unseen and unsaid one over the other — so the place is as
 *   tall as the tallest from the first frame, and the common verdicts move
 *   nothing when they land. A verdict no taller than the place sits centred
 *   in it.
 * - **It is never blank.** While the verdict is out a skeleton of the card
 *   that is coming fills it, quietly, said to a screen reader as [waiting].
 *   A reserved 120 dp of nothing read as a sheet that had failed to draw.
 * - **A taller verdict is shown whole** (the device round, item 1 —
 *   security): a third balance row, the warning under an unverified token.
 *   The place grows to the verdict's own height — no scroll of its own, no
 *   fade, no "more" mark, nothing clipped. It used to keep its height and
 *   scroll the verdict inside, so the one block a site cannot write could
 *   end under a fold, above a live confirm. Growing costs the confirm
 *   nothing now: it is in the sheet's footer, outside the body this is in.
 * - **It brings itself into view.** When the verdict lands, or grows, and
 *   part of it is outside the body's frame — a sheet taller than the screen
 *   — the body scrolls to it: all of it when it fits the frame, else from
 *   its top. The frame is the body's own, between the pinned header and the
 *   footer: "in view" is never "under the header".
 */
@Composable
private fun VerdictPlace(
    waiting: String,
    rooms: @Composable () -> Unit,
    /** The verdict in the place, as data: what "it landed, or changed" is read from. */
    verdict: SigningBlock?,
    shown: (@Composable () -> Unit)?,
    /** The sheet's body: its frame as laid out (outside its own scroll), and its scroll. */
    bodyFrame: Laid,
    bodyScroll: ScrollState,
) {
    val place = remember { Laid() }
    var height by remember { mutableIntStateOf(0) }
    // The verdict landed, changed, or the place took another height: the
    // body shows it. Keyed on those alone — not on a scroll, nor on the body
    // changing size: a person reading further down is left there.
    // Brought in, the card stands clear of the footer's edge — by less than
    // the form's own gap, so the next thing in the form stays under the edge.
    val clear = with(LocalDensity.current) { VelaSpacing.lg.toPx() }
    LaunchedEffect(verdict, height) {
        if (verdict == null) return@LaunchedEffect
        val at = place.at?.takeIf { it.isAttached } ?: return@LaunchedEffect
        val frame = bodyFrame.at?.takeIf { it.isAttached } ?: return@LaunchedEffect
        val distance = distanceIntoView(
            top = frame.localPositionOf(at, Offset.Zero).y,
            height = at.size.height.toFloat(),
            frame = frame.size.height.toFloat(),
            margin = clear,
        )
        if (distance != 0f) bodyScroll.animateScrollBy(distance)
    }
    Layout(
        modifier = Modifier
            .fillMaxWidth()
            .testTag(VERDICT_PLACE_TAG)
            .onGloballyPositioned { coordinates ->
                place.at = coordinates
                height = coordinates.size.height
            },
        content = {
            Box(modifier = Modifier.alpha(0f).clearAndSetSemantics {}) { rooms() }
            if (shown != null) {
                Box(modifier = Modifier.testTag(VERDICT_SHOWN_TAG)) { shown() }
            } else {
                VerdictSkeleton(waiting)
            }
        },
    ) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val room = measurables[0].measure(loose)
        // The verdict at its own height, whatever that is; the skeleton is
        // given exactly the rooms'.
        val inside = measurables[1].measure(
            if (shown != null) loose else loose.copy(minHeight = room.height, maxHeight = room.height),
        )
        // The rooms are the least the place is; a taller verdict is the place.
        val tall = maxOf(room.height, inside.height)
        layout(constraints.maxWidth, tall) {
            room.place(0, 0)
            inside.place(0, (tall - inside.height) / 2)
        }
    }
}

/**
 * How far a scrolling frame [frame] tall moves to show something [height]
 * tall whose top is [top] from the frame's own: nothing when it is all in
 * view; else all of it when it fits the frame — the edge that is out comes
 * in, [margin] clear of the frame's edge where there is room for that — and
 * from its top when it does not fit. Positive scrolls forward.
 */
internal fun distanceIntoView(top: Float, height: Float, frame: Float, margin: Float = 0f): Float {
    val bottom = top + height
    val clear = if (height + 2 * margin <= frame) margin else 0f
    return when {
        top >= 0f && bottom <= frame -> 0f
        // Taller than the frame: from its top.
        height > frame -> top
        // It fits: the edge that is out comes in.
        top < 0f -> top - clear
        else -> bottom + clear - frame
    }
}

/** The skeleton that stands in the verdict's place while the simulation is out. */
const val VERDICT_WAITING_TAG = "signing-verdict-waiting"

/**
 * The verdict's place while the simulation is out: the outline of the
 * balance card that most often lands in it — a title's bar and a row's two —
 * pulsing gently, the size of the place. No words are drawn (the verdict is
 * not known, and a sentence here would be one more thing to swap); a screen
 * reader hears [label].
 */
@Composable
private fun VerdictSkeleton(label: String) {
    val colors = VelaTheme.colors
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .testTag(VERDICT_WAITING_TAG)
            .clearAndSetSemantics { if (label.isNotEmpty()) contentDescription = label }
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.xl))
            .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.xl),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        SkeletonBlock(Modifier.fillMaxWidth(0.34f).height(VelaSpacing.lg))
        repeat(SigningLive.USUAL_MOVES) {
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                SkeletonBlock(Modifier.fillMaxWidth(0.2f).height(VelaSpacing.lg))
                SkeletonBlock(Modifier.fillMaxWidth(0.38f).height(VelaSpacing.lg))
            }
        }
    }
}

/**
 * Spec 102 D4: the hand-off card on its own, for a signature no signing sheet
 * is showing (a send the person started): where, with which key, what is
 * trusted about the page, and Open. Swiping it away is the same as Cancel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HandoffSheet(model: HandoffModel, onOpen: () -> Unit, onCancel: () -> Unit, cancel: String, onTrust: () -> Unit = {}) {
    VelaModalSheet(
        onDismissRequest = onCancel,
        containerColor = VelaTheme.colors.bgRaised,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        HandoffSheetContent(model, onOpen = onOpen, onCancel = onCancel, cancel = cancel, onTrust = onTrust)
    }
}

/** The card on its own and its Cancel — the modal's body, and the gallery's CS40/CS41. */
@Composable
fun HandoffSheetContent(model: HandoffModel, onOpen: () -> Unit, onCancel: () -> Unit, cancel: String, onTrust: () -> Unit = {}) {
    Column(
        modifier = Modifier.padding(horizontal = VelaSizing.screenPaddingX).padding(bottom = VelaSpacing.xl3),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        HandoffCard(model, enabled = model.integrity.opens, onOpen = onOpen, onTrust = onTrust)
        app.getvela.wallet.core.designsystem.components.VelaSecondaryButton(cancel, onCancel, Modifier.fillMaxWidth())
    }
}

/**
 * Spec 071: the waiting card on its own, for a signature no signing sheet is
 * showing (a send the person started). Swiping it away is the same as Cancel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrustedSignerWaitingSheet(model: TrustedSignerWaitModel, onReopen: () -> Unit, onCancel: () -> Unit) {
    VelaModalSheet(
        onDismissRequest = onCancel,
        containerColor = VelaTheme.colors.bgRaised,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        TrustedSignerWaiting(
            model,
            onReopen = onReopen,
            onCancel = onCancel,
            modifier = Modifier.padding(horizontal = VelaSizing.screenPaddingX).padding(bottom = VelaSpacing.xl3),
        )
    }
}

/**
 * Spec 079: the ending of a request whose sheet the core has closed — the same
 * header, the send receipt's body. Closes only on its ✕ or its button (the
 * signing sheet's rule), or by itself once the tick has been seen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SigningAftercareSheet(
    header: SigningScreenModel,
    receipt: app.getvela.wallet.feature.flows.SendReceiptModel,
    onClose: () -> Unit,
    onExplorer: () -> Unit,
) {
    VelaModalSheet(
        onDismissRequest = onClose,
        containerColor = VelaTheme.colors.bgRaised,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        dismissible = false,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
        ) {
            SigningSheetHeader(header, onClose)
            SendReceiptBody(model = receipt, onExplorer = onExplorer, onCta = onClose)
        }
    }
}

/**
 * The sheet's header, in every mode (form, receipt, aftercare): a site's —
 * its icon, its name, the network — or the wallet's own request's one row,
 * its title and the ✕. Either way the ✕ is there: it is the only close.
 */
@Composable
private fun SigningSheetHeader(model: SigningScreenModel, onClose: (() -> Unit)?) {
    if (model.dappOwn) {
        SigningOwnHeader(
            headline = model.headline.orEmpty(),
            onClose = onClose,
            closeLabel = model.closeLabel,
        )
    } else {
        SigningHeader(
            name = model.dappName,
            host = model.dappHost,
            letter = model.dappLetter,
            tint = model.dappTint,
            networkName = model.networkName,
            networkDot = model.networkDot,
            iconUrls = model.dappIconUrls,
            networkLogoUrl = model.networkLogoUrl,
            onClose = onClose,
            closeLabel = model.closeLabel,
        )
    }
}
