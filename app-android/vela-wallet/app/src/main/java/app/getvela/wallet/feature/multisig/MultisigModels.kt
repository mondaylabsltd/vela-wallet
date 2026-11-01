package app.getvela.wallet.feature.multisig

import app.getvela.wallet.feature.multisig.core.MsAccount
import app.getvela.wallet.feature.multisig.core.MsProposalItem
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MsSigners
import app.getvela.wallet.feature.multisig.core.MsWaiting
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull

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
        get() = if (isDone) approvalsCount else words.standingLine(standing, item)

    /** r10-7: the execution's own line is said once — the notes' line, else the chain's word for it. */
    val executedSaidInNotes: Boolean get() = executionLine?.id == "executed"

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

    /**
     * The core's sign rule (`SignRule`): `allowed` false — nothing is signed,
     * no override; `anyway` — allowed, but only through the signing sheet's
     * own friction: a danger-styled "… anyway" and a second, explicit
     * confirmation (r8-1).
     */
    val signAllowed: Boolean get() = review["sign"]["allowed"].bool ?: true
    val signAnyway: Boolean get() = review["sign"]["anyway"].bool ?: false

    /**
     * r8-1: the one review every signer signs from is on screen. Before it
     * is, no action may sign — a tap on a sheet still filling in (its review
     * landing pushes the buttons down under the finger) approved a request
     * nobody had read.
     */
    val reviewReady: Boolean get() = review.let { it != null && it !is JsonNull }

    /** A press may sign: the review is read and its rule allows it. */
    val canSign: Boolean get() = reviewReady && signAllowed

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
            if (standing.kind == "blocked") out += MsLine(words.t("status.cannotRun", "reason" to words.blockReason(standing["block"])), MsTone.Warning, "blocked")
            if (item.unsent) {
                view.outbox.entries.firstOrNull { it.key.lowercase().endsWith(hash.lowercase()) }?.value?.let {
                    out += MsLine(outboxLine(it), MsTone.Warning, "outbox")
                }
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
    val approveFailure: MsLine? get() = words.actionFailure(approveState, chainId)?.let { MsLine(it, MsTone.Danger, "approveFailed") }
    val executeFailure: MsLine? get() = words.actionFailure(executeState, chainId)?.let { MsLine(it, words.actionFailureTone(executeState), "executeFailed") }

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

    /** r8-16: this one's outbox entry is being sent now (the core's `sending`): no Send again. */
    val outboxSending: Boolean
        get() = view.outbox.entries.firstOrNull { it.key.lowercase().endsWith(hash.lowercase()) }?.value
            ?.let { it["sending"].bool == true || it["why"].str == "sending" } ?: false

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
        val seconds = entry["retryAt"].long?.let { ((it - nowMs()) / 1000).coerceAtLeast(0) } ?: 0
        // r8-16: on its way to the service right now — said so, and no Send again.
        if (entry["sending"].bool == true || entry["why"].str == "sending") return words.t("proposal.outboxSending")
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
                val fee = "${MsRules.formatUnits(preview["fee"]["amount"].str ?: "0", 18)} $symbol"
                if (preview["canPay"].bool == false) {
                    words.t(
                        "proposal.cannotPay",
                        "name" to words.label(executor), "symbol" to symbol, "fee" to fee,
                        "balance" to "${MsRules.formatUnits(preview["balance"].str ?: "0", 18)} $symbol",
                    ) to MsTone.Warning
                } else {
                    words.t("proposal.feeLine", "fee" to fee, "name" to words.label(executor)) to MsTone.Neutral
                }
            }
            "fails" -> words.t("proposal.wouldFail", "reason" to words.whyFails(preview["why"])) to MsTone.Danger
            "unpriced" -> words.t("proposal.feeUnpriced") to MsTone.Warning
            "unchecked" -> words.t("proposal.feeUnchecked") to MsTone.Warning
            "refused" -> words.t("authority.deployUnverified", "network" to words.network(chainId)) to MsTone.Danger
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
            words.t("proposal.feeSetup", "network" to words.network(chainId), "amount" to "${MsRules.formatUnits(it, 18)} ${words.symbol(chainId)}")
        } ?: words.t("proposal.feeSetupPlain", "network" to words.network(chainId))
        return listOf(head) + lines
    }

    /**
     * r10 S4: why [member]'s approval — which would complete it — is offered
     * as Approve alone: its look says it cannot pay the fee, or it would
     * fail. `null` when theirs would not complete it, or nothing stops it.
     */
    fun approveOnlyWhy(member: String): Pair<String, MsTone>? {
        if (item.approved + 1 < item.threshold) return null
        if (view.approveExecuteBy(item).any { it.equals(member, ignoreCase = true) }) return null
        val look = preview(member) ?: return null
        val stops = look.kind == "fails" || (look.kind == "priced" && look["canPay"].bool == false)
        return if (stops) feeLine(member) else null
    }

    /** The preview stops it (would fail, nobody can pay): Execute waits. */
    fun executeHeld(executor: String): Boolean {
        val preview = preview(executor) ?: return false
        if (preview.kind in setOf("fails", "refused", "spent")) return true
        return preview.kind == "priced" && preview["canPay"].bool == false
    }

    // -- asking the others (`signers`) -------------------------------------------------

    val stillNeeded: List<String> get() = members.filter { it.lowercase() !in approvedBy }

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
 * The group's one note (`waiting_note`), and whether Retry goes with it —
 * the core says which; this only words it. [time] words a moment (ms).
 */
fun waitingNote(words: MultisigWords, note: JsonElement?, time: (Long) -> String): Pair<String, Boolean>? {
    note ?: return null
    val networks = { words.list(note["chain_ids"].arr.mapNotNull { it.int }.map(words::network)) }
    return when (note.kind) {
        "offline" -> note["since"].long?.let { words.t("service.offlineSince", "time" to time(it)) to false } ?: (words.t("service.offline") to false)
        "unreachable" -> words.t("service.unreachableNote") to true
        "chainUnreachable" -> words.t("service.chainUnreachable", "network" to networks()) to true
        "lastKnown" -> words.t("panel.lastKnown", "time" to time(note["at"].long ?: 0)) to false
        "stale" -> words.t("panel.stale") to false
        "historyUnreadable" -> words.t("panel.historyUnreadable", "network" to networks()) to true
        "checking" -> words.t("panel.checking") to false
        else -> null
    }
}
