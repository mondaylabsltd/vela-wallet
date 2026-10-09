package app.getvela.wallet.feature.signing

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.platform.testTag
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
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

/** The Trusted Signer account's "continue to the signing page" in the confirm's place (spec 079). */
const val OPEN_SIGNER_TAG = "signing-open-signer"

/**
 * The signing sheet (spec 022): the universal block renderer plus a fixed
 * footer — technical details → fee → signer → confirm — over the page that
 * asked for the signature, so the site you are dealing with never leaves the
 * screen.
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

/** The sheet's body, hostable anywhere (the preview gallery mounts it bare). */
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

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX)
            .padding(bottom = VelaSpacing.xl3),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        SigningSheetHeader(model, onClose)

        // Spec 079: approved — the receipt replaces the form, unless the
        // trusted signer's page is open, whose waiting card says more.
        val receipt = model.receipt
        if (receipt != null && model.trustedSignerWait == null) {
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
        model.blocks.forEach { block ->
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
            }
        }

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
        val waiting = model.trustedSignerWait
        // Three states, in order of precedence: waiting on the Trusted Signer's
        // page (071), a request that can be confirmed, and a request that
        // cannot. The last draws NOTHING — spec 081: a refused request offers
        // no confirm control at all, not a disabled one, because the wallet
        // never offered it.
        val action = model.confirmAction
        val handoff = model.handoff
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
        } else if (action != null) {
            // Issue #461: a tap, like the Send screen's Confirm — the same
            // button, saying the action alone ("确认兑换", "签名", "备份公钥").
            // Shut (dimmed) only while the core says so; once approved, the
            // receipt takes the form's place, so busy never looks shut.
            VelaPrimaryButton(
                text = action,
                onClick = onConfirm,
                enabled = model.confirmEnabled,
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag(CONFIRM_TAG),
            )
        }
        // Spec 099 R7: a shut confirm never sits there without a reason. The
        // fee measured again shuts the confirm for a moment ("Working out the
        // network fee…") and opens it again, and a line coming and going under
        // a bottom-anchored sheet moved the whole form each time. Once a note
        // has been said its line stays (the web's rule), holding the last
        // words invisible and silent while the gate is open. A live sheet
        // opens with one (reading, measuring), so the line is there from the
        // first frame; a board that never shut gains no blank line.
        val confirmNote = remember(model.requestKey) { HeldLine() }
        val note = model.confirmBlockLine?.takeIf { !model.confirmEnabled }
        val noteRoom = confirmNote.room(note)
        if (waiting == null && action != null && noteRoom != null) {
            Text(
                text = note ?: noteRoom,
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
