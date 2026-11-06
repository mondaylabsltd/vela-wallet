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
import androidx.compose.runtime.mutableIntStateOf
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
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaValueButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.flows.components.QrCard
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.bool
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
    tokenInfo: suspend (Int, String) -> Pair<String, Int?>,
    explorerTx: (Int, String) -> String?,
    onClose: () -> Unit,
    onAddNetwork: ((Int) -> Unit)? = null,
    /** A site's request (M5): what the site is told, and when. */
    siteNote: String? = null,
    /**
     * r14-2: this sheet is still the one open. Its dismissal can land after
     * a newer link's sheet took its place; it then closes nothing — never
     * the newer sheet, never its link (`close_linked`).
     */
    isCurrent: () -> Boolean = { true },
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
        // r14-6: a review read before this sheet opened (a minute ago, before
        // the network went) is read again from the start — "Reading the
        // transaction…", every sign button shut, until the core says.
        hub.reviewOnOpen(chainId, safeTxHash)
        hub.sendProposal(ev("timeline", "chain_id" to chainId, "safe_tx_hash" to safeTxHash))
        hub.sendProposal(ev("refresh", "fresh" to false))
        // §3.5: who can act for it on this network, read once a session (the
        // signing gate reads the chain again itself before any signature).
        (hub.accountsView.value.active ?: hub.proposalsView.value.followed)?.let { address ->
            hub.send(ev("check_on", "address" to address, "chain_id" to chainId))
        }
        onDispose { release() }
    }
    // r10-7: once it is done (executed from this sheet, or seen executed),
    // the machine reads how it ran — the executor, the approvals that
    // counted (`frozen`), the timeline's "Executed by" — asked again then,
    // not only as the sheet opened on an open one.
    LaunchedEffect(chainId, safeTxHash, item?.status == "done") {
        if (item?.status == "done") hub.sendProposal(ev("timeline", "chain_id" to chainId, "safe_tx_hash" to safeTxHash))
    }
    // The one review (§3.6), asked once the machine HAS the transaction —
    // the machine answers nothing for one it has not read yet, and a sheet
    // that asked only as it opened (a link, a notification, a dApp's
    // request, right after proposing) waited on "Checking again…" for ever
    // (r8-19). Asked again whenever what it is of changes, every
    // REVIEW_FRESH_MS while it waits, and every REVIEW_RETRY_MS while the
    // last read could not reach the chain (the web's rule); a finished one's
    // once. §12: one the core says `failed` is not asked again behind the
    // person's back — it stays said, with "Try again" (`review_again`).
    val reviewKey = item?.let { "${it.safeTxHash}:${it.status == "done"}:${view.chain(chainId)?.members?.owners?.joinToString(",")}:${view.chain(chainId)?.members?.threshold}" }
    LaunchedEffect(reviewKey) {
        val current = item ?: return@LaunchedEffect
        var lastFresh = 0L
        while (true) {
            val now = System.currentTimeMillis()
            val v = hub.proposalsView.value
            val listed = v.item(chainId, current.safeTxHash)
            val shown = listed?.let { v.review(it) }
            val failed = listed?.let { v.reviewState(it) }.kind == "failed"
            if (lastFresh == 0L || (!failed && (now - lastFresh >= REVIEW_FRESH_MS || reviewUnread(shown)))) {
                hub.sendProposal(ev("review", "chain_id" to chainId, "safe_tx_hash" to current.safeTxHash, "blind" to false))
                lastFresh = now
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
    // Each executor's look (would it run, the fee, can it pay), asked as the
    // sheet opens and again when what it is of moves (its status, its
    // approvals), when the app comes back to the front, and on "Check again"
    // (r13 S4: an open sheet re-quoted the fee every 20 s for as long as it
    // stood). The timer asks only while a look has not priced it — none yet,
    // unpriced, unchecked — and at most LookAsk.AUTO_LIMIT times. A priced
    // look stands: Execute takes a new one itself when it is older than the
    // machine's PREVIEW_FRESH_MS, before any passkey.
    var lookRound by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(chainId, safeTxHash) {
        lookRound += 1
        onPauseOrDispose { }
    }
    // r14-6: a network that answers again asks at once (the hub's word).
    val reachedAgain by hub.reachedAgain.collectAsStateWithLifecycle()
    LaunchedEffect(chainId, safeTxHash, item?.status, item?.approved, lookRound, reachedAgain) {
        var asked = false
        var auto = 0
        var lastAuto = 0L
        while (true) {
            // A ready one's executors, and a member whose approval would
            // complete it (the machine's `approve_execute` reads that look).
            val v = hub.proposalsView.value
            val current = v.item(chainId, safeTxHash)
            if (current != null && current.status != "done") {
                // S4: a member whose approval would complete it is offered
                // "Approve and execute" with its fee said first — its look asked too.
                val oneShort = if (current.approved + 1 >= current.threshold) current.canApprove else emptyList()
                val executors = (current.canExecute + oneShort).distinctBy { it.lowercase() }
                val clock = System.currentTimeMillis()
                val now = executors.filter { e -> !asked || LookAsk.timerAsks(v.preview(current, e).kind, auto, clock - lastAuto) }
                for (e in now) {
                    hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to current.safeTxHash, "executor" to e))
                }
                if (asked && now.isNotEmpty()) {
                    auto += 1
                    lastAuto = clock
                }
                asked = true
            }
            delay(LookAsk.ASK_EVERY_MS)
        }
    }

    val close = {
        if (isCurrent()) {
            if (hub.proposalsView.value.linked != null) hub.sendProposal(ev("close_linked"))
            onClose()
        }
    }
    val approving = item?.let { view.action("approve:${it.safeTxHash}").kind == "working" } ?: false
    val executingNow = item?.let { view.action("execute:${it.safeTxHash}").kind == "working" || it.executing } ?: false

    MsSheet(words.t("proposal.title"), words.raw("common.done"), onClose = close, dismissible = !approving && !executingNow, tag = "multisig.proposal") {
        siteNote?.let { MsNote(it, tag = "multisig.proposal.site") }
        if (item == null) {
            val missing = LinkedStands.missing(view, chainId, words)
            LinkedState(
                view.linked?.stand, view.linked?.preview == true, words, LinkedStands.failure(view.linked, words),
                onTryAgain = if (LinkedStands.offersTryAgain(view.linked) || missing?.retry == true) ({ hub.sendProposal(ev("retry")) }) else null,
                missing = missing,
            )
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
                model.statusLine.takeIf { it.isNotEmpty() }?.let { line ->
                    MsChip(line, Modifier.testTag("multisig.proposal.status"), tone = if (model.standing.kind == "blocked") MsTone.Warning else MsTone.Neutral)
                }
                MsText("${words.network(chainId)} · ${words.t("action.numberValue", "nonce" to item.nonce)}", MsType.Sub, color = colors.fgSubtle)
            }
        }
        for (note in model.sheetNotes) MsNote(note.text, tone = note.tone, tag = "multisig.proposal.note.${note.id}")
        model.executedTx?.let { tx ->
            if (model.saysExecuted) MsNote(words.t("proposal.executed"), tone = MsTone.Positive, tag = "multisig.proposal.executed")
            explorerTx(chainId, tx)?.let { url ->
                val context = LocalContext.current
                MsTextButton(words.t("proposal.explorer"), tag = "multisig.proposal.explorer") {
                    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                }
            }
        }
        // §12: the review's place, as the core's sign door says it — read
        // (the review), being read ("Reading the transaction…", bounded by
        // the core), or not read (its reason and "Try again"). Approve,
        // Approve and execute and Execute below are the same door's.
        MsReviewDoor(model.door, words, "multisig.proposal", onTryAgain = { hub.sendProposal(model.reviewAgain) }, network = words.network(chainId)) {
            model.review?.takeUnless { it is JsonNull }?.let { MultisigReviewBody(it, chainId, words, tokenInfo, executed = model.isDone) }
        }

        // Approvals.
        // r10-7: an executed one is never re-counted against the mailbox —
        // "Executed", the members whose approvals counted when it ran (the
        // machine's `frozen`), the executor's own row "Executed", and who
        // sent it under the list (the web's r4a-8).
        val count = model.approvalsCount
        MsSection(if (count.isEmpty()) words.t("proposal.approvals") else "${words.t("proposal.approvals")} · $count", Modifier.testTag("multisig.proposal.approvals")) {
            for (m in model.listedMembers) {
                val said = model.memberState(m)
                val done = said.done
                val state = said.text
                Row(
                    Modifier.fillMaxWidth().padding(vertical = VelaSpacing.sm).testTag("multisig.proposal.member")
                        .semantics(mergeDescendants = true) { contentDescription = if (state.isEmpty()) words.label(m) else "${words.label(m)}, $state" },
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
            model.executedFacts?.let { MsText(it, MsType.Sub, Modifier.testTag("multisig.proposal.executedFacts"), color = colors.fgMuted) }
        }

        // Actions.
        if (!model.isDone) {
            // The one press is offered with its own fee line (above): the general note would say it twice.
            val oneTapOffered = model.showApprove && model.oneTapShown(member ?: model.defaultMember)
            if (model.showApprove) {
                val chosen = member ?: model.defaultMember
                if (model.canApprove.size > 1) {
                    MemberPicker(words, words.t("proposal.as"), model.canApprove, chosen, "multisig.proposal.as") { member = it }
                }
                // core-ms-2: the machine offers this member "Approve and execute"
                // in one press (`approve_execute`): theirs reaches the threshold
                // and their look says they can pay. r16-14: only while a press
                // can sign — a shut button reads Approve, steady.
                val oneTap = model.oneTapShown(chosen)
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
                                // r16-4: both or nothing — the core approves nothing when the
                                // execution stops before its signature, and the stop is said
                                // beside this button (`executeFailure`). §24: it carries the fee
                                // drawn under it (`shown_fee`).
                                val ran = hub.proposals.request(model.executeEvent(owner, oneTap = true)) { v -> settledOf(v, "execute") }
                                VelaLog.event("multisig.approveExecute", "settled", "kind" to (ran.kind ?: "none"))
                                if (ran.kind == "done") haptic(VelaHaptic.Success) else settledHaptic(ran.kind)?.let(haptic)
                                return@launch
                            }
                            val settled = hub.proposals.request(ev("approve", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "owner" to owner)) { v -> settledOf(v, "approve") }
                            // r9-5: what the machine said is logged, and drawn under this
                            // button (`approveFailure`) — a refusal is never a haptic alone.
                            VelaLog.event("multisig.approve", "settled", "kind" to (settled.kind ?: "none"))
                            settledHaptic(settled.kind)?.let(haptic)
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.approve"),
                    enabled = model.canSign && !approving && !(oneTap && executingNow),
                    loading = if (oneTap) executingNow else approving,
                )
                // S4: what the one press costs, before it is pressed — the
                // machine's look for this member (`previews`), as Execute's.
                // r16-14: only while a press can sign; a shut door's reason is
                // the one note.
                if (oneTap && chosen != null && model.saysExecution) {
                    // r15-7: and, for a multisig not deployed yet, its one-time setup.
                    ExecutionLines(model.executionLines(chosen, model.executeFailure), "multisig.proposal.oneTapFee", "multisig.proposal.oneTapSetup")
                    // r14-6: a fee that could not be worked out or checked offers
                    // Check again here too (the web's), as under Execute.
                    if (model.oneTapCheckAgain(chosen)) {
                        MsTextButton(words.t("proposal.feeRetry"), tag = "multisig.proposal.oneTapFeeRetry") {
                            hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                            lookRound += 1
                        }
                    }
                } else if (chosen != null && model.saysExecution) {
                    // r10 S4: theirs would complete it, but the one press is not
                    // offered — said why (can't pay the fee, would fail), never
                    // a silent "Approve" alone (the web's execute-cant-pay).
                    model.approveOnlyWhy(chosen)?.let { (text, tone) -> MsNote(text, tone = tone, tag = "multisig.proposal.approveOnlyWhy") }
                }
                // r9-5: why the press did not go, here where it was pressed.
                // r16-4: a one press stopped for a fee it could not work out is
                // asked again by a new look ("Check again").
                val checksFee = oneTap && model.failureChecksFee
                ActionFailure(
                    listOfNotNull(model.approveFailure, if (oneTap) model.executeFailure else null),
                    words,
                    retryLabel = if (checksFee) words.t("proposal.feeRetry") else null,
                ) {
                    if (checksFee && chosen != null) {
                        hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                        lookRound += 1
                    } else {
                        hub.sendProposal(ev("refresh", "fresh" to true))
                        (hub.accountsView.value.active ?: hub.proposalsView.value.followed)?.let { address ->
                            hub.send(ev("check_on", "address" to address, "chain_id" to chainId))
                        }
                    }
                }
                // r16-4: the approval alone, beside the one press (the web's and
                // iOS's second button) — the one press does both or nothing.
                if (model.approveAloneBeside(chosen)) {
                    MsSignButton(
                        text = words.t(if (model.signAnyway) "review.approveAnyway" else "proposal.approve"),
                        anyway = model.signAnyway,
                        dangerLines = model.dangerLines,
                        words = words,
                        secondary = true,
                        onConfirm = {
                            val owner = chosen ?: return@MsSignButton
                            haptic(VelaHaptic.Press)
                            scope.launch {
                                val settled = hub.proposals.request(ev("approve", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "owner" to owner)) { v ->
                                    val s = (v["actions"] as? JsonObject)?.entries?.firstOrNull { it.key.equals("approve:${item.safeTxHash}", ignoreCase = true) }?.value
                                    if (s == null || s.kind == "working") null else s
                                }
                                VelaLog.event("multisig.approve", "settled", "kind" to (settled.kind ?: "none"))
                                settledHaptic(settled.kind)?.let(haptic)
                            }
                        },
                        modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.approveOnly"),
                        enabled = model.canSign && !approving && !executingNow,
                        loading = approving,
                    )
                }
            }
            if (model.isReady && model.canExecute.isNotEmpty()) {
                val chosen = executor ?: model.canExecute.first()
                if (model.canExecute.size > 1) {
                    MemberPicker(words, words.t("proposal.as"), model.canExecute, chosen, "multisig.proposal.executor") { executor = it }
                }
                // r16-14: only while a press can sign.
                if (model.saysExecution) {
                    ExecutionLines(model.executionLines(chosen, model.executeFailure), "multisig.proposal.fee", "multisig.proposal.setup")
                }
                MsSignButton(
                    text = words.t(if (model.signAnyway) "review.executeAnyway" else "proposal.execute"),
                    anyway = model.signAnyway,
                    dangerLines = model.dangerLines,
                    words = words,
                    onConfirm = {
                        haptic(VelaHaptic.Press)
                        // §24: the figure drawn above it travels with the press (`shown_fee`):
                        // a quote that landed since the paint signs nothing (`fee_moved`).
                        hub.sendProposal(model.executeEvent(chosen, oneTap = false))
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.execute"),
                    enabled = !model.executeHeld(chosen) && model.canSign && !executingNow,
                    loading = executingNow,
                )
                ActionFailure(listOfNotNull(model.executeFailure), words) {
                    hub.sendProposal(ev("refresh", "fresh" to true))
                    hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                    lookRound += 1
                }
                val look = model.preview(chosen).kind
                if (model.saysExecution && model.executeFailure == null && LookAsk.offersCheckAgain(look)) {
                    MsTextButton(words.t("proposal.feeRetry"), tag = "multisig.proposal.feeRetry") {
                        hub.sendProposal(ev("preview", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash, "executor" to chosen))
                        // "Check again": the timer's few asks are the person's again.
                        lookRound += 1
                    }
                }
            } else if (!model.isReady && !oneTapOffered) {
                MsNote(words.t("proposal.feeNote"))
            }
            if (item.unsent && !model.outboxSending) {
                VelaPrimaryButton(
                    text = words.t("proposal.outboxSendAgain"),
                    onClick = { hub.sendProposal(ev("send_again", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash)) },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.sendAgain"),
                )
            }
            // r9-13: a cancellation waiting at its number is the core's
            // (`cancelledBy`, `sameNonce`): said here beside the actions, as
            // the web says it, with the cancellation itself one tap away —
            // its own sheet, review and approvals.
            for (note in model.cancelNotes) MsNote(note.text, tone = note.tone, tag = "multisig.proposal.note.${note.id}")
            item.cancelledBy?.takeIf { item.action.kind != "cancel" }?.let { cancel ->
                MsTextButton(words.t("action.cancel", "nonce" to item.nonce), tag = "multisig.proposal.openCancellation") {
                    hub.sheet.value = app.getvela.wallet.feature.multisig.core.MultisigSheet.Proposal(chainId, cancel)
                }
            }
            if ((model.canApprove.isNotEmpty() || model.approvedByHere || model.canExecute.isNotEmpty()) &&
                item.action.kind != "cancel" && item.cancelledBy == null
            ) {
                var confirming by remember { mutableStateOf(false) }
                // r16-5: how the last Reject ended, when there is something to say
                // (the core's answer to the cancellation's draft) — beside Reject.
                var rejectSettled by remember(item.safeTxHash) { mutableStateOf<JsonElement?>(null) }
                var rejecting by remember(item.safeTxHash) { mutableStateOf(false) }
                // r4D-13: the person's own proposal is withdrawn, not rejected.
                val rejectWord = words.t(if (model.ownProposal) "proposal.withdraw" else "proposal.reject")
                val reject: () -> Unit = {
                    rejectSettled = null
                    rejecting = true
                    // r9-13: the cancellation is a proposal like any other —
                    // drawn with its review first (the draft sheet, its member
                    // picked there), signed only on the person's word there,
                    // through the core's sign door (§12).
                    // On the hub's scope: this sheet steps aside for the review.
                    hub.scope.launch {
                        val settled = hub.propose(chainId, mapOf("kind" to "cancel", "nonce" to item.nonce), address = accounts.active)
                        VelaLog.event("multisig.reject", "settled", "kind" to (settled.kind ?: "none"))
                        rejectSettled = settled
                        rejecting = false
                    }
                }
                MsTextButton(rejectWord, tag = "multisig.proposal.reject", danger = true, enabled = !rejecting) { confirming = true }
                rejectFailureLine(words, rejectSettled, chainId)?.let { line ->
                    Row(verticalAlignment = Alignment.Top) {
                        Column(Modifier.weight(1f)) { MsNote(line.text, tone = line.tone, tag = "multisig.proposal.rejectFailed") }
                        MsTextButton(words.t("review.state.tryAgain"), tag = "multisig.proposal.rejectRetry") { reject() }
                    }
                }
                if (confirming) {
                    AlertDialog(
                        onDismissRequest = { confirming = false },
                        title = { Text(rejectWord) },
                        text = { Text(words.plural("proposal.rejectHint", item.threshold)) },
                        confirmButton = {
                            TextButton(onClick = {
                                confirming = false
                                reject()
                            }, modifier = Modifier.testTag("multisig.proposal.rejectConfirm")) { Text(rejectWord, color = colors.errorBase) }
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
        // r11 S4: a transaction Vela refuses for what it is asks nobody to
        // approve it (the core leaves it nobody who can, and names why).
        // r12-4: the refusal is the ONE review's (`reviews[…].refused`), never
        // the item's thin review — which has no such field, so the section
        // stayed for deep4 and a delegatecall.
        if (model.asksMembers) {
            MsSection(words.t("proposal.invite")) {
                MsNote(words.t("proposal.stillNeeded", "missing" to words.list(model.stillNeeded.map(words::label))))
                for (line in model.signerLines) MsNote(line.text, tag = "multisig.proposal.signers.${line.id}")
                if (model.showVelaLink) model.link?.let { link -> ShareRow(words, link, "multisig.proposal") }
                if (model.outsideCanSign) {
                    VelaSecondaryButton(
                        text = words.t("external.signWithWallet"),
                        onClick = { hub.sendProposal(ev("approve_outside", "chain_id" to chainId, "safe_tx_hash" to item.safeTxHash)) },
                        modifier = Modifier.fillMaxWidth().testTag("multisig.proposal.outside"),
                        // §12: an approval asked of another wallet is still an approval — the same door.
                        enabled = model.canSign,
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
        // r10-7: approvals that "count until it is executed" — not said of one that was.
        if (!model.isDone) MsNote(words.t("proposal.noExpiry"))
    }
}

/**
 * r9-5: the haptic a settled press gets — counted, nothing for the person's
 * own cancel or "don't sign", and the refusal buzz only beside the words
 * that say it ([ActionFailure]).
 */
internal fun settledHaptic(kind: String?): VelaHaptic? = when (kind) {
    "done" -> VelaHaptic.Counted
    null, "cancelled", "riskDeclined", "outsideAsked" -> null
    else -> VelaHaptic.Reject
}

/** r9-5: why a press did not go, said under its button, with a way on (read again, then press again). */
@Composable
private fun ActionFailure(lines: List<MsLine>, words: MultisigWords, retryLabel: String? = null, onRetry: () -> Unit) {
    if (lines.isEmpty()) return
    Row(verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            for (line in lines) MsNote(line.text, tone = line.tone, tag = "multisig.proposal.note.${line.id}")
        }
        MsTextButton(retryLabel ?: words.t("service.retry"), tag = "multisig.proposal.failureRetry") { onRetry() }
    }
}

/**
 * r15-7 + r16-15: what an execution costs, under its press — the fee line
 * on its own, then the one-time setup as ONE block: its head ("Includes
 * one-time setup on {network} ≈ {amount}:") and the lines it introduces
 * drawn together. Each line drawn as its own note put the sheet's section
 * gap between a head ending in a colon and its first item.
 */
@Composable
private fun ExecutionLines(lines: List<MsLine>, feeTag: String, setupTag: String) {
    lines.firstOrNull { it.id == "fee" }?.let { MsNote(it.text, tone = it.tone, tag = feeTag) }
    val setup = lines.filter { it.id != "fee" }
    if (setup.isEmpty()) return
    Column(Modifier.fillMaxWidth().testTag(setupTag), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
        setup.forEachIndexed { i, line ->
            MsText(if (i == 0) line.text else "· ${line.text}", MsType.Sub, color = VelaTheme.colors.fgMuted)
        }
    }
}

/**
 * r16-5: how a Reject (the cancellation's draft, `hub.propose`) ended, in the
 * core's words, when there is something to say — its network or the multisig
 * service not read, a refusal, a failure. `null` for the person's own
 * withdrawal on the review, a cancellation proposed, or nothing yet. Said
 * beside Reject with Try again: confirming did nothing visible while the
 * draft's reads failed (`multisig.reject settled kind=unread`).
 */
internal fun rejectFailureLine(words: MultisigWords, settled: JsonElement?, chainId: Int): MsLine? {
    val kind = settled?.kind ?: return null
    if (kind in setOf("done", "working", "cancelled", "riskDeclined", "outsideAsked")) return null
    // The person's own passkey "cancel" is their word, not a failure (iOS's `rejectSaysWhy`).
    val failure = MultisigShellFailure.read(settled["detail"].str)
    if (kind == "failed" && failure.kind == "passkey" && failure["failure"].str == "cancelled" && failure["timedOut"].bool != true) return null
    val text = words.actionFailure(settled, chainId) ?: return null
    val tone = if (kind == "unread" || kind == "mailboxUnreadable") MsTone.Warning else MsTone.Danger
    return MsLine(text, tone, "rejectFailed")
}

/** r13-3: what a linked transaction's stand offers. Pure. */
object LinkedStands {
    /**
     * A link whose read failed — not read in time (`failed`), its network
     * silent (`unreadable`), the multisig service silent (`notFound`) —
     * offers Try again whenever the core says so (`retry`, §18 item 1): the
     * machine's `retry`, which reads it again.
     */
    fun offersTryAgain(linked: app.getvela.wallet.feature.multisig.core.MsLinkedView?): Boolean =
        linked?.retry == true

    /** The core's words for the read that failed (`reason_key`), or `null` when the core offers no Try again. */
    fun failure(linked: app.getvela.wallet.feature.multisig.core.MsLinkedView?, words: MultisigWords): String? {
        val key = linked?.reasonKey ?: return null
        return if (offersTryAgain(linked)) words.unreadText(key, linked.chainId) else null
    }

    /** What a sheet opened without a link says while its transaction is not in the list. */
    data class Missing(val text: String, val retry: Boolean)

    /**
     * §18 (b), round 14 + round 16 leftover 3: a sheet opened WITHOUT a link
     * whose transaction is not in the list says why, in the core's words
     * (`review.missing`, iOS's `missingSaid`): its network not read, the
     * multisig service not read — or busy (its rate limit: the home's busy
     * words, never "Couldn't reach the multisig service") — each with Try
     * again; read and not there; not a network the multisig is read on.
     * `null` while it is still being looked for ("Opening…"), and for a sheet
     * opened from a link (its stand says it). Pure.
     */
    fun missing(view: app.getvela.wallet.feature.multisig.core.MsProposalsView, chainId: Int, words: MultisigWords): Missing? {
        if (view.linked != null) return null
        val said = app.getvela.wallet.feature.multisig.core.MsRules.missingSaid(
            // The list of no multisig shown is not read yet: still opening.
            loading = view.loading || view.followed == null,
            unreadable = chainId in view.unreadable,
            mailbox = view.chain(chainId)?.mailbox,
            busy = view.busy,
        )
        if (said.key == app.getvela.wallet.feature.multisig.core.MsMissingSaid.OPENING_KEY) return null
        return Missing(words.unreadText(said.key, chainId), said.retry)
    }
}

/** Where a linked transaction stands before its item reads. */
@Composable
private fun LinkedState(
    stand: String?,
    preview: Boolean,
    words: MultisigWords,
    failure: String? = null,
    onTryAgain: (() -> Unit)? = null,
    missing: LinkedStands.Missing? = null,
) {
    when {
        // r13-3 / §18 item 1: a read that failed (`failed`, `unreadable`,
        // `notFound`) — the read named, with Try again; never "Opening…" for
        // ever, never "it hasn't been shared".
        failure != null -> {
            MsNote(failure, tone = MsTone.Warning, tag = "multisig.proposal.openFailed")
            onTryAgain?.let { retry ->
                VelaSecondaryButton(words.t("review.state.tryAgain"), retry, Modifier.fillMaxWidth().testTag("multisig.proposal.openRetry"))
            }
        }
        stand == "notInMailbox" -> MsNote(words.t("proposal.notInMailbox"), tone = MsTone.Warning, tag = "multisig.proposal.notInMailbox")
        // §18 (b) + round 16 leftover 3: opened without a link and not in the
        // list — the core's words for why (a busy service said busy), with
        // Try again when its read did not come back.
        missing != null -> {
            MsNote(missing.text, tone = MsTone.Warning, tag = "multisig.proposal.missing")
            if (missing.retry && onTryAgain != null) {
                VelaSecondaryButton(words.t("review.state.tryAgain"), onTryAgain, Modifier.fillMaxWidth().testTag("multisig.proposal.missingRetry"))
            }
        }
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

/**
 * r13 S4: when an open sheet's timer asks the machine for an executor's
 * look again. Only a look that has not priced it (none yet, `unpriced`,
 * `unchecked`) is asked again by the timer, at most [AUTO_LIMIT] times
 * (the fee policy's `AUTO_REQUOTE_LIMIT`) — then "Check again", the app
 * coming back to the front, or the transaction moving asks. A priced look,
 * or one that says it cannot run, stands: Execute looks again itself before
 * its passkey when the look is older than the machine's freshness window.
 */
internal object LookAsk {
    const val ASK_EVERY_MS = 4_000L
    const val AUTO_LIMIT = 4

    /**
     * r14-6: past its first few asks, a look that still has not priced it is
     * asked again this often — the network may be back (a fee line said
     * "Couldn't check whether it would run" for minutes after Arc answered
     * again). A priced look still stands (r13 S4): only what could not be
     * said is asked again.
     */
    const val SLOW_EVERY_MS = 20_000L

    fun timerAsks(kind: String?, autoAsked: Int, sinceLastAutoMs: Long = 0L): Boolean =
        (kind == null || kind == "unpriced" || kind == "unchecked") &&
            (autoAsked < AUTO_LIMIT || sinceLastAutoMs >= SLOW_EVERY_MS)

    /** r14-6: a fee line that could not be worked out or checked offers "Check again" — beside Execute and beside the one press. */
    fun offersCheckAgain(kind: String?): Boolean = kind == "unpriced" || kind == "unchecked"
}

/** How soon a review that could not read the chain is asked again (the web's `REVIEW_RETRY_MS`). */
internal const val REVIEW_RETRY_MS = 5_000L

/**
 * A review that was read, but whose chain reads did not all come back (its
 * state or its simulation): asked again soon. One that could not be read at
 * all is the core's `failed` state (§12) — said with "Try again", and how
 * long "Reading the transaction…" may stand is the core's bound, not this
 * sheet's.
 */
internal fun reviewUnread(review: JsonElement?): Boolean =
    review != null && review !is JsonNull &&
        (review["state"]["verdict"]["kind"].str == "unavailable" || review["simulation"]["kind"].str == "unreachable")
