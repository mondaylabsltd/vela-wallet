package app.getvela.wallet.feature.multisig

import app.getvela.wallet.feature.multisig.core.MsAccount
import app.getvela.wallet.feature.multisig.core.MsProposalItem
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MsSignDoor
import app.getvela.wallet.feature.multisig.core.MsSigners
import app.getvela.wallet.feature.multisig.core.MsWaiting
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.isNullish
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/** One line a sheet says, with how it reads and a stable tag. */
data class MsLine(val text: String, val tone: MsTone, val id: String)

/** A member's word in the approvals list, and whether it is a done one (drawn in the success tone). */
data class MemberState(val text: String, val done: Boolean)

/**
 * What one multisig transaction's sheet draws, read from the proposals
 * machine's view — apart from the composable, so a JVM test reads every
 * branch (iOS's `MsProposalModel`). Every verdict is the machine's: this
 * only chooses the words.
 */
class MsProposalModel(
    val item: MsProposalItem,
    val view: MsProposalsView,
    val words: MultisigWords,
    val account: MsAccount?,
    /** Where links point and in which language (`shareOrigin`, the app's page locale). */
    private val origin: String = "https://getvela.app",
    private val locale: String = "en",
    private val nowMs: () -> Long = { System.currentTimeMillis() },
) {
    val chainId: Int get() = item.chainId
    val hash: String get() = item.safeTxHash
    val review: JsonElement? get() = view.review(item)
    val standing: JsonElement? get() = view.standing(item)
    val timeline: List<JsonElement> get() = view.timeline(item)
    val signers: MsSigners? get() = view.signers(item)
    val execution: JsonElement? get() = view.execution(item)
    val approveState: JsonElement? get() = view.action("approve:$hash")
    val executeState: JsonElement? get() = view.action("execute:$hash")

    /**
     * r10-8: the core's reading names a call or a batch ("Approve · USDC",
     * "Batch of 1 call", as the web and the desktop head it); else the
     * multisig's own line (a member change, a cancellation, a rename).
     */
    val title: String get() = readingTitle(review, chainId, words) ?: words.actionTitle(chainId, item.action, item.nonce)
    val statusLine: String
        // r10-7 (round 11, live): an executed one has no standing — the chip
        // fell back to today's count and read "1/2 approved" over an
        // execution that counted 2 of 2. Finished: what the heading says.
        get() = when {
            // r16-8 (iOS's finding, Android parity): executed from here — the
            // press's passkey and send, before the chain has it — is
            // "Executing…", never the mailbox's "1/2 approved" (the one press
            // carries the last approval itself). r17-6: and whenever the body
            // says "Executing…" (the execution followed here is `waiting`),
            // checked BEFORE a finished one's count: the mailbox may say done
            // while the chain has not answered yet, and the chip read nothing
            // (or a count) over "Executing…".
            chipSaysExecuting -> words.t("status.executing")
            isDone -> approvalsCount
            else -> words.standingLine(standing, item)
        }

    /**
     * r17-6: the chip says "Executing…" — the body does (the execution's
     * verdict is `waiting`), or this device's press is out (its `execute:`
     * action works, or the item is executing).
     */
    val chipSaysExecuting: Boolean
        get() = execution.kind == "waiting" || executeState.kind == "working" || item.executing

    /** r10-7: the execution's own line is said once — the notes' line, else the chain's word for it. */
    val executedSaidInNotes: Boolean get() = executionLine?.id == "executed"

    /**
     * r12 S4: the notes the sheet draws under its heading — never the chip's
     * own word again (an executed one's chip says "Executed"; the green
     * "Executed" box under it said it a second time).
     */
    val sheetNotes: List<MsLine>
        get() = notes.filterNot {
            (isDone && it.text == statusLine) ||
                // r17-6: the chip says "Executing…"; the execution's own
                // "Executing…" note beside it said it a second time.
                (chipSaysExecuting && it.id == "waiting")
        }

    /** r12 S4: the sheet's own "Executed" line, for an execution neither the chip nor the notes say. */
    val saysExecuted: Boolean get() = !executedSaidInNotes && statusLine != words.t("proposal.executed")

    /** Who approved, by address (lowercased). */
    val approvedBy: Set<String> get() = item.approvers.map { it.lowercase() }.toSet()

    /** The members on this network now (the chain's word), else the original. */
    val members: List<String> get() = view.chain(chainId)?.members?.owners ?: account?.owners ?: emptyList()

    val isDone: Boolean get() = item.status == "done"
    val isReady: Boolean get() = item.status == "ready"
    val canApprove: List<String> get() = item.canApprove
    val canExecute: List<String> get() = item.canExecute

    /** The member this device approves as by default: the one used last, else the first. */
    val defaultMember: String?
        get() = view.lastActor?.let { last -> canApprove.firstOrNull { it.equals(last, ignoreCase = true) } } ?: canApprove.firstOrNull()

    /** §12: where its review stands (`MultisigReviewState`; `null`: not asked yet). */
    val reviewState: JsonElement? get() = view.reviewState(item)

    /**
     * §12, the core's sign door (`review.door`) over the review's state and
     * its sign rule (`SignRule`) — one answer for Approve, Approve and
     * execute, Execute and "Approve with another wallet": enabled only when
     * the review is `ready` and its rule allows it; `anyway` — allowed, but
     * only through the signing sheet's own friction (a danger-styled
     * "… anyway" and a second, explicit confirmation, r8-1). A missing
     * review, a missing state or a missing sign rule is never "allowed".
     */
    val door: MsSignDoor by lazy { MsRules.reviewDoor(reviewState, review) }

    val signAnyway: Boolean get() = door.anyway

    /**
     * r8-1: the one review every signer signs from is on screen (the door
     * shows it). Before it is, no action may sign — a tap on a sheet still
     * filling in (its review landing pushes the buttons down under the
     * finger) approved a request nobody had read.
     */
    val reviewReady: Boolean get() = door.showsReview

    /** A press may sign: the core's door is open. */
    val canSign: Boolean get() = door.enabled

    /** §12: the event "Try again" sends — this proposal's review, read again. */
    val reviewAgain: JsonObject get() = ev("review_again", "chain_id" to chainId, "safe_tx_hash" to hash)

    /**
     * r8-18 (the web's `approveFirst`): Approve is offered only while
     * approvals are missing. A transaction already at its threshold (2/2)
     * is executed, never approved again — its main button is Execute.
     */
    val showApprove: Boolean get() = canApprove.isNotEmpty() && item.approved < item.threshold

    /** The danger lines a "… anyway" press is confirmed over: the review's own, said again in the confirmation. */
    val dangerLines: List<String>
        get() {
            val r = review ?: return emptyList()
            return shownConcerns(r).mapNotNull { reviewConcern(words, it) }.filter { it.second == MsTone.Danger }.map { it.first }
        }

    /** One of this device's members approved it already. */
    val approvedByHere: Boolean
        get() = canApprove.isEmpty() && item.approvers.any { owner ->
            members.any { it.equals(owner, ignoreCase = true) } && words.localName(owner) != null
        }

    /** The lines that say where it stands, in order. */
    val notes: List<MsLine>
        get() {
            val out = mutableListOf<MsLine>()
            // r9-9: a review that names its refusal (`refused.key`) says it in its
            // body, once; the core's English `refusal` only from a core without it.
            if (review["refused"]["key"].str == null) {
                item.refusal?.takeIf { it.isNotEmpty() }?.let { out += MsLine(words.t("proposal.refused", "reason" to it), MsTone.Danger, "refused") }
            }
            if (item.status == "queued") out += MsLine(queuedNote(), MsTone.Neutral, "queued")
            if (item.renameUndeployed) out += MsLine(words.t("proposal.renameUndeployed", "network" to words.network(chainId)), MsTone.Warning, "renameUndeployed")
            if (standing.kind == "blocked") out += MsLine(words.t("status.cannotRun", "reason" to words.blockReason(standing["block"], chainId)), MsTone.Warning, "blocked")
            // r17-16: an approval signed here and not in the mailbox is always
            // worded, never a bare Send again — its outbox entry's state, else
            // (none kept) "Not sent yet". On its way now is no warning.
            if (item.unsent) {
                val entry = outboxEntry
                out += MsLine(entry?.let(::outboxLine) ?: words.t("proposal.outboxNotSent"), if (outboxSending) MsTone.Neutral else MsTone.Warning, "outbox")
            }
            executionLine?.let { out += it }
            // r9-9: "none is signed in on this device" only when the core says no
            // member of it is here (`members_here`) — never because a refused
            // transaction leaves nobody able to approve it.
            val noMemberHere = item.membersHere?.isEmpty()
                ?: (canApprove.isEmpty() && !approvedByHere && canExecute.isEmpty() && account != null && account.owners.isNotEmpty())
            if (noMemberHere && !isDone) {
                out += MsLine(words.t("proposal.notMember"), MsTone.Neutral, "notMember")
            }
            return out
        }

    /**
     * r9-5: why the last approve (or execute) pressed here did not go, in
     * the machine's words (`MultisigActionState`) — said right under the
     * button that was pressed, with a way forward, never only a haptic (and
     * never only at the top of a sheet scrolled to its buttons). `null`
     * while it works, once done, or for the person's own cancel.
     */
    val approveFailure: MsLine?
        get() = if (staleAfter("approve", "execute")) null else words.actionFailure(approveState, chainId)?.let { MsLine(it, MsTone.Danger, "approveFailed") }
    val executeFailure: MsLine?
        get() = if (staleAfter("execute", "approve")) null else words.actionFailure(executeState, chainId)?.let { MsLine(it, words.actionFailureTone(executeState), "executeFailed") }

    /**
     * r17-16: [what]'s failure is stale — a press of [other] on this same
     * transaction came after it and went through (`done`). The core keeps
     * each action's last state apart (`approve:` / `execute:`), with no order
     * between them; the order is the shell's, from when each went to
     * `working` ([MsProposalsView.pressOrder]). Not known: drawn, never
     * hidden on a guess.
     */
    private fun staleAfter(what: String, other: String): Boolean {
        if (view.action("$other:$hash").kind != "done") return false
        val mine = view.pressOrder["$what:${hash.lowercase()}"] ?: return false
        val theirs = view.pressOrder["$other:${hash.lowercase()}"] ?: return false
        return theirs > mine
    }

    /**
     * r11-9: the fee line under the button, unless the action's own state
     * speaks for the look now — one box, never the look's line and the
     * action's beside it (the action state replaces the fee line).
     */
    fun feeLineBeside(executor: String, failure: MsLine?): Pair<String, MsTone>? {
        val line = feeLine(executor) ?: return null
        return if (failure != null && (failure.text == line.first || executeState.kind == "stopped")) null else line
    }

    /**
     * r9-13: the core's view of a transaction a cancellation waits on — only
     * one at its number can run (`sameNonce`), and a cancellation of it is
     * waiting (`cancelledBy`): said beside its actions, as the web does.
     */
    val cancelNotes: List<MsLine>
        get() = buildList {
            if (item.sameNonce.isNotEmpty() && !isDone) add(MsLine(words.t("proposal.conflict", "nonce" to item.nonce), MsTone.Warning, "conflict"))
            if (item.cancelledBy != null && item.action.kind != "cancel" && !isDone) add(MsLine(words.t("proposal.cancelPending"), MsTone.Warning, "cancelPending"))
        }

    /** r4D-13: proposed from a member on this device — withdrawn, not rejected (the web's `ownProposal`). */
    val ownProposal: Boolean get() = item.proposer.isNotEmpty() && words.localName(item.proposer) != null

    /**
     * This transaction's entry in the machine's outbox (signed here, not in
     * the mailbox yet). The core keys it `chainId:safe:safeTxHash:owner`
     * (`outbox_key`) — r17-16: it was matched as ENDING in the hash, so no
     * entry was ever found and a kept approval was never worded.
     */
    val outboxEntry: JsonElement?
        get() {
            if (!item.unsent) return null
            val marker = ":${hash.lowercase()}:"
            return view.outbox.entries.firstOrNull { (key, _) ->
                val k = key.lowercase()
                k.startsWith("$chainId:") && k.contains(marker)
            }?.value
        }

    /** r8-16: this one's outbox entry is being sent now (the core's `sending`): no Send again. */
    val outboxSending: Boolean
        get() = outboxEntry?.let { it["sending"].bool == true || it["why"].str == "sending" } ?: false

    /**
     * "Send again" only when nothing is on its way and no retry is due (the
     * web's and iOS's r8-16): a kept approval the core sends again on its
     * own is said with its countdown, never offered a second way.
     */
    val offersSendAgain: Boolean
        get() {
            if (!item.unsent) return false
            val entry = outboxEntry ?: return true
            return !outboxSending && entry["retryAt"].long == null
        }

    private val previousNonce: String get() = ((item.nonce.toLongOrNull() ?: 1L) - 1).coerceAtLeast(0).toString()

    /**
     * r11 S4: "It can run once…" — every number it waits behind, the core's
     * (`waits_for`): one named, several listed as "#n" in the person's list
     * form. The previous number alone only from a core without the list.
     */
    internal fun queuedNote(): String = when (item.waitsFor.size) {
        0 -> words.t("proposal.queuedNote", "nonce" to previousNonce)
        1 -> words.t("proposal.queuedNote", "nonce" to item.waitsFor.single())
        else -> words.t("proposal.queuedNoteMany", "nonces" to words.list(item.waitsFor.map { "#$it" }))
    }

    private fun outboxLine(entry: JsonElement): String {
        // r8-16: on its way to the service right now — said so, and no Send again.
        if (entry["sending"].bool == true || entry["why"].str == "sending") return words.t("proposal.outboxSending")
        // Sent again only on the person's word: "Not sent yet" (with Send again).
        val retryAt = entry["retryAt"].long ?: return words.t("proposal.outboxNotSent")
        val seconds = ((retryAt - nowMs() + 999) / 1000).coerceAtLeast(1)
        return when (entry["why"].str) {
            "busy" -> words.t("proposal.outboxBusy", "seconds" to seconds.toString())
            "away", "unreachable" -> words.t("proposal.outboxAway", "seconds" to seconds.toString())
            else -> words.t("proposal.outboxNotSent")
        }
    }

    /** How an execution followed here ended (`executions`), in words. */
    private val executionLine: MsLine?
        get() {
            val verdict = execution ?: return null
            val paid = verdict["paid"].bool == true
            return when (verdict.kind) {
                "executed" -> MsLine(words.t("proposal.executed"), MsTone.Positive, "executed")
                "executedByOther" -> MsLine(words.t(if (paid) "proposal.lostRacePaid" else "proposal.lostRace"), MsTone.Warning, "lostRace")
                "replaced" -> MsLine(words.t(if (paid) "proposal.replacedFirstPaid" else "proposal.replacedFirst", "nonce" to item.nonce), MsTone.Warning, "replaced")
                "failed" -> MsLine(words.t("proposal.execFailed"), MsTone.Danger, "execFailed")
                "notSent" -> MsLine(words.t("proposal.notSent"), MsTone.Danger, "notSent")
                "unknown" -> MsLine(words.t("proposal.cantCheckUsed", "network" to words.network(chainId)), MsTone.Warning, "unknown")
                "timedOut" -> MsLine(words.t("proposal.notLanded"), MsTone.Warning, "timedOut")
                "waiting" -> MsLine(words.t("proposal.executing"), MsTone.Neutral, "waiting")
                else -> null
            }
        }

    /** The executed transaction's hash, when the chain says it ran. */
    val executedTx: String?
        get() = execution["txHash"].str
            ?: timeline.firstOrNull { it.kind == "executed" && it["tx_hash"].str != null }?.get("tx_hash").str

    // -- the executor's look (`previews`) -------------------------------------------

    fun preview(executor: String): JsonElement? = view.preview(item, executor)

    /** The fee line under Execute, for [executor]. */
    fun feeLine(executor: String): Pair<String, MsTone>? {
        val preview = preview(executor) ?: return words.t("proposal.feeChecking") to MsTone.Neutral
        val symbol = words.symbol(chainId)
        return when (preview.kind) {
            "priced" -> {
                val fee = "${MsRules.formatUnits(preview["fee"]["amount"].str ?: "0", MsRules.COIN_DECIMALS)} $symbol"
                if (preview["canPay"].bool == false) {
                    words.t(
                        "proposal.cannotPay",
                        "name" to words.label(executor), "symbol" to symbol, "fee" to fee,
                        "balance" to "${MsRules.formatUnits(preview["balance"].str ?: "0", MsRules.COIN_DECIMALS)} $symbol",
                    ) to MsTone.Warning
                } else {
                    words.t("proposal.feeLine", "fee" to fee, "name" to words.label(executor)) to MsTone.Neutral
                }
            }
            "fails" -> words.t("proposal.wouldFail", "reason" to words.whyFails(preview["why"], chainId)) to MsTone.Danger
            "unpriced" -> words.t("proposal.feeUnpriced") to MsTone.Warning
            // D8: "you can still approve" only where Approve can be pressed —
            // never under one shut while the review is read (or not read).
            "unchecked" -> words.t(if (door.enabled) "proposal.feeUnchecked" else "proposal.feeUncheckedHeld") to MsTone.Warning
            // r14-4: a setup the core could not check (`unverified`) is never
            // said as one it read and refused (`refused`).
            "unverified" -> words.t("authority.deployUnverified", "network" to words.network(chainId)) to MsTone.Warning
            "refused" -> words.deployRefused(preview["authority"], chainId) to MsTone.Danger
            "spent" -> words.t("proposal.doneNote") to MsTone.Neutral
            else -> null
        }
    }

    /** The setup an execution carries (deploying the multisig, activating accounts). */
    fun setupLines(executor: String): List<String> {
        val preview = preview(executor) ?: return emptyList()
        val setup = preview["setup"] ?: return emptyList()
        if (preview.kind != "priced") return emptyList()
        val lines = mutableListOf<String>()
        if (setup["multisig"].bool == true) lines += words.t("proposal.setupMultisig")
        setup["members"].arr.mapNotNull { it.str }.forEach { lines += words.t("proposal.setupMember", "name" to words.label(it)) }
        if (setup["executor"].bool == true) lines += words.t("proposal.setupExecutor", "name" to words.label(executor))
        if (lines.isEmpty()) return emptyList()
        val head = setup["amount"].str?.let {
            words.t("proposal.feeSetup", "network" to words.network(chainId), "amount" to "${MsRules.formatUnits(it, MsRules.COIN_DECIMALS)} ${words.symbol(chainId)}")
        } ?: words.t("proposal.feeSetupPlain", "network" to words.network(chainId))
        return listOf(head) + lines
    }

    /**
     * r16-14: what is said under the presses about executing it — the fee
     * lines, the setup, "Check again", why the one press is not offered —
     * only while a press can sign. While the review is not read (or not
     * signable) the door's own reason is the one note: the looks the
     * timer takes meanwhile come and go with the same outage, and a label
     * or a fee line that followed them flipped under a shut button.
     */
    val saysExecution: Boolean get() = canSign

    /**
     * core-ms-2 + r16-14: the main button is "Approve and execute" for
     * [chosen] — the core offers them the one press (`approve_execute`) and
     * a press can sign. While the door is shut it reads Approve, steady.
     */
    fun oneTapShown(chosen: String?): Boolean =
        saysExecution && chosen != null && view.approveExecuteBy(item).any { it.equals(chosen, ignoreCase = true) }

    /**
     * r16-4: the approval alone, beside the one press (the web's and iOS's
     * second button): the one press does both or nothing, so approving
     * without executing is the person's own press.
     */
    fun approveAloneBeside(chosen: String?): Boolean = oneTapShown(chosen)

    /**
     * r16-4: the execution stopped only because its fee could not be worked
     * out or checked at the press — its way on is a new look ("Check
     * again"), not a read of the list.
     */
    val failureChecksFee: Boolean
        get() = executeFailure != null && executeState.kind == "stopped" && executeState["preview"].kind in setOf("unpriced", "unchecked")

    /**
     * r14-6 + r16-4: "Check again" under the one press — a look that could
     * not price or check it, while a press can sign and no stop beside the
     * press already offers it (said once).
     */
    fun oneTapCheckAgain(chosen: String): Boolean =
        saysExecution && executeFailure == null && LookAsk.offersCheckAgain(preview(chosen).kind)

    /**
     * r10 S4: why [member]'s approval — which would complete it — is offered
     * as Approve alone: its look says it cannot pay the fee, or it would
     * fail. `null` when theirs would not complete it, or nothing stops it.
     */
    fun approveOnlyWhy(member: String): Pair<String, MsTone>? {
        if (item.approved + 1 < item.threshold) return null
        if (view.approveExecuteBy(item).any { it.equals(member, ignoreCase = true) }) return null
        val look = preview(member) ?: return null
        // r15-7: a setup whose Safe contracts went unread (`unverified`), or
        // one read and refused, stops the one press too — said here, as iOS
        // and the web say it, never left unsaid.
        val stops = look.kind in setOf("fails", "refused", "unverified") || (look.kind == "priced" && look["canPay"].bool == false)
        return if (stops) feeLine(member) else null
    }

    /**
     * r15-7: what an execution by [executor] costs, said before the press —
     * its fee line ([feeLineBeside]) and, for a multisig not deployed yet,
     * the one-time setup it carries ([setupLines]). The one press and
     * Execute draw the same lines.
     */
    fun executionLines(executor: String, failure: MsLine?): List<MsLine> = buildList {
        feeLineBeside(executor, failure)?.let { (text, tone) -> add(MsLine(text, tone, "fee")) }
        setupLines(executor).forEachIndexed { i, line -> add(MsLine(line, MsTone.Neutral, "setup$i")) }
    }

    /**
     * §24: the fee figure this sheet draws for [executor]'s execution (base
     * units, as the core gave it) — what a press under it carries as
     * `shown_fee`, so the core signs exactly that figure or stops with
     * `fee_moved` (§19.11). `null` when no figure is drawn: no priced look,
     * or the action's own line stands in the fee line's place. Read from the
     * model the sheet painted, never from a view not drawn yet.
     */
    fun shownFee(executor: String): String? {
        val look = preview(executor) ?: return null
        if (look.kind != "priced" || feeLineBeside(executor, executeFailure) == null) return null
        return look["fee"]["amount"].str?.takeIf { it.isNotBlank() }
    }

    /**
     * §24: the press that executes — Execute, or the one press "Approve and
     * execute" ([oneTap]) — carrying the figure drawn under it ([shownFee]).
     */
    fun executeEvent(executor: String, oneTap: Boolean): JsonObject {
        val fields = mutableListOf<Pair<String, Any?>>("chain_id" to chainId, "safe_tx_hash" to hash, "executor" to executor)
        shownFee(executor)?.let { fields += "shown_fee" to it }
        return ev(if (oneTap) "approve_execute" else "execute", *fields.toTypedArray())
    }

    /** The preview stops it (would fail, nobody can pay): Execute waits. */
    fun executeHeld(executor: String): Boolean {
        val preview = preview(executor) ?: return false
        if (preview.kind in setOf("fails", "refused", "unverified", "spent")) return true
        return preview.kind == "priced" && preview["canPay"].bool == false
    }

    // -- asking the others (`signers`) -------------------------------------------------

    val stillNeeded: List<String> get() = members.filter { it.lowercase() !in approvedBy }

    /**
     * r11-4, r12-4 (the web's `nobodySigns`): the core refuses it — the one
     * review's `refused` (a batch nested too deep, a delegatecall, a refund, a
     * state change), its sign rule shut, or no kind Vela signs at all. Every
     * member's Vela runs the same review, so nobody signs it: nobody is asked
     * to. Read from the full review (`reviews[…]`); the item's own review is
     * the thin one (action, refusal) and never carries `refused`.
     */
    val nobodySigns: Boolean
        get() = item.action.isNullish ||
            !review["refused"].isNullish ||
            review["sign"]["allowed"].bool == false ||
            // The core left nobody here who can act, and said why.
            (canApprove.isEmpty() && canExecute.isEmpty() && !item.refusal.isNullOrEmpty())

    /**
     * r12-4: the members still to approve are asked — "Ask members to
     * approve", Send link, Copy link, the outside-wallet link and "Approve
     * with another wallet" — only for an open one somebody can sign.
     */
    val asksMembers: Boolean
        get() = !isDone && !nobodySigns && stillNeeded.isNotEmpty() && item.approved < item.threshold

    // -- the approvals section (r10-7; the web's r4a-8) ---------------------------------

    /** How it ran, as the machine read it (`frozen`): never today's members re-counted. */
    val frozen: JsonElement? get() = view.frozen(item)

    /**
     * The approvals heading's count: an executed one says "Executed" (never
     * "1/2" beside an execution that counted 2), one that finished otherwise
     * nothing, an open one its approvals against today's threshold.
     */
    val approvalsCount: String
        get() = when {
            isDone && (frozen != null || timeline.any { it.kind == "executed" }) -> words.t("proposal.executed")
            isDone -> ""
            else -> "${item.approved}/${item.threshold}"
        }

    /**
     * Whom the list names: the members whose approvals counted when it ran;
     * an executed one's approvers while those are not known (never a
     * "Waiting" row on one that can no longer move); else the members now.
     */
    val listedMembers: List<String>
        get() = frozen?.get("approvers").arr.mapNotNull { it.str }.takeIf { it.isNotEmpty() }
            ?: if (isDone) item.approvers else members

    /** A listed member's one word: Executed (its press approved and sent it), Approved, Not needed, Waiting, or nothing. */
    fun memberState(member: String): MemberState {
        val f = frozen
        if (f != null && f["executorApproved"].bool == true && f["executor"].str.equals(member, ignoreCase = true)) {
            return MemberState(words.t("proposal.executed"), done = true)
        }
        if (f != null || member.lowercase() in approvedBy) return MemberState(words.t("proposal.approved"), done = true)
        return when {
            isDone -> MemberState("", done = false)
            item.approved >= item.threshold -> MemberState(words.t("proposal.notNeeded"), done = false)
            else -> MemberState(words.t("proposal.waiting"), done = false)
        }
    }

    /** Under the list of an executed one: who sent it and what counted then; or that this device does not know which counted. */
    val executedFacts: String?
        get() {
            if (!isDone) return null
            val f = frozen
            if (f == null) return if (timeline.any { it.kind == "executed" }) words.t("proposal.approvalsSigned") else null
            return listOfNotNull(
                f["executor"].str?.takeIf { it.isNotBlank() }?.let { words.t("proposal.executedBy", "name" to words.label(it)) },
                words.t(
                    "proposal.countedThen",
                    "approved" to f["approvers"].arr.size.toString(),
                    "threshold" to (f["threshold"].long ?: item.threshold.toLong()).toString(),
                ),
            ).joinToString(" · ")
        }

    val signerLines: List<MsLine>
        get() {
            val s = signers ?: return emptyList()
            if (isDone) return emptyList()
            val out = mutableListOf<MsLine>()
            if (s.inVela.isNotEmpty()) out += MsLine(words.t("proposal.approvesInVela", "names" to words.list(s.inVela.map(words::label))), MsTone.Neutral, "inVela")
            if (s.either.isNotEmpty()) out += MsLine(words.t("proposal.eitherWay", "names" to words.list(s.either.map(words::label))), MsTone.Neutral, "either")
            if (s.contract.isNotEmpty()) out += MsLine(words.t("proposal.contractMembers", "names" to words.list(s.contract.map(words::label))), MsTone.Neutral, "contract")
            return out
        }

    /** An outside wallet may sign for one still to approve (the core's split: `outside`, `either`). */
    val outsideCanSign: Boolean get() = signers?.let { s -> (s.outside + s.either).any(::stillToApprove) } ?: false

    private fun stillToApprove(address: String): Boolean = stillNeeded.any { it.equals(address, ignoreCase = true) }

    /** r8-8: the members still to approve whom the core's split says sign with an outside wallet (`outside`). */
    val outsideMembers: List<String> get() = signers?.outside?.filter(::stillToApprove) ?: emptyList()

    /**
     * r8-8 (the web's `velaMissing`): members still to approve the Vela link
     * is for — in Vela elsewhere, either way, or not read yet; never this
     * device's own, a contract, or a surely outside wallet.
     */
    val velaMissing: List<String>
        get() {
            val s = signers
            return stillNeeded.filter { owner ->
                words.localName(owner) == null && when {
                    s == null -> true
                    s.inVela.any { it.equals(owner, ignoreCase = true) } || s.either.any { it.equals(owner, ignoreCase = true) } -> true
                    else -> s.rows.any { (o, kind) -> o.equals(owner, ignoreCase = true) && kind == null }
                }
            }
        }

    /** The Vela link is shown (the web's rule): someone needs it, or nothing else is offered at all. */
    val showVelaLink: Boolean
        get() = velaMissing.isNotEmpty() || (outsideMembers.isEmpty() && (signers?.contract ?: emptyList()).none(::stillToApprove))

    /**
     * r8-8 (r4C-13): the one link an outside member opens where its wallet
     * is — Vela's connect page naming the transaction, which reads, reviews
     * and signs it alone. The core writes it (`connectLink`); `null` when the
     * core's split names no outside member still to approve.
     */
    val outsideLink: String?
        get() {
            val outside = outsideMembers
            if (outside.isEmpty()) return null
            val multisig = account?.address ?: view.followed ?: return null
            return MsRules.connectLink("${origin.trimEnd('/')}/$locale/connect", null, outside, Triple(chainId, multisig, hash))
        }

    /** The proposal's link, for another member's Vela. */
    val link: String?
        get() = MsRules.proposalLink(origin, locale, chainId, account?.address ?: view.followed.orEmpty(), hash)

    // -- history --------------------------------------------------------------------

    fun timelineLine(step: JsonElement): String? = when (step.kind) {
        "created" -> words.t("timeline.created", "name" to words.label(step["who"].str.orEmpty()))
        "approved" -> {
            val who = words.label(step["who"].str.orEmpty())
            var line = words.t(if (step["by_executing"].bool == true) "timeline.approvedByExecuting" else "timeline.approved", "name" to who)
            step["counted"].bool?.let { counted -> line += " · " + words.t(if (counted) "timeline.counted" else "timeline.notCounted") }
            line
        }
        "executing" -> words.t("proposal.executing")
        "executed" -> step["who"].str?.let { words.t("timeline.executedBy", "name" to words.label(it)) } ?: words.t("timeline.executed")
        "cancelProposed" -> words.t("proposal.cancelPending")
        "replaced" -> words.t("proposal.voided")
        else -> null
    }
}

/**
 * §18 item 3: the card's note, beside Activity. While Activity's empty state
 * says the history could not be read — with Try again (r13-7) — the card does
 * not say it again: one sentence and one control per failure. Every other
 * note, and this one when Activity shows rows, stays the card's. Pure.
 */
fun cardNote(note: JsonElement?, historySaidInActivity: Boolean): JsonElement? =
    if (historySaidInActivity && note.kind == "historyUnreadable") null else note

/**
 * The group's one note (`waiting_note`), and whether Retry goes with it —
 * the core says which; this only words it. [time] words a moment (ms).
 */
fun waitingNote(words: MultisigWords, note: JsonElement?, time: (Long) -> String): Pair<String, Boolean>? {
    note ?: return null
    val networks = { words.list(note["chain_ids"].arr.mapNotNull { it.int }.map(words::network)) }
    return when (note.kind) {
        "offline" -> note["since"].long?.let { words.t("service.offlineSince", "time" to time(it)) to false } ?: (words.t("service.offline") to false)
        "unreachable" -> words.t("service.unreachableNote") to true
        // r16-16: the service answered its rate limit — busy, with Retry.
        "busy" -> words.t("service.busyNote") to true
        "chainUnreachable" -> words.t("service.chainUnreachable", "network" to networks()) to true
        "lastKnown" -> words.t("panel.lastKnown", "time" to time(note["at"].long ?: 0)) to false
        "stale" -> words.t("panel.stale") to false
        "historyUnreadable" -> words.t("panel.historyUnreadable", "network" to networks()) to true
        "checking" -> words.t("panel.checking") to false
        else -> null
    }
}
