package app.getvela.wallet.feature.multisig

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaValueButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.flows.components.QrCard
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject

/**
 * One multisig transaction (M2) — the web's `ProposalSheet.svelte`, iOS's
 * `MultisigProposalSheet`: what it is (the one review every signer signs
 * from), where it stands (the core's one word), who has approved and how the
 * rest can (`signers`), what executing it costs and whether it would run now
 * (the machine's preview, paced by the machine), its history, and the
 * actions this device may take: approve, execute, reject, send again, ask
 * the others.
 *
 * Every decision is the proposals machine's. A signature always follows a
 * fresh chain read the machine makes itself (the signing gate, §3.5), and a
 * page venue approves on the member's trusted page.
 */
@Composable
fun MultisigProposalSheet(
    hub: MultisigHub,
    chainId: Int,
    safeTxHash: String,
    words: MultisigWords,
    origin: String,
    locale: String,
    tokenInfo: suspend (Int, String) -> Pair<String, Int>,
    explorerTx: (Int, String) -> String?,
    onClose: () -> Unit,
    onAddNetwork: ((Int) -> Unit)? = null,
    /** A site's request (M5): what the site is told, and when. */
    siteNote: String? = null,
) {
    val view by hub.proposalsView.collectAsStateWithLifecycle()
    val accounts by hub.accountsView.collectAsStateWithLifecycle()
    val item = view.item(chainId, safeTxHash)
        ?: view.linked?.takeIf { it.chainId == chainId && it.safeTxHash.equals(safeTxHash, ignoreCase = true) }?.item
    val haptic = rememberVelaHaptic()
    val scope = rememberCoroutineScope()
    var member by remember { mutableStateOf<String?>(null) }
    var executor by remember { mutableStateOf<String?>(null) }

    // A sheet holds a transaction: the machine's faster pace until let go;
    // its review, signers and history asked once.
    DisposableEffect(chainId, safeTxHash) {
        val release = hub.watch()
        hub.sendProposal(ev("timeline", "chain_id" to chainId, "safe_tx_hash" to safeTxHash))
        hub.sendProposal(ev("refresh", "fresh" to false))
        // §3.5: who can act for it on this network, read once a session (the
        // signing gate reads the chain again itself before any signature).
        (hub.accountsView.value.active ?: hub.proposalsView.value.followed)?.let { address ->
            hub.send(ev("check_on", "address" to address, "chain_id" to chainId))
        }
        onDispose { release() }
    }
    // The one review (§3.6), asked once the machine HAS the transaction —
    // the machine answers nothing for one it has not read yet, and a sheet
    // that asked only as it opened (a link, a notification, a dApp's
    // request, right after proposing) waited on "Checking again…" for ever
    // (r8-19). Asked again whenever what it is of changes, every
    // REVIEW_FRESH_MS while it waits, and every REVIEW_RETRY_MS while the
    // last could not reach the chain (the web's rule); a finished one's once.
    val reviewKey = item?.let { "${it.safeTxHash}:${it.status == "done"}:${view.chain(chainId)?.members?.owners?.joinToString(",")}:${view.chain(chainId)?.members?.threshold}" }
    var reviewAskedAt by remember(chainId, safeTxHash) { mutableStateOf(0L) }
    LaunchedEffect(reviewKey) {
        val current = item ?: return@LaunchedEffect
        var lastFresh = 0L
        while (true) {
            val now = System.currentTimeMillis()
            val shown = hub.proposalsView.value.item(chainId, current.safeTxHash)?.let { hub.proposalsView.value.review(it) }
            if (lastFresh == 0L || now - lastFresh >= REVIEW_FRESH_MS || reviewUnread(shown)) {
                hub.sendProposal(ev("review", "chain_id" to chainId, "safe_tx_hash" to current.safeTxHash, "blind" to false))
                lastFresh = now
                reviewAskedAt = now
            }
            if (current.status == "done") break
            delay(REVIEW_RETRY_MS)
        }
    }
    // How each member still to approve signs (r7-8's split, r8-8): asked
    // once the transaction is the machine's, and again as the members
    // still to approve change — never only at the sheet's opening, when a
    // proposal just made (or a link) is not in the machine's list yet and
    // the question was dropped: no outside-wallet link, no "Approve with
    // another wallet".
    val signersKey = item?.takeIf { it.status != "done" }?.let { "${it.safeTxHash}:${it.approvers.joinToString(",")}" }
    LaunchedEffect(signersKey) {
        val current = item ?: return@LaunchedEffect
        if (signersKey == null) return@LaunchedEffect
        hub.sendProposal(ev("read_signers", "chain_id" to chainId, "safe_tx_hash" to current.safeTxHash))
    }
    // The machine decides whether the last look is stale; the sheet only
    // asks every few seconds (docs/multisig-machines.md §1.4).
    LaunchedEffect(chainId, safeTxHash, item?.status) {
        while (true) {
            // Each executor's look, before any tap — a ready one's, and one a
            // member's approval would complete (the machine's `approve_execute`
            // reads that look).
            val current = hub.proposalsView.value.item(chainId, safeTxHash)
            if (current != null && current.status != "done" && current.canExecute.isNotEmpty()) {
                for (e in current.canExecute) {
                    hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to current.safeTxHash, "executor" to e))
                }
            }
            delay(4_000)
        }
    }

    val close = {
        if (hub.proposalsView.value.linked != null) hub.sendProposal(ev("close_linked"))
        onClose()
    }
    val approving = item?.let { view.action("approve:${it.safeTxHash}").kind == "working" } ?: false
    val executingNow = item?.let { view.action("execute:${it.safeTxHash}").kind == "working" || it.executing } ?: false

    MsSheet(words.t("proposal.title"), words.raw("common.done"), onClose = close, dismissible = !approving && !executingNow, tag = "multisig.proposal") {
        siteNote?.let { MsNote(it, tag = "multisig.proposal.site") }
        if (item == null) {
            LinkedState(view.linked?.stand, view.linked?.preview == true, chainId, words)
            // ms-21: no member here — the person may follow it on this device (kept, not shown).
            val linked = view.linked
            if (linked?.preview == true && !accounts.accounts.any { it.account.address.equals(linked.safe, ignoreCase = true) }) {
                MsTextButton(words.t("proposal.watch"), tag = "multisig.proposal.watch") {
                    hub.send(ev("keep_found", "accept_risks" to false))
                }
            } else if (linked?.preview == true) {
                MsNote(words.t("proposal.watching"), tag = "multisig.proposal.watching")
            }
            return@MsSheet
        }
        val model = MsProposalModel(item, view, words, accounts.activeRow?.account, origin, locale)
        val colors = VelaTheme.colors
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            MsText(model.title, MsType.Title, Modifier.testTag("multisig.proposal.title"))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                MsChip(model.statusLine, Modifier.testTag("multisig.proposal.status"), tone = if (model.standing.kind == "blocked") MsTone.Warning else MsTone.Neutral)
                MsText("${words.network(chainId)} · ${words.t("action.numberValue", "nonce" to item.nonce)}", MsType.Sub, color = colors.fgSubtle)
            }
        }
        for (note in model.notes) MsNote(note.text, tone = note.tone, tag = "multisig.proposal.note.${note.id}")
        model.executedTx?.let { tx ->
            MsNote(words.t("proposal.executed"), tone = MsTone.Positive, tag = "multisig.proposal.executed")
            explorerTx(chainId, tx)?.let { url ->
                val context = LocalContext.current
                MsTextButton(words.t("proposal.explorer"), tag = "multisig.proposal.explorer") {
                    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                }
            }
        }
        val review = model.review
        // r8-19: "Checking again…" is bounded — a review not back within
        // REVIEW_CAP_MS (or one the machine could not gather) is said as
        // unread, with Retry, never a spinner for ever.
        val now by produceState(System.currentTimeMillis(), model.reviewReady) {
            while (!model.reviewReady) {
                value = System.currentTimeMillis()
                delay(1_000)
            }
        }
        when (reviewStand(review, reviewAskedAt, now)) {
            ReviewStand.Shown -> MultisigReviewBody(review!!, chainId, words, tokenInfo)
            ReviewStand.Checking -> Row(Modifier.testTag("multisig.proposal.reviewChecking"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                CircularProgressIndicator(Modifier.padding(VelaSpacing.xs), color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
                MsText(words.raw("componentsUi.signing.simRechecking"), MsType.Sub, color = colors.fgMuted)
            }
            ReviewStand.Unread -> Row(verticalAlignment = Alignment.Top) {
                MsNote(
                    words.t(if (model.isDone) "proposal.cantCheckUsed" else "proposal.cantCheck", "network" to words.network(chainId)),
                    Modifier.weight(1f), tone = MsTone.Warning, tag = "multisig.proposal.reviewUnread",
                )
                MsTextButton(words.t("service.retry"), tag = "multisig.proposal.reviewRetry") {
                    reviewAskedAt = System.currentTimeMillis()
                    hub.sendProposal(ev("review", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "blind" to false))
                }
            }
        }

        // Approvals.
        MsSection("${words.t("proposal.approvals")} · ${item.approved}/${item.threshold}") {
            for (m in model.members) {
                val done = m.lowercase() in model.approvedBy
                val state = words.t(if (done) "proposal.approved" else if (item.approved >= item.threshold) "proposal.notNeeded" else "proposal.waiting")
                Row(
                    Modifier.fillMaxWidth().padding(vertical = VelaSpacing.sm).testTag("multisig.proposal.member")
                        .semantics(mergeDescendants = true) { contentDescription = "${words.label(m)}, $state" },
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                ) {
                    IdenticonAvatar(seed = m, size = VelaIconSize.xl, tappable = false)
                    Column(Modifier.weight(1f)) {
                        MsText(words.label(m), MsType.Field)
                        if (words.label(m) != MultisigWords.short(m)) MsText(MultisigWords.short(m), MsType.Mono, color = colors.fgSubtle)
                    }
                    MsText(state, MsType.Sub, color = if (done) colors.successBase else colors.fgSubtle)
                }
            }
        }

        // Actions.
        if (!model.isDone) {
            if (model.showApprove) {
                val chosen = member ?: model.defaultMember
                if (model.canApprove.size > 1) {
                    MemberPicker(words, words.t("proposal.as"), model.canApprove, chosen, "multisig.proposal.as") { member = it }
                }
                // core-ms-2: the machine offers this member "Approve and execute"
                // in one press (`approve_execute`): theirs reaches the threshold
                // and their look says they can pay.
                val oneTap = chosen != null && view.approveExecuteBy(item).any { it.equals(chosen, ignoreCase = true) }
                // r8-1: the core's sign rule decides the friction — `anyway` is
                // danger-styled and confirmed a second time (MsSignButton).
                MsSignButton(
                    text = when {
                        oneTap -> words.t(if (model.signAnyway) "review.approveExecuteAnyway" else "proposal.approveAndExecute")
                        else -> words.t(if (model.signAnyway) "review.approveAnyway" else "proposal.approve")
                    },
                    anyway = model.signAnyway,
                    dangerLines = model.dangerLines,
                    words = words,
                    onConfirm = {
                        val owner = chosen ?: return@MsSignButton
                        haptic(VelaHaptic.Press)
                        scope.launch {
                            fun settledOf(v: JsonObject, what: String): JsonElement? {
                                val s = (v["actions"] as? JsonObject)?.entries?.firstOrNull { it.key.equals("$what:${item.safeTxHash}", ignoreCase = true) }?.value
                                return if (s == null || s.kind == "working") null else s
                            }
                            if (oneTap) {
                                // Should only the execute part be unable to run (before its
                                // passkey), the machine still gives and keeps the approval (r4-11).
                                val ran = hub.proposals.request(ev("approve_execute", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to owner)) { v -> settledOf(v, "execute") }
                                if (ran.kind == "done") {
                                    haptic(VelaHaptic.Success)
                                    return@launch
                                }
                                val approved = hub.proposals.until(600_000) { v -> settledOf(v, "approve") }
                                when (approved.kind) {
                                    "done" -> haptic(VelaHaptic.Counted)
                                    null, "cancelled" -> Unit
                                    else -> haptic(VelaHaptic.Reject)
                                }
                                return@launch
                            }
                            val settled = hub.proposals.request(ev("approve", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "owner" to owner)) { v -> settledOf(v, "approve") }
                            when (settled.kind) {
                                "done" -> haptic(VelaHaptic.Counted)
                                null, "cancelled" -> Unit
                                else -> haptic(VelaHaptic.Reject)
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.approve"),
                    enabled = model.canSign && !approving,
                    loading = approving,
                )
            }
            if (model.isReady && model.canExecute.isNotEmpty()) {
                val chosen = executor ?: model.canExecute.first()
                if (model.canExecute.size > 1) {
                    MemberPicker(words, words.t("proposal.as"), model.canExecute, chosen, "multisig.proposal.executor") { executor = it }
                }
                model.feeLine(chosen)?.let { (text, tone) -> MsNote(text, tone = tone, tag = "multisig.proposal.fee") }
                for (line in model.setupLines(chosen)) MsNote(line)
                MsSignButton(
                    text = words.t(if (model.signAnyway) "review.executeAnyway" else "proposal.execute"),
                    anyway = model.signAnyway,
                    dangerLines = model.dangerLines,
                    words = words,
                    onConfirm = {
                        haptic(VelaHaptic.Press)
                        hub.sendProposal(ev("execute", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.execute"),
                    enabled = !model.executeHeld(chosen) && model.canSign && !executingNow,
                    loading = executingNow,
                )
                val look = model.preview(chosen).kind
                if (look == "unpriced" || look == "unchecked") {
                    MsTextButton(words.t("proposal.feeRetry"), tag = "multisig.proposal.feeRetry") {
                        hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                    }
                }
            } else if (!model.isReady) {
                MsNote(words.t("proposal.feeNote"))
            }
            if (item.unsent && !model.outboxSending) {
                VelaPrimaryButton(
                    text = words.t("proposal.outboxSendAgain"),
                    onClick = { hub.sendProposal(ev("send_again", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash)) },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.sendAgain"),
                )
            }
            if ((model.canApprove.isNotEmpty() || model.approvedByHere || model.canExecute.isNotEmpty()) &&
                item.action.kind != "cancel" && item.cancelledBy == null
            ) {
                var confirming by remember { mutableStateOf(false) }
                MsTextButton(words.t("proposal.reject"), tag = "multisig.proposal.reject", danger = true) { confirming = true }
                if (confirming) {
                    AlertDialog(
                        onDismissRequest = { confirming = false },
                        title = { Text(words.t("proposal.reject")) },
                        text = { Text(words.t("proposal.rejectHint", "threshold" to item.threshold.toString())) },
                        confirmButton = {
                            TextButton(onClick = {
                                confirming = false
                                val owner = member ?: model.defaultMember ?: model.canExecute.firstOrNull()
                                scope.launch {
                                    hub.propose(chainId, mapOf("kind" to "cancel", "nonce" to item.nonce), address = accounts.active, review = false, asMember = owner)
                                }
                            }, modifier = Modifier.testTag("multisig.proposal.rejectConfirm")) { Text(words.t("proposal.reject"), color = colors.errorBase) }
                        },
                        dismissButton = { TextButton(onClick = { confirming = false }) { Text(words.t("proposal.rejectCancel")) } },
                    )
                }
            }
        }

        // Asking the others.
        // r8-8: each member still to approve is offered the way the core's
        // split says (`signers`): the Vela link for Vela members (and the
        // ones nothing says yet), the outside-wallet link and code for an
        // outside one, and "Approve with another wallet" (a live session with
        // that wallet) for an outside or either one — as the web and desktop.
        if (!model.isDone && model.stillNeeded.isNotEmpty() && item.approved < item.threshold) {
            MsSection(words.t("proposal.invite")) {
                MsNote(words.t("proposal.stillNeeded", "missing" to model.stillNeeded.joinToString(", ", transform = words::label)))
                for (line in model.signerLines) MsNote(line.text, tag = "multisig.proposal.signers.${line.id}")
                if (model.showVelaLink) model.link?.let { link -> ShareRow(words, link, "multisig.proposal") }
                if (model.outsideCanSign) {
                    VelaSecondaryButton(
                        text = words.t("external.signWithWallet"),
                        onClick = { hub.sendProposal(ev("approve_outside", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash)) },
                        modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.outside"),
                    )
                }
            }
            model.outsideLink?.let { link ->
                MsSection(words.t("external.outsideLinkTitle"), Modifier.testTag("multisig.proposal.outsideInvite")) {
                    MsNote(words.t("external.outsideLinkHint"))
                    QrCard(label = words.t("external.outsideLinkTitle"), payload = link, modifier = Modifier.testTag("multisig.proposal.outsideQr"))
                    MsText(link, MsType.Mono, Modifier.testTag("multisig.proposal.outsideLink"), color = colors.fgMuted, maxLines = 4)
                    ShareRow(words, link, "multisig.proposal.outsideLink")
                }
            }
        }

        // History.
        val lines = model.timeline.mapNotNull(model::timelineLine)
        if (lines.isNotEmpty()) {
            MsSection(words.t("timeline.heading"), Modifier.testTag("multisig.proposal.history")) {
                for (line in lines) MsText(line, MsType.Sub, color = colors.fgMuted)
            }
        }
        MsRow(words.t("proposal.hash"), item.safeTxHash, mono = true, copyable = true)
        MsNote(words.t("proposal.noExpiry"))
    }
}

/** Where a linked transaction stands before its item reads. */
@Composable
private fun LinkedState(stand: String?, preview: Boolean, chainId: Int, words: MultisigWords) {
    when (stand) {
        "notInMailbox" -> MsNote(words.t("proposal.notInMailbox"), tone = MsTone.Warning, tag = "multisig.proposal.notInMailbox")
        "notFound" -> MsNote(words.t("proposal.notFound"), tone = MsTone.Warning, tag = "multisig.proposal.notFound")
        "unreadable" -> MsNote(words.t("proposal.cantCheck", "network" to words.network(chainId)), tone = MsTone.Warning)
        else -> Row(Modifier.testTag("multisig.proposal.opening"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            CircularProgressIndicator(color = VelaTheme.colors.fgMuted, strokeWidth = VelaSpacing.xs)
            MsText(words.t("proposal.opening"), MsType.Body, color = VelaTheme.colors.fgMuted)
        }
    }
    if (preview) MsNote(words.t("proposal.previewNote"))
}

/** Send the link (the share sheet) or copy it. */
@Composable
fun ShareRow(words: MultisigWords, link: String, tagPrefix: String) {
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    val haptic = rememberVelaHaptic()
    var copied by remember { mutableStateOf(false) }
    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl)) {
        MsTextButton(words.t("proposal.sendLink"), tag = "$tagPrefix.share") {
            val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link)
            runCatching { context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        }
        MsTextButton(if (copied) words.t("proposal.copied") else words.t("proposal.copyLink"), tag = "$tagPrefix.copyLink") {
            clipboard.setText(AnnotatedString(link))
            copied = true
            haptic(VelaHaptic.Select)
        }
    }
}

/** How often an open sheet reads its review again (the web's and the core's `REVIEW_FRESH_MS`). */
internal const val REVIEW_FRESH_MS = 30_000L

/** How soon a review that could not read the chain is asked again (the web's `REVIEW_RETRY_MS`). */
internal const val REVIEW_RETRY_MS = 5_000L

/** r8-19: how long "Checking again…" may stand before the review is said as unread, with Retry (the core's `PREVIEW_CAP_MS`). */
internal const val REVIEW_CAP_MS = 15_000L

/** The last review could not read the chain (its state or its simulation), or could not be gathered at all: asked again soon. */
internal fun reviewUnread(review: JsonElement?): Boolean =
    review is JsonNull || (review != null && (review["state"]["verdict"]["kind"].str == "unavailable" || review["simulation"]["kind"].str == "unreachable"))

/** Where a sheet's review stands (r8-19). */
enum class ReviewStand { Shown, Checking, Unread }

/**
 * Shown once the machine has it; "Checking again…" while it is gathered —
 * for at most [REVIEW_CAP_MS] after it was last asked; then, or when the
 * machine could not gather it (`null` in `reviews`), unread with Retry.
 * Never a spinner for ever.
 */
internal fun reviewStand(review: JsonElement?, askedAtMs: Long, nowMs: Long): ReviewStand = when {
    review != null && review !is JsonNull -> ReviewStand.Shown
    askedAtMs > 0 && nowMs - askedAtMs < (if (review is JsonNull) 1_500L else REVIEW_CAP_MS) -> ReviewStand.Checking
    askedAtMs == 0L && review == null -> ReviewStand.Checking
    else -> ReviewStand.Unread
}
