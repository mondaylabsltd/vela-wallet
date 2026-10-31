package app.getvela.wallet.feature.multisig.core

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/*
 * The multisig machines' views, typed for the screens — read from the core's
 * JSON (`app-web/vela-wallet/src/lib/multisig/generated/` is the wire's
 * mirror), never re-derived. Every field is lenient: a field this build does
 * not know is skipped, one it expects and is not there reads as its empty
 * value, so a newer core never blanks a screen (iOS's `MultisigViews.swift`).
 *
 * Parts the wire carries as `unknown` (a stored record, the one review, a
 * preview) are kept whole as [JsonElement] and read through the accessors in
 * `MsJson.kt`.
 */

private fun JsonElement?.strings(): List<String> = arr.mapNotNull { it.str }
internal fun JsonElement?.ints(): List<Int> = arr.mapNotNull { it.int }

// -- Accounts ------------------------------------------------------------------

/** What the multisigs here keep of a member (`MultisigMemberInfo`): a label, never authority. */
data class MsMemberInfo(val name: String?, val kind: String?, val typed: Boolean?) {
    companion object {
        fun of(e: JsonElement?) = MsMemberInfo(e["name"].str, e["kind"].str, e["typed"].bool)
    }
}

/** A multisig on this device — the stored record (`MultisigAccount`), with the record whole in [raw]. */
data class MsAccount(
    val address: String,
    val owners: List<String>,
    val threshold: Int,
    val recorded: Boolean,
    val shared: Boolean?,
    val chainIds: List<Int>,
    val label: String?,
    val members: Map<String, MsMemberInfo>,
    val firstRun: Boolean?,
    val genesisName: String?,
    val raw: JsonObject,
) {
    val id: String get() = address.lowercase()

    companion object {
        fun of(e: JsonElement?): MsAccount = MsAccount(
            address = e["address"].str.orEmpty(),
            owners = e["owners"].strings(),
            threshold = e["threshold"].int ?: 1,
            recorded = e["recorded"].bool ?: false,
            shared = e["shared"].bool,
            chainIds = e["chainIds"].ints(),
            label = e["label"].str,
            members = e["members"].obj.mapValues { MsMemberInfo.of(it.value) },
            firstRun = e["firstRun"].bool,
            genesisName = e["genesis"]["name"].str,
            raw = e.obj,
        )
    }
}

data class MsAccountRow(
    val account: MsAccount,
    val name: String?,
    val shared: Boolean,
    val recordWaiting: Boolean,
    val localMembers: List<String>,
    val highRiskOn: List<Int>,
    val refusedOn: List<Int>,
    val checkedOn: List<Int>,
    val checkingOn: List<Int>,
    val inviteShared: Boolean,
) {
    val id: String get() = account.id

    companion object {
        fun of(e: JsonElement?) = MsAccountRow(
            account = MsAccount.of(e["account"]),
            name = e["name"].str,
            shared = e["shared"].bool ?: false,
            recordWaiting = e["record_waiting"].bool ?: false,
            localMembers = e["local_members"].strings(),
            highRiskOn = e["high_risk_on"].ints(),
            refusedOn = e["refused_on"].ints(),
            checkedOn = e["checked_on"].ints(),
            checkingOn = e["checking_on"].ints(),
            inviteShared = e["invite_shared"].bool ?: false,
        )
    }
}

/** `MultisigPhase`: idle, working, done, cancelled, failed. */
enum class MsPhase {
    Idle, Working, Done, Cancelled, Failed;

    companion object {
        fun of(s: String?): MsPhase = when (s) {
            "working" -> Working
            "done" -> Done
            "cancelled" -> Cancelled
            "failed" -> Failed
            else -> Idle
        }
    }
}

data class MsCreateDraft(
    val owners: List<String>,
    val members: Map<String, MsMemberInfo>,
    val name: String,
    val threshold: Int,
    val chainId: Int?,
) {
    companion object {
        fun of(e: JsonElement?) = MsCreateDraft(
            owners = e["owners"].strings(),
            members = e["members"].obj.mapValues { MsMemberInfo.of(it.value) },
            name = e["name"].str.orEmpty(),
            threshold = e["threshold"].int ?: 1,
            chainId = e["chainId"].int,
        )
    }
}

data class MsCreateView(
    val phase: MsPhase = MsPhase.Idle,
    val draftAddress: String? = null,
    val draftError: String? = null,
    val address: String? = null,
    val refused: String? = null,
    val error: String? = null,
    /** `present` / `missing` / `unknown`, per chain id. */
    val networks: Map<String, JsonElement> = emptyMap(),
    val otherSpace: List<Int> = emptyList(),
    val kept: MsCreateDraft? = null,
    /**
     * core-ms-2: the threshold step for the draft's members
     * (`MultisigThresholdView` `{value, members, note, risky, can_lower,
     * can_raise}`); `null` before a draft names members. Its `value` is the
     * threshold sent with `draft` / `keep_draft`.
     */
    val threshold: JsonElement? = null,
) {
    /** The Safe contracts on [chainId]: `present`, `missing`, `unknown`, or `null` (not asked). */
    fun contracts(chainId: Int): String? = networks[chainId.toString()].let { it.str ?: it.kind }

    companion object {
        fun of(e: JsonElement?) = MsCreateView(
            phase = MsPhase.of(e["phase"].str),
            draftAddress = e["draft_address"].str,
            draftError = e["draft_error"].str,
            address = e["address"].str,
            refused = e["refused"].str,
            error = e["error"].str,
            networks = e["networks"].obj,
            otherSpace = e["other_space"].ints(),
            kept = e["kept"]?.let(MsCreateDraft::of),
            threshold = e["threshold"],
        )
    }
}

data class MsAddView(
    val phase: MsPhase = MsPhase.Idle,
    val address: String? = null,
    val outcome: JsonElement? = null,
    val found: JsonElement? = null,
    val ens: JsonElement? = null,
) {
    companion object {
        fun of(e: JsonElement?) = MsAddView(
            phase = MsPhase.of(e["phase"].str),
            address = e["address"].str,
            outcome = e["outcome"],
            found = e["found"],
            ens = e["ens"],
        )
    }
}

data class MsImportView(
    val safes: List<JsonElement> = emptyList(),
    val error: String? = null,
    val busy: String? = null,
    val outcomes: Map<String, JsonElement> = emptyMap(),
) {
    companion object {
        fun of(e: JsonElement?) = MsImportView(
            safes = e["safes"].arr,
            error = e["error"].str,
            busy = e["busy"].str,
            outcomes = e["outcomes"].obj,
        )
    }
}

data class MsRenamePlan(val chainId: Int?, val deployed: Boolean?, val blocked: String?, val landsOn: Int?) {
    companion object {
        fun of(e: JsonElement?) = MsRenamePlan(
            chainId = e["chain_id"].int,
            deployed = e["deployed"].bool,
            blocked = e["blocked"].str ?: e["blocked"].kind,
            landsOn = e["lands_on"].int,
        )
    }
}

data class MsKindChoice(val kind: String?, val suggested: String?, val locked: Boolean, val note: String?) {
    companion object {
        fun of(e: JsonElement?) = MsKindChoice(
            kind = e["kind"].str,
            suggested = e["suggested"].str,
            locked = e["locked"].bool ?: false,
            note = e["note"].str,
        )
    }
}

/** Where a link opened here stands (`MultisigLinkStand`). */
data class MsLinkStand(
    val kind: String,
    val link: JsonElement?,
    val text: String?,
    val fault: String?,
    val address: String?,
    val keptBefore: Boolean,
    val kept: Boolean,
    val reason: JsonElement?,
    val raw: JsonElement?,
) {
    val chainId: Int? get() = link["chainId"].int
    val multisig: String? get() = link["multisig"].str
    val safeTxHash: String? get() = link["safeTxHash"].str
    val chainIds: List<Int> get() = link["chainIds"].ints()

    companion object {
        fun of(e: JsonElement?) = MsLinkStand(
            kind = e["kind"].str.orEmpty(),
            link = e["link"],
            text = e["text"].str,
            fault = e["fault"].str,
            address = e["address"].str,
            keptBefore = e["kept_before"].bool ?: false,
            kept = e["kept"].bool ?: false,
            reason = e["reason"],
            raw = e,
        )
    }
}

data class MsAccountsView(
    val loaded: Boolean = false,
    val accounts: List<MsAccountRow> = emptyList(),
    val active: String? = null,
    val create: MsCreateView = MsCreateView(),
    val add: MsAddView = MsAddView(),
    val importing: MsImportView = MsImportView(),
    val backup: Map<String, JsonElement> = emptyMap(),
    val shareAgain: Map<String, MsPhase> = emptyMap(),
    val shareAgainFailure: Map<String, String> = emptyMap(),
    val rename: Map<String, MsRenamePlan> = emptyMap(),
    val serviceDown: Boolean = false,
    val link: MsLinkStand? = null,
    val memberKinds: Map<String, MsKindChoice> = emptyMap(),
    val knownMembers: Map<String, MsMemberInfo> = emptyMap(),
    /** core-ms-2: the invite the add sheet shows (`{link, on_device, missing, missing_name}`). */
    val invite: JsonElement? = null,
    /** core-ms-2: where this device's own transactions were (`hints.recent`). */
    val recentNetworks: List<Int> = emptyList(),
    /** core-ms-2: where the multisigs here are used (`hints.used`). */
    val usedNetworks: List<Int> = emptyList(),
) {
    /** The core's hints for `networks.likely` / `networks.firstDefault`, with the wallet's own. */
    fun networkHints(here: Int? = null): Map<String, Any?> = mapOf(
        "here" to listOfNotNull(here),
        "used" to usedNetworks,
        "recent" to recentNetworks,
    )

    fun row(address: String?): MsAccountRow? =
        address?.let { a -> accounts.firstOrNull { it.account.address.equals(a, ignoreCase = true) } }

    /** The multisig shown, when one is. */
    val activeRow: MsAccountRow? get() = row(active)

    /** Keyed by an address, any case. */
    fun <T> lookup(map: Map<String, T>, address: String): T? =
        map[address] ?: map[address.lowercase()] ?: map.entries.firstOrNull { it.key.equals(address, ignoreCase = true) }?.value

    companion object {
        fun of(e: JsonElement) = MsAccountsView(
            loaded = e["loaded"].bool ?: false,
            accounts = e["accounts"].arr.map(MsAccountRow::of),
            active = e["active"].str,
            create = MsCreateView.of(e["create"]),
            add = MsAddView.of(e["add"]),
            importing = MsImportView.of(e["import"]),
            backup = e["backup"].obj,
            shareAgain = e["share_again"].obj.mapValues { MsPhase.of(it.value.str) },
            shareAgainFailure = e["share_again_failure"].obj.mapValues { it.value.str.orEmpty() },
            rename = e["rename"].obj.mapValues { MsRenamePlan.of(it.value) },
            serviceDown = e["service_down"].bool ?: false,
            link = e["link"]?.let(MsLinkStand::of),
            memberKinds = e["member_kinds"].obj.mapValues { MsKindChoice.of(it.value) },
            knownMembers = e["known_members"].obj.mapValues { MsMemberInfo.of(it.value) },
            invite = e["invite"],
            recentNetworks = e["recent_networks"].ints(),
            usedNetworks = e["used_networks"].ints(),
        )
    }
}

// -- Proposals -----------------------------------------------------------------

data class MsProposalItem(
    val chainId: Int,
    val safeTxHash: String,
    val tx: JsonElement?,
    val review: JsonElement?,
    val nonce: String,
    val proposer: String,
    val approvals: List<JsonElement>,
    val approved: Int,
    val threshold: Int,
    val status: String,
    val canApprove: List<String>,
    val canExecute: List<String>,
    val cancelledBy: String?,
    val sameNonce: List<String>,
    val executing: Boolean,
    val createdMs: Double,
    val unsent: Boolean,
    val renameUndeployed: Boolean,
    val raw: JsonElement?,
) {
    val id: String get() = "$chainId:${safeTxHash.lowercase()}"

    /** What it does, as the core read it (`Review.action`). */
    val action: JsonElement? get() = review["action"]
    val refusal: String? get() = review["refusal"].str
    val approvers: List<String> get() = approvals.mapNotNull { it["owner"].str }

    companion object {
        fun of(e: JsonElement?) = MsProposalItem(
            chainId = e["chainId"].int ?: 0,
            safeTxHash = e["safeTxHash"].str.orEmpty(),
            tx = e["tx"],
            review = e["review"],
            nonce = e["nonce"].str ?: "0",
            proposer = e["proposer"].str.orEmpty(),
            approvals = e["approvals"].arr,
            approved = e["approved"].int ?: 0,
            threshold = e["threshold"].int ?: 1,
            status = e["status"].str ?: "pending",
            canApprove = e["canApprove"].strings(),
            canExecute = e["canExecute"].strings(),
            cancelledBy = e["cancelledBy"].str,
            sameNonce = e["sameNonce"].strings(),
            executing = e["executing"].bool ?: false,
            createdMs = e["createdMs"].long?.toDouble() ?: 0.0,
            unsent = e["unsent"].bool ?: false,
            renameUndeployed = e["renameUndeployed"].bool ?: false,
            raw = e,
        )
    }
}

data class MsChainMembers(val deployed: Boolean, val owners: List<String>, val threshold: Int, val nonce: String) {
    companion object {
        fun of(e: JsonElement?) = MsChainMembers(
            deployed = e["deployed"].bool ?: false,
            owners = e["owners"].strings(),
            threshold = e["threshold"].int ?: 1,
            nonce = e["nonce"].str ?: "0",
        )
    }
}

data class MsChainState(val chainId: Int, val members: MsChainMembers?, val items: List<MsProposalItem>, val mailbox: Boolean) {
    companion object {
        fun of(e: JsonElement?) = MsChainState(
            chainId = e["chainId"].int ?: 0,
            members = e["members"]?.let(MsChainMembers::of),
            items = e["items"].arr.map(MsProposalItem::of),
            mailbox = e["mailbox"].bool ?: false,
        )
    }
}

data class MsDraft(
    val item: MsProposalItem,
    val review: JsonElement?,
    val membersHere: List<String>,
    val defaultMember: String?,
    /** core-ms-2: the proposer's own approval makes it ready (a site's transaction): proposed and executed in one press (`propose_execute`). */
    val executeNow: Boolean = false,
) {
    companion object {
        fun of(e: JsonElement?) = MsDraft(
            item = MsProposalItem.of(e["item"]),
            review = e["review"],
            membersHere = e["membersHere"].strings(),
            defaultMember = e["defaultMember"].str,
            executeNow = e["executeNow"].bool ?: false,
        )
    }
}

/** Who is still to approve, each said once (W9, r7-8). */
data class MsSigners(
    val rows: List<Pair<String, String?>>,
    val outside: List<String>,
    val inVela: List<String>,
    val either: List<String>,
    val contract: List<String>,
    val enough: Boolean,
) {
    companion object {
        fun of(e: JsonElement?) = MsSigners(
            rows = e["rows"].arr.map { (it["owner"].str.orEmpty()) to it["kind"].str },
            outside = e["outside"].strings(),
            inVela = e["in_vela"].strings(),
            either = e["either"].strings(),
            contract = e["contract"].strings(),
            enough = e["enough"].bool ?: false,
        )
    }
}

/** What waits on one multisig for this device (`MultisigWaiting`). */
data class MsWaiting(val needsYou: Int, val ready: Int, val waitingPayer: Int, val first: JsonElement?) {
    val firstChainId: Int? get() = first["chain_id"].int ?: first["chainId"].int
    val firstHash: String? get() = first["safe_tx_hash"].str ?: first["safeTxHash"].str

    companion object {
        fun of(e: JsonElement?) = MsWaiting(
            needsYou = e["needs_you"].int ?: 0,
            ready = e["ready"].int ?: 0,
            waitingPayer = e["waiting_payer"].int ?: 0,
            first = e["first"],
        )
    }
}

data class MsLinkedView(
    val safe: String,
    val chainId: Int,
    val safeTxHash: String,
    val preview: Boolean,
    val stand: String,
    val item: MsProposalItem?,
    val members: MsChainMembers?,
    val stale: Boolean,
) {
    companion object {
        fun of(e: JsonElement?) = MsLinkedView(
            safe = e["safe"].str.orEmpty(),
            chainId = e["chain_id"].int ?: 0,
            safeTxHash = e["safe_tx_hash"].str.orEmpty(),
            preview = e["preview"].bool ?: false,
            stand = e["stand"].str ?: "reading",
            item = e["item"]?.let(MsProposalItem::of),
            members = e["members"]?.let(MsChainMembers::of),
            stale = e["stale"].bool ?: false,
        )
    }
}

data class MsProposalsView(
    val followed: String? = null,
    val chains: List<MsChainState> = emptyList(),
    val items: List<MsProposalItem> = emptyList(),
    val missing: List<JsonElement> = emptyList(),
    val unreadable: List<Int> = emptyList(),
    val historyUnreadable: List<Int> = emptyList(),
    val offline: Boolean = false,
    val loading: Boolean = false,
    val stale: Boolean = false,
    val unreachable: Boolean = false,
    val paceMs: Long = 12_000,
    val outbox: Map<String, JsonElement> = emptyMap(),
    val draft: MsDraft? = null,
    val actions: Map<String, JsonElement> = emptyMap(),
    val reviews: Map<String, JsonElement> = emptyMap(),
    val previews: Map<String, JsonElement> = emptyMap(),
    val executions: Map<String, JsonElement> = emptyMap(),
    val lastActor: String? = null,
    val presence: Map<String, MsWaiting?> = emptyMap(),
    val othersWaiting: Boolean = false,
    val membersAnywhere: List<String> = emptyList(),
    val standings: Map<String, JsonElement> = emptyMap(),
    val waitingNote: JsonElement? = null,
    val timelines: Map<String, List<JsonElement>> = emptyMap(),
    val dapp: Map<String, JsonElement> = emptyMap(),
    val callsStatus: Map<String, JsonElement> = emptyMap(),
    /** r8-2: the token facts the core read, per `chainId:tokenLowercase` (symbol, decimals — null when unknown). */
    val tokens: Map<String, JsonElement> = emptyMap(),
    val linked: MsLinkedView? = null,
    val signers: Map<String, MsSigners> = emptyMap(),
    val frozen: Map<String, JsonElement> = emptyMap(),
    /** core-ms-2: per `<chainId>:<hash lowercase>`, the members here whose one press approves and executes it. */
    val approveExecute: Map<String, List<String>> = emptyMap(),
    /** core-ms-2: the header's rule chip for the multisig followed (`MultisigRuleChip`). */
    val ruleChip: JsonElement? = null,
    /** core-ms-2: per multisig here (lowercase), the waiting line's parts, in order, nonzero only. */
    val waitingParts: Map<String, List<JsonElement>> = emptyMap(),
    /** core-ms-2: the line at the top of the followed multisig's home (`{parts, first}`), when something waits. */
    val banner: JsonElement? = null,
    /** core-ms-2: the followed multisig is deployed on a network read. */
    val setUp: Boolean = false,
) {
    /** The members here whose one press approves and executes [item] (the machine's `approve_execute`). */
    fun approveExecuteBy(item: MsProposalItem): List<String> = lookup(approveExecute, item.chainId, item.safeTxHash) ?: emptyList()

    /** The waiting line's parts for [address] (the machine's `waiting_parts`). */
    fun waitingPartsOf(address: String): List<JsonElement> =
        waitingParts[address.lowercase()] ?: waitingParts.entries.firstOrNull { it.key.equals(address, ignoreCase = true) }?.value ?: emptyList()

    fun item(chainId: Int, hash: String): MsProposalItem? =
        items.firstOrNull { it.chainId == chainId && it.safeTxHash.equals(hash, ignoreCase = true) }
            ?: chains.flatMap { it.items }.firstOrNull { it.chainId == chainId && it.safeTxHash.equals(hash, ignoreCase = true) }

    fun chain(chainId: Int): MsChainState? = chains.firstOrNull { it.chainId == chainId }

    /** `<chainId>:<hash>` maps are keyed as the core wrote the hash; both spellings are looked up. */
    fun <T> lookup(map: Map<String, T>, chainId: Int, hash: String): T? {
        val key = "$chainId:${hash.lowercase()}"
        return map["$chainId:$hash"] ?: map[key] ?: map.entries.firstOrNull { it.key.lowercase() == key }?.value
    }

    fun review(item: MsProposalItem): JsonElement? = lookup(reviews, item.chainId, item.safeTxHash)
    fun standing(item: MsProposalItem): JsonElement? = lookup(standings, item.chainId, item.safeTxHash)
    fun timeline(item: MsProposalItem): List<JsonElement> = lookup(timelines, item.chainId, item.safeTxHash) ?: emptyList()
    fun signers(item: MsProposalItem): MsSigners? = lookup(signers, item.chainId, item.safeTxHash)
    fun execution(item: MsProposalItem): JsonElement? = lookup(executions, item.chainId, item.safeTxHash)

    fun preview(item: MsProposalItem, executor: String): JsonElement? {
        val key = "${item.chainId}:${item.safeTxHash.lowercase()}:${executor.lowercase()}"
        return previews[key] ?: previews.entries.firstOrNull { it.key.lowercase() == key }?.value
    }

    fun action(key: String): JsonElement? = actions[key] ?: actions.entries.firstOrNull { it.key.equals(key, ignoreCase = true) }?.value

    companion object {
        fun of(e: JsonElement) = MsProposalsView(
            followed = e["followed"].str,
            chains = e["chains"].arr.map(MsChainState::of),
            items = e["items"].arr.map(MsProposalItem::of),
            missing = e["missing"].arr,
            unreadable = e["unreadable"].ints(),
            historyUnreadable = e["history_unreadable"].ints(),
            offline = e["offline"].bool ?: false,
            loading = e["loading"].bool ?: false,
            stale = e["stale"].bool ?: false,
            unreachable = e["unreachable"].bool ?: false,
            paceMs = e["pace_ms"].long ?: 12_000,
            outbox = e["outbox"].obj,
            draft = e["draft"]?.let(MsDraft::of),
            actions = e["actions"].obj,
            reviews = e["reviews"].obj,
            previews = e["previews"].obj,
            executions = e["executions"].obj,
            lastActor = e["last_actor"].str,
            presence = e["presence"].obj.mapValues { (_, v) -> if (v.isNullish) null else MsWaiting.of(v) },
            othersWaiting = e["others_waiting"].bool ?: false,
            membersAnywhere = e["members_anywhere"].strings(),
            standings = e["standings"].obj,
            waitingNote = e["waiting_note"],
            timelines = e["timelines"].obj.mapValues { it.value.arr },
            dapp = e["dapp"].obj,
            callsStatus = e["calls_status"].obj,
            tokens = e["tokens"].obj,
            linked = e["linked"]?.let(MsLinkedView::of),
            signers = e["signers"].obj.mapValues { MsSigners.of(it.value) },
            frozen = e["frozen"].obj,
            approveExecute = e["approve_execute"].obj.mapValues { it.value.strings() },
            ruleChip = e["rule_chip"],
            waitingParts = e["waiting_parts"].obj.mapValues { it.value.arr },
            banner = e["banner"],
            setUp = e["set_up"].bool ?: false,
        )
    }
}
