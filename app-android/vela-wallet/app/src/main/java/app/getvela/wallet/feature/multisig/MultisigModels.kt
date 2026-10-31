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

    /** The core's sign rule: may this be signed at all — `anyway` only with the "… anyway" friction. */
    val signAllowed: Boolean get() = review["sign"]["allowed"].bool ?: true
    val signAnyway: Boolean get() = review["sign"]["anyway"].bool ?: false

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

    private val previousNonce: String get() = ((item.nonce.toLongOrNull() ?: 1L) - 1).coerceAtLeast(0).toString()

    private fun outboxLine(entry: JsonElement): String {
        val seconds = entry["retryAt"].long?.let { ((it - nowMs()) / 1000).coerceAtLeast(0) } ?: 0
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
    val outsideCanSign: Boolean get() = signers?.let { (it.outside + it.either).isNotEmpty() } ?: false

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

/** The home's waiting line ("1 needs your approval · 1 ready"), or `null` when nothing waits. */
fun waitingLine(words: MultisigWords, waiting: MsWaiting?): String? {
    waiting ?: return null
    val parts = mutableListOf<String>()
    if (waiting.needsYou > 0) parts += words.plural("presence.needsYou", waiting.needsYou)
    if (waiting.ready > 0) parts += words.plural("presence.ready", waiting.ready)
    if (waiting.waitingPayer > 0) parts += words.plural("presence.waitingPayer", waiting.waitingPayer)
    return parts.takeIf { it.isNotEmpty() }?.joinToString(" · ")
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
