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
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
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
        hub.sendProposal(ev("review", "chain_id" to chainId, "safe_tx_hash" to safeTxHash, "blind" to false))
        hub.sendProposal(ev("read_signers", "chain_id" to chainId, "safe_tx_hash" to safeTxHash))
        hub.sendProposal(ev("timeline", "chain_id" to chainId, "safe_tx_hash" to safeTxHash))
        hub.sendProposal(ev("refresh", "fresh" to false))
        // §3.5: who can act for it on this network, read once a session (the
        // signing gate reads the chain again itself before any signature).
        (hub.accountsView.value.active ?: hub.proposalsView.value.followed)?.let { address ->
            hub.send(ev("check_on", "address" to address, "chain_id" to chainId))
        }
        onDispose { release() }
    }
    // The machine decides whether the last look is stale; the sheet only
    // asks every few seconds (docs/multisig-machines.md §1.4).
    LaunchedEffect(chainId, safeTxHash, item?.status) {
        while (true) {
            val current = hub.proposalsView.value.item(chainId, safeTxHash)
            if (current?.status == "ready") {
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
        if (review != null) {
            MultisigReviewBody(review, chainId, words, tokenInfo)
        } else {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                CircularProgressIndicator(Modifier.padding(VelaSpacing.xs), color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
                MsText(words.raw("componentsUi.signing.simRechecking"), MsType.Sub, color = colors.fgMuted)
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
                        MsText(MultisigWords.short(m), MsType.Mono, color = colors.fgSubtle)
                    }
                    MsText(state, MsType.Sub, color = if (done) colors.successBase else colors.fgSubtle)
                }
            }
        }

        // Actions.
        if (!model.isDone) {
            if (model.canApprove.isNotEmpty()) {
                val chosen = member ?: model.defaultMember
                if (model.canApprove.size > 1) {
                    MemberPicker(words, words.t("proposal.as"), model.canApprove, chosen, "multisig.proposal.as") { member = it }
                }
                VelaValueButton(
                    text = words.t(if (model.signAllowed) "proposal.approve" else "review.approveAnyway"),
                    onClick = {
                        val owner = chosen ?: return@VelaValueButton
                        haptic(VelaHaptic.Press)
                        scope.launch {
                            val settled = hub.proposals.request(ev("approve", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "owner" to owner)) { v ->
                                val s = (v["actions"] as? JsonObject)?.entries?.firstOrNull { it.key.equals("approve:${item.safeTxHash}", ignoreCase = true) }?.value
                                if (s == null || s.kind == "working") null else s
                            }
                            when (settled.kind) {
                                "done" -> haptic(VelaHaptic.Counted)
                                null, "cancelled" -> Unit
                                else -> haptic(VelaHaptic.Reject)
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.approve"),
                    enabled = (model.signAllowed || model.signAnyway) && !approving,
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
                VelaValueButton(
                    text = words.t(if (model.signAllowed) "proposal.execute" else "review.executeAnyway"),
                    onClick = {
                        haptic(VelaHaptic.Press)
                        hub.sendProposal(ev("execute", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.execute"),
                    enabled = !model.executeHeld(chosen) && (model.signAllowed || model.signAnyway) && !executingNow,
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
            if (item.unsent) {
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
                MsTextButton(words.t("proposal.reject"), tag = "multisig.proposal.reject") { confirming = true }
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
        if (!model.isDone && model.stillNeeded.isNotEmpty() && item.approved < item.threshold) {
            MsSection(words.t("proposal.invite")) {
                MsNote(words.t("proposal.stillNeeded", "missing" to model.stillNeeded.joinToString(", ", transform = words::label)))
                for (line in model.signerLines) MsNote(line.text, tag = "multisig.proposal.signers.${line.id}")
                model.link?.let { link -> ShareRow(words, link, "multisig.proposal") }
                if (model.outsideCanSign) {
                    VelaSecondaryButton(
                        text = words.t("external.signWithWallet"),
                        onClick = { hub.sendProposal(ev("approve_outside", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash)) },
                        modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.outside"),
                    )
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
