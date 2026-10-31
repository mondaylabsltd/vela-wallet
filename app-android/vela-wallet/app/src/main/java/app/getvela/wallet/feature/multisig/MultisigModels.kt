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

    val title: String get() = words.actionTitle(chainId, item.action, item.nonce)
    val statusLine: String get() = words.standing(standing, item)

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
            item.refusal?.takeIf { it.isNotEmpty() }?.let { out += MsLine(words.t("proposal.refused", "reason" to it), MsTone.Danger, "refused") }
            if (item.cancelledBy != null && item.action.kind != "cancel") out += MsLine(words.t("proposal.cancelPending"), MsTone.Warning, "cancelPending")
            if (item.sameNonce.isNotEmpty() && !isDone) out += MsLine(words.t("proposal.conflict", "nonce" to item.nonce), MsTone.Warning, "conflict")
            if (item.status == "queued") out += MsLine(words.t("proposal.queuedNote", "nonce" to previousNonce), MsTone.Neutral, "queued")
            if (item.renameUndeployed) out += MsLine(words.t("proposal.renameUndeployed", "network" to words.network(chainId)), MsTone.Warning, "renameUndeployed")
            if (standing.kind == "blocked") out += MsLine(words.t("status.cannotRun", "reason" to words.blockReason(standing["block"])), MsTone.Warning, "blocked")
            if (item.unsent) {
                view.outbox.entries.firstOrNull { it.key.lowercase().endsWith(hash.lowercase()) }?.value?.let {
                    out += MsLine(outboxLine(it), MsTone.Warning, "outbox")
                }
            }
            executionLine?.let { out += it }
            words.actionFailure(approveState, chainId)?.let { out += MsLine(it, MsTone.Danger, "approveFailed") }
            words.actionFailure(executeState, chainId)?.let { out += MsLine(it, MsTone.Danger, "executeFailed") }
            if (canApprove.isEmpty() && !approvedByHere && !isDone && account != null && account.owners.isNotEmpty() && canExecute.isEmpty()) {
                out += MsLine(words.t("proposal.notMember"), MsTone.Neutral, "notMember")
            }
            return out
        }

    /** r8-16: this one's outbox entry is being sent now (the core's `sending`): no Send again. */
    val outboxSending: Boolean
        get() = view.outbox.entries.firstOrNull { it.key.lowercase().endsWith(hash.lowercase()) }?.value
            ?.let { it["sending"].bool == true || it["why"].str == "sending" } ?: false

    private val previousNonce: String get() = ((item.nonce.toLongOrNull() ?: 1L) - 1).coerceAtLeast(0).toString()

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

    /** The preview stops it (would fail, nobody can pay): Execute waits. */
    fun executeHeld(executor: String): Boolean {
        val preview = preview(executor) ?: return false
        if (preview.kind in setOf("fails", "refused", "spent")) return true
        return preview.kind == "priced" && preview["canPay"].bool == false
    }

    // -- asking the others (`signers`) -------------------------------------------------

    val stillNeeded: List<String> get() = members.filter { it.lowercase() !in approvedBy }

    val signerLines: List<MsLine>
        get() {
            val s = signers ?: return emptyList()
            if (isDone) return emptyList()
            val out = mutableListOf<MsLine>()
            if (s.inVela.isNotEmpty()) out += MsLine(words.t("proposal.approvesInVela", "names" to s.inVela.joinToString(", ", transform = words::label)), MsTone.Neutral, "inVela")
            if (s.either.isNotEmpty()) out += MsLine(words.t("proposal.eitherWay", "names" to s.either.joinToString(", ", transform = words::label)), MsTone.Neutral, "either")
            if (s.contract.isNotEmpty()) out += MsLine(words.t("proposal.contractMembers", "names" to s.contract.joinToString(", ", transform = words::label)), MsTone.Neutral, "contract")
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
    val networks = { note["chain_ids"].arr.mapNotNull { it.int }.joinToString(", ", transform = words::network) }
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
