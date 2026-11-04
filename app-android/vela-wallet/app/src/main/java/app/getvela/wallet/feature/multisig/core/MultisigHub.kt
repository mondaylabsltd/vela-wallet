package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import uniffi.vela_core_uniffi.MultisigAccountsCore
import uniffi.vela_core_uniffi.MultisigMembersCore
import uniffi.vela_core_uniffi.MultisigProposalsCore

/** What the app shows for the multisig: one sheet at a time, as on the web. */
sealed interface MultisigSheet {
    data object Create : MultisigSheet
    data class Add(val prefill: String? = null) : MultisigSheet
    data class Invite(val address: String) : MultisigSheet
    data class Settings(val address: String) : MultisigSheet
    data class Members(val address: String) : MultisigSheet
    data class Proposal(val chainId: Int, val safeTxHash: String) : MultisigSheet
    /** The proposal drafted for the person's review, before its passkey. */
    data class Review(val chainId: Int) : MultisigSheet
    /** A link where it stands (the accounts machine's `link`). */
    data object Link : MultisigSheet
    /** A site's request to a multisig, held by the proposals machine (M5). */
    data class Dapp(val id: String) : MultisigSheet
}

/** The high-risk question (§3.5): on this network something besides the members can act. */
class MultisigRiskAsk(val chainId: Int, val safe: String, val findings: List<JsonElement>, private val resume: (Boolean) -> Unit) {
    @Volatile private var answered = false
    fun answer(signs: Boolean) {
        if (answered) return
        answered = true
        resume(signs)
    }
}

/**
 * The app-wide host of the core's long-lived multisig machines — one each,
 * over the one shell ([MultisigExecutor], the app's pipelines from
 * [MultisigLive]) — the web's `shell/machines.svelte.ts` and
 * `proposals-host.svelte.ts`, iOS's `MultisigHub.swift`.
 *
 * It boots with the app, feeds the machines the app's facts (this device's
 * Vela accounts as `MultisigLocalAccount`, its networks and the space's, the
 * multisig service in use), routes the notices one machine sends another
 * (ONE router, [route]), and ticks the timers the machines name
 * (docs/multisig-machines.md §1.4). It decides nothing: what a fact means is
 * the machines'.
 *
 * Screens read the typed views ([accountsView], [proposalsView],
 * [membersView]) and send events through the verbs below; a sheet the
 * machines open (a link's transaction, the risk question, an outside
 * member's wallet) is published here for the root to present.
 */
class MultisigHub(
    val live: MultisigShellPorts,
    private val facts: suspend () -> Facts,
    /** Executed rows to the Activity store; `true` when stored. */
    private val writeRows: suspend (List<org.json.JSONObject>) -> Boolean = { true },
    /** A token's symbol and decimals (`null` token: the chain's coin). */
    private val tokenInfo: suspend (chainId: Int, token: String?) -> Pair<String, Int> = { _, _ -> "ETH" to 18 },
    /** The proposal's own title, in words (the activity row's `intent`, a notification's title). */
    var titleOf: suspend (chainId: Int, action: JsonElement?, nonce: String) -> String = { _, _, _ -> "" },
    cores: Triple<CoreBridge, CoreBridge, CoreBridge>? = null,
    val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
) {
    /** The app's facts as the machines read them (iOS `MultisigHub.Facts`). */
    data class Facts(
        val serviceUrl: String,
        /** This device's Vela accounts as `MultisigLocalAccount`. */
        val accounts: List<Map<String, Any?>>,
        val networks: List<Int>,
        val space: List<Int>,
        /** The addresses the person knows (a review's look-alikes and first sends). */
        val known: List<Map<String, Any?>>,
        /** The address book, as the members machine reads it. */
        val contacts: List<Map<String, Any?>> = emptyList(),
    )

    private val made = cores ?: Triple(
        MultisigAccountsCore().asBridge(),
        MultisigMembersCore().asBridge(),
        MultisigProposalsCore().asBridge(),
    )

    val accounts = MultisigMachine(made.first, scope, live, "accounts")
    val members = MultisigMachine(made.second, scope, live, "members")
    val proposals = MultisigMachine(made.third, scope, live, "proposals")

    private val _accountsView = MutableStateFlow(MsAccountsView())
    val accountsView: StateFlow<MsAccountsView> = _accountsView.asStateFlow()

    private val _proposalsView = MutableStateFlow(MsProposalsView())
    val proposalsView: StateFlow<MsProposalsView> = _proposalsView.asStateFlow()

    val membersView: StateFlow<JsonObject> get() = members.view

    /** The multisig sheet open now (one at a time). */
    val sheet = MutableStateFlow<MultisigSheet?>(null)

    private val _riskAsk = MutableStateFlow<MultisigRiskAsk?>(null)
    /** The high-risk question, while a signature waits on it. */
    val riskAsk: StateFlow<MultisigRiskAsk?> = _riskAsk.asStateFlow()

    /** An outside member's wallet asked to sign (M4), while it is open. */
    val outsideSession = MutableStateFlow<MultisigOutsideSessionModel?>(null)

    /** The chain a proposal is drawn on for the person's review, while it is. */
    val drafting = MutableStateFlow<Int?>(null)

    /** Executed rows were written to the Activity: whoever shows it refreshes. */
    @Volatile var onRecorded: (() -> Unit)? = null

    /** The multisig shown changed (the home follows it). */
    @Volatile var onActive: ((String?) -> Unit)? = null

    /** An executed transaction, for the notifications (M6): title, chain, hash. */
    @Volatile var onExecuted: ((title: String, chainId: Int, safeTxHash: String) -> Unit)? = null

    /** Something worth saying changed while the app is away (M6): evaluated by the notifier. */
    @Volatile var onProposalsMoved: ((MsAccountsView, MsProposalsView) -> Unit)? = null

    /** A proposal was made here (the first is when notifications are asked for, M6). */
    @Volatile var onProposed: (() -> Unit)? = null

    /** An outside member's session (M4): built by the app, which owns the rooms. */
    @Volatile var openOutsideSession: ((request: JsonObject, onOutcome: (JsonObject) -> Unit) -> MultisigOutsideSessionModel?)? = null

    private val bootLock = Mutex()
    @Volatile private var booted = false
    @Volatile var loaded = false
        private set
    @Volatile private var followed: String? = null
    private var recordTimer: Job? = null
    private var proposalTimer: Job? = null
    private var othersTimer: Job? = null
    @Volatile private var checkedRecords = false
    @Volatile private var lastAccountsJson = ""
    @Volatile private var lastWaitingJson = ""
    @Volatile private var lastRecheckKey = ""
    @Volatile private var appActive = true
    @Volatile private var lastPace = 12_000L

    // -- boot -------------------------------------------------------------------

    /** Idempotent: the machines start from the app's facts; the accounts machine reads its store and says `loaded`. */
    suspend fun boot() {
        bootLock.withLock {
            if (booted) return
            booted = true
        }
        val f = facts()
        accounts.dispatch(ev("start", "service_url" to f.serviceUrl, "accounts" to f.accounts, "networks" to f.networks))
        accounts.dispatch(ev("space", "chain_ids" to f.space))
        proposals.dispatch(
            ev("start", "service_url" to f.serviceUrl, "accounts" to f.accounts, "networks" to f.networks, "known" to f.known),
        )
        accounts.until(30_000) { if (it["loaded"].bool == true) true else null }
        loaded = true
        accountsMoved(accounts.view.value)
        followRecords()
        followOthers()
    }

    /** This device's Vela accounts, read again (one was added or signed in). */
    suspend fun refreshLocal() {
        val f = facts()
        accounts.dispatch(ev("local_accounts", "accounts" to f.accounts))
        members.dispatch(ev("local_accounts", "local" to f.accounts))
        proposals.dispatch(ev("local_accounts", "accounts" to f.accounts))
        proposals.dispatch(ev("known", "known" to f.known))
    }

    /** The device's networks moved (one added, the space switched). */
    suspend fun networksChanged() {
        val f = facts()
        accounts.dispatch(ev("networks", "networks" to f.networks))
        accounts.dispatch(ev("space", "chain_ids" to f.space))
        proposals.dispatch(ev("networks", "networks" to f.networks))
    }

    /** The multisig service changed (Settings → Service endpoints). */
    suspend fun serviceChanged() {
        val url = facts().serviceUrl
        accounts.dispatch(ev("service", "url" to url))
        proposals.dispatch(ev("service", "url" to url))
    }

    // -- views moved ------------------------------------------------------------

    private fun accountsMoved(view: JsonObject) {
        val typed = MsAccountsView.of(view)
        _accountsView.value = typed
        if (!typed.loaded) return
        // Every multisig on this device, to the proposals machine (what waits
        // on each is its to read) — said again only when they changed.
        val records = view["accounts"].arr.mapNotNull { it["account"] }
        val json = records.toString()
        if (json != lastAccountsJson) {
            lastAccountsJson = json
            proposals.dispatch(ev("accounts", "accounts" to records))
            val shown = shownRecord()
            if (shown != null && followed.equals(typed.active, ignoreCase = true)) {
                proposals.dispatch(ev("account", "account" to shown))
            }
        }
        // Who can act for each multisig, per network (§3.5): read again when
        // the list or a multisig's networks move (the web's `recheck` effect).
        val recheckKey = typed.accounts.joinToString(";") { "${it.account.address}:${it.account.chainIds.joinToString(",")}" }
        if (recheckKey.isNotEmpty() && recheckKey != lastRecheckKey) {
            lastRecheckKey = recheckKey
            accounts.dispatch(ev("recheck"))
        }
        val active = typed.active
        if (!active.equals(followed, ignoreCase = true)) {
            followed = active
            proposals.dispatch(ev("follow", "account" to (shownRecord() ?: JsonNull)))
            // Once a session: networks another member made it used on (deployed
            // there, or proposed there) become its networks here too.
            if (active != null) scope.launch { accounts.dispatch(ev("discover", "address" to active, "chain_ids" to facts().space)) }
            onActive?.invoke(active)
            restartProposalTimer()
        }
    }

    private fun proposalsMoved(view: JsonObject) {
        val typed = MsProposalsView.of(view)
        _proposalsView.value = typed
        if (dappAnswers.isNotEmpty()) settleDapps(typed)
        if (loaded) onProposalsMoved?.invoke(_accountsView.value, typed)
        if (typed.paceMs != lastPace) {
            lastPace = typed.paceMs
            restartProposalTimer()
        }
        // The member changes waiting, as the members machine reads them.
        val waiting = typed.items.map { item ->
            mapOf(
                "chain_id" to item.chainId, "safe_tx_hash" to item.safeTxHash, "nonce" to item.nonce,
                "action" to (item.action ?: JsonNull), "approved" to item.approved,
                "threshold" to item.threshold, "status" to item.status,
            )
        }
        // Said once per change (the web's `#tell`): a dispatch per commit
        // would be a members view per proposals view.
        val told = jsonOf(waiting).toString()
        if (told == lastWaitingJson) return
        lastWaitingJson = told
        members.dispatch(ev("waiting", "proposals" to waiting))
    }

    /** The stored record of the multisig shown, whole (as the core keeps it). */
    fun shownRecord(): JsonElement? = record(_accountsView.value.active)

    fun record(address: String?): JsonElement? {
        address ?: return null
        return accounts.view.value["accounts"].arr
            .firstOrNull { it["account"]["address"].str.equals(address, ignoreCase = true) }
            ?.get("account")
    }

    // -- timers (docs/multisig-machines.md §1.4) --------------------------------

    /** The app came to the front or went away. */
    fun appActive(active: Boolean) {
        appActive = active
        if (!loaded) return
        if (active) {
            accounts.dispatch(ev("tick"))
            proposals.dispatch(ev("refresh", "fresh" to false))
            proposals.dispatch(ev("online", "online" to true))
        }
        restartProposalTimer()
    }

    private fun followRecords() {
        if (!checkedRecords) {
            checkedRecords = true
            accounts.dispatch(ev("check_records"))
        }
        accounts.dispatch(ev("tick"))
        recordTimer?.cancel()
        recordTimer = scope.launch {
            while (isActive) {
                delay(60_000)
                if (appActive) accounts.dispatch(ev("tick"))
            }
        }
    }

    private fun followOthers() {
        othersTimer?.cancel()
        proposals.dispatch(ev("others_tick"))
        othersTimer = scope.launch {
            while (isActive) {
                delay(60_000)
                if (appActive) proposals.dispatch(ev("others_tick"))
            }
        }
    }

    /** The shown multisig's list, at the machine's pace, while the app is up. */
    private fun restartProposalTimer() {
        proposalTimer?.cancel()
        if (followed == null || !appActive) return
        val pace = maxOf(2_000L, _proposalsView.value.paceMs)
        proposalTimer = scope.launch {
            while (isActive) {
                delay(pace)
                proposals.dispatch(ev("tick"))
            }
        }
    }

    /** A sheet holds a transaction: the machine's faster pace until let go. */
    fun watch(): () -> Unit {
        proposals.dispatch(ev("watch"))
        var released = false
        return {
            if (!released) {
                released = true
                proposals.dispatch(ev("unwatch"))
            }
        }
    }

    // -- verbs (accounts) ---------------------------------------------------------

    fun send(event: JsonObject) = accounts.dispatch(event)

    /** Show [address] (or the Vela account again, `null`). */
    fun activate(address: String?) = accounts.dispatch(ev("activate", "address" to address))

    fun openLink(hash: String, takeKept: Boolean = false) =
        accounts.dispatch(ev("open_link", "hash" to hash, "take_kept" to takeKept))

    /** Whether [address] is a multisig on this device (a site granted it). */
    fun isMultisig(address: String): Boolean = _accountsView.value.row(address) != null

    // -- verbs (members) ----------------------------------------------------------

    /**
     * The multisig Settings shows: its members read on each of its networks,
     * named from this device's accounts and what the multisigs here keep.
     */
    suspend fun showMembers(address: String) {
        val record = record(address) ?: return
        val f = facts()
        val chainIds = record["chainIds"].arr.mapNotNull { it.int }.distinct()
            .sortedBy { id -> f.networks.indexOf(id).let { if (it < 0) Int.MAX_VALUE else it } }
        val known = accounts.view.value["accounts"].arr.mapNotNull { it["account"] }
        members.dispatch(ev("show", "account" to record, "chain_ids" to chainIds, "local" to f.accounts, "known" to known))
        members.dispatch(ev("contacts", "contacts" to f.contacts))
    }

    fun sendMembers(event: JsonObject) = members.dispatch(event)

    // -- verbs (proposals) --------------------------------------------------------

    fun sendProposal(event: JsonObject) = proposals.dispatch(event)

    /** The sheet a review covered, shown again once it is answered. */
    @Volatile private var reviewReturn: MultisigSheet? = null

    /**
     * Propose [proposal] on [chainId] for the multisig shown (or [address]):
     * drawn with its review first (`draft`); the person confirms it on the
     * review sheet (`confirm_draft`). Answers once the machine settled it
     * (`done`: its safeTxHash) or why not — the action state, `kind` tagged.
     *
     * §12: there is no path that confirms a draft without its review sheet —
     * the sheet's button is the core's sign door (`review.door`), and the
     * core refuses (`signingUnchecked`) a draft whose review is not `ready`.
     */
    suspend fun propose(
        chainId: Int,
        proposal: Map<String, Any?>,
        address: String? = null,
    ): JsonElement? {
        if (address != null && !address.equals(followed, ignoreCase = true)) {
            record(address)?.let { rec ->
                proposals.dispatch(ev("follow", "account" to rec))
                followed = address
            }
        }
        fun state(view: JsonObject): JsonElement? = view["actions"]["propose"]
        val cancelled = jo("kind" to "cancelled")
        // Drawn (its draft up), or why not (its state) — read from the views
        // after the draft was applied, never the last proposal's "done".
        val drafted: JsonElement? = proposals.request(ev("draft", "chain_id" to chainId, "proposal" to proposal)) { view ->
            if (view["draft"] is JsonObject) return@request jo("kind" to DRAWN)
            val tagged = state(view) ?: return@request null
            if (tagged.kind == "working") null else tagged
        }
        if (drafted.kind != DRAWN) return drafted
        drafting.value = chainId
        reviewReturn = sheet.value
        sheet.value = MultisigSheet.Review(chainId)
        try {
            // The person's word on its review: confirmed (working), or withdrawn.
            val asked: JsonElement? = proposals.until { view ->
                val tagged = state(view)
                if (tagged.kind == "working" || view["draft"] !is JsonObject) tagged ?: cancelled else null
            }
            if (sheet.value == MultisigSheet.Review(chainId)) sheet.value = reviewReturn
            reviewReturn = null
            if (asked.kind != "working" && asked.kind != "done") return asked
        } finally {
            if (drafting.value == chainId) drafting.value = null
        }
        // No member here: an outside member's wallet proposes it — its answer settles it.
        val done: JsonElement? = proposals.until { view ->
            val tagged = state(view) ?: return@until null
            if (tagged.kind == "working" || tagged.kind == "outsideAsked") null else tagged
        }
        if (done.kind == "done") onProposed?.invoke()
        if (done.kind != "done" && proposals.view.value["draft"] is JsonObject) {
            proposals.dispatch(ev("withdraw"))
        }
        return done
    }

    /**
     * The send machine's `propose_safe_tx` (a multisig's confirm): the
     * reviewed transfers, proposed — answered `proposed` / `propose_failed`
     * (`SendShellResult`'s two answers, as JSON the send executor maps).
     */
    suspend fun proposeFromSend(chainId: Int, multisig: String, transfers: List<Map<String, Any?>>): ProposeOutcome {
        val settled = propose(chainId, mapOf("kind" to "transfers", "transfers" to transfers), address = multisig)
        val hash = settled["safe_tx_hash"].str ?: settled["safeTxHash"].str
        if (settled.kind == "done" && hash != null) return ProposeOutcome.Proposed(hash)
        val failure = MultisigShellFailure.read(settled["detail"].str)
        return when {
            settled == null || settled.kind == "cancelled" || settled.kind == "riskDeclined" -> ProposeOutcome.Failed(cancelled = true, reason = null)
            settled.kind == "mailboxUnreadable" -> ProposeOutcome.Failed(cancelled = false, reason = "mailbox_down")
            settled.kind == "failed" && failure.kind == "passkey" && failure["failure"].str == "cancelled" ->
                ProposeOutcome.Failed(cancelled = true, reason = null)
            else -> ProposeOutcome.Failed(cancelled = false, reason = if (_proposalsView.value.offline) "offline" else null)
        }
    }

    sealed interface ProposeOutcome {
        data class Proposed(val safeTxHash: String) : ProposeOutcome
        data class Failed(val cancelled: Boolean, val reason: String?) : ProposeOutcome
    }

    fun confirmDraft(asMember: String?) = proposals.dispatch(ev("confirm_draft", "as_member" to asMember))

    fun withdrawDraft() = proposals.dispatch(ev("withdraw"))

    /**
     * One background round (M6): every multisig here read again, the one
     * followed refreshed — bounded, as the worker that runs it is. Returns
     * when it began (ms): what it may say is what a read since then holds.
     *
     * r11-1: the round waits for ITS reads — every multisig's list read
     * since it began (`read_at`, the core's), not "the machine went quiet":
     * a chain read takes 8 s and more on a slow network, and the quiet came
     * before the reads, so the round judged a kept list (said nothing) and
     * the read that landed after it was never said. A tick already out when
     * the round began is not the round's: the tick is asked again until one
     * of its own has read every multisig, or the time is up.
     */
    suspend fun backgroundRound(): Long {
        val since = System.currentTimeMillis()
        awaySince = since
        proposals.dispatch(ev("others_tick"))
        proposals.dispatch(ev("refresh", "fresh" to true))
        val deadline = since + ROUND_WAIT_MS
        var asked = since
        while (System.currentTimeMillis() < deadline) {
            delay(ROUND_POLL_MS)
            if (roundRead(_accountsView.value, _proposalsView.value, since)) break
            if (System.currentTimeMillis() - asked >= ROUND_ASK_AGAIN_MS) {
                asked = System.currentTimeMillis()
                proposals.dispatch(ev("others_tick"))
            }
        }
        return since
    }

    /**
     * r11-1: the app was put away with a round in this process — a read that
     * lands later (while still away) is said when it lands, measured from the
     * round's start. `null`: on screen, or no round yet.
     */
    @Volatile var awaySince: Long? = null

    // -- a dApp's request to a multisig (M5) ------------------------------------

    /** The requests held by the machine, by id: who answers the site. */
    private val dappAnswers = java.util.concurrent.ConcurrentHashMap<String, (JsonElement) -> Unit>()

    /** The transaction each answered request became (chain, safeTxHash): its sheet stays on it once the machine lets the request go. */
    private val dappProposed = java.util.concurrent.ConcurrentHashMap<String, Pair<Int, String>>()

    /** What the request [id] became, once proposed (`null`: nothing yet, or never). */
    fun dappProposal(id: String): Pair<Int, String>? = dappProposed[id]

    /**
     * r8-10: the site behind [id] is still waiting for its answer. The
     * machine applies its events in order and LATER than the dispatch, so
     * right after [dappRequest] the view does not hold the request yet — and
     * a sheet that read "not in the view" as "answered" closed itself before
     * the machine saw the request: no sheet, and the site's promise pending
     * for ever. Not answered is this: the answer not yet handed to the site.
     */
    fun dappWaiting(id: String): Boolean = dappAnswers.containsKey(id)

    /**
     * A site's request to a multisig (`eth_sendTransaction`, `wallet_sendCalls`,
     * …): the machine decides everything — refused (a message), drafted with
     * its review, proposed on the person's word, held until executed, told
     * "proposed" before the request's limit — and its answer goes to the site
     * exactly once ([answer]: the machine's `MultisigDappAnswer`, `{result}`
     * or `{error: {code, message, data?}}`, delivered as it is — the browser
     * machine's `multisig_answered`).
     */
    fun dappRequest(
        id: String,
        address: String,
        method: String,
        params: JsonElement,
        origin: String,
        chainId: Int,
        answer: (JsonElement) -> Unit,
    ) {
        val rec = record(address)
        if (rec == null) {
            answer(jo("error" to jo("code" to 4100, "message" to "not a multisig on this device")))
            return
        }
        dappAnswers[id] = answer
        proposals.dispatch(
            ev(
                "dapp_request", "id" to id, "account" to rec, "method" to method, "params" to params,
                "origin" to origin, "chain_id" to maxOf(0, chainId), "limit_ms" to null,
            ),
        )
        sheet.value = MultisigSheet.Dapp(id)
    }

    /** The person left the request (declined, closed, read the refusal): the machine answers the site as it stands. */
    fun dappLeave(id: String) = proposals.dispatch(ev("dapp_leave", "id" to id))

    /** The page behind a held request is gone (already answered): its sheet goes, the machine lets it go. */
    fun dappGone(id: String) {
        dappAnswers.remove(id)
        if (sheet.value == MultisigSheet.Dapp(id)) sheet.value = null
        proposals.dispatch(ev("dapp_leave", "id" to id))
    }

    /**
     * `wallet_getCallsStatus` of a batch a multisig answered a site with
     * (`calls_id`: the proposal's safeTxHash, on [chainId]): the machine's
     * JSON-RPC body (`{result}` or `{error}`, delivered as it is), then
     * forgotten there (`calls_status_taken`). `null`: no word in time.
     */
    suspend fun callsStatus(chainId: Int, callsId: String): JsonElement? {
        val id = "calls-${java.util.UUID.randomUUID()}"
        val body = proposals.request(ev("calls_status", "id" to id, "chain_id" to chainId, "calls_id" to callsId), 15_000) { view ->
            view["calls_status"][id]
        }
        proposals.dispatch(ev("calls_status_taken", "id" to id))
        return body
    }

    /** Each held request the machine answered: to its site, once; its proposal's sheet opened once it exists. */
    private fun settleDapps(view: MsProposalsView) {
        for ((id, answer) in dappAnswers.entries.toList()) {
            val held = view.dapp[id] ?: continue
            val result = held["answer"] ?: continue
            if (dappAnswers.remove(id) == null) continue
            held["safe_tx_hash"].str?.takeIf { it.isNotEmpty() }?.let { dappProposed[id] = (held["chain_id"].int ?: 0) to it }
            // An error the site was not told of takes the sheet with it.
            if (result["error"] != null && held["told"].bool != true && sheet.value == MultisigSheet.Dapp(id)) sheet.value = null
            answer(result)
            proposals.dispatch(ev("dapp_settled", "id" to id))
        }
    }

    // -- risk (§3.5) ------------------------------------------------------------

    private suspend fun askRisk(op: MultisigOperation.AskRisk): Boolean {
        val answer = CompletableDeferred<Boolean>()
        _riskAsk.value?.answer(false)
        _riskAsk.value = MultisigRiskAsk(op.chain_id.toInt(), op.safe, op.findings) { signs ->
            _riskAsk.value = null
            answer.complete(signs)
        }
        return answer.await()
    }

    // -- the router (docs/multisig-machines.md §1.2) ------------------------------

    suspend fun route(notice: JsonObject) {
        when (notice.kind) {
            "account" -> accounts.dispatch(ev("apply", "address" to notice["address"].str.orEmpty(), "update" to (notice["update"] ?: JsonNull)))
            "activity_rows" -> writeActivity(notice["rows"].arr)
            "mailbox_took" -> if (notice["safe"].str.equals(followed, ignoreCase = true)) {
                proposals.dispatch(ev("refresh", "fresh" to true))
            }
            "activated" -> Unit
            "open_proposal" -> {
                val chainId = notice["chain_id"].int ?: 0
                val hash = notice["safe_tx_hash"].str.orEmpty()
                proposals.dispatch(ev("open_linked", "chain_id" to chainId, "safe_tx_hash" to hash, "preview" to (notice["preview"] ?: JsonNull)))
                sheet.value = MultisigSheet.Proposal(chainId, hash)
            }
            "outside_sign" -> openOutside(notice["request"].obj)
            else -> VelaLog.event("multisig", "notice nobody routes", "kind" to (notice.kind ?: "?"))
        }
    }

    /** An outside member's wallet signs it (M4): a session of its own; its answer back to the proposals machine. */
    private fun openOutside(request: JsonObject) {
        val chainId = request["chainId"] ?: request["chain_id"] ?: jsonOf(0)
        val hash = request["safeTxHash"] ?: request["safe_tx_hash"] ?: jsonOf("")
        lateinit var model: MultisigOutsideSessionModel
        val built = openOutsideSession?.invoke(request) { outcome ->
            proposals.dispatch(ev("outside_answered", "chain_id" to chainId, "safe_tx_hash" to hash, "outcome" to outcome))
            if (outsideSession.value === model) outsideSession.value = null
        }
        if (built == null) {
            // No transport: the machine is told nobody answered, never left waiting.
            proposals.dispatch(ev("outside_answered", "chain_id" to chainId, "safe_tx_hash" to hash, "outcome" to jo("kind" to "cancelled")))
            return
        }
        model = built
        outsideSession.value?.close()
        outsideSession.value = built
        built.start()
    }

    // -- activity rows ----------------------------------------------------------

    /**
     * The machine's executed rows, worded as Activity rows (the web's
     * `executionRows`): a token's symbol and decimals, the proposal's title.
     */
    private suspend fun writeActivity(executed: List<JsonElement>) {
        val out = mutableListOf<org.json.JSONObject>()
        for (e in executed) {
            val rows = e["rows"]
            if (rows == null) {
                out += org.json.JSONObject(e.toString())
                continue
            }
            val chainId = e["chainId"].int ?: 0
            val hash = e["safeTxHash"].str.orEmpty()
            for (row in rows.arr) {
                val index = row["index"].int ?: 0
                val base = org.json.JSONObject()
                    .put("id", MsRules.executedRowId(chainId, hash, index))
                    .put("userOpHash", e["userOpHash"].str.orEmpty())
                    .put("txHash", e["txHash"].str.orEmpty())
                    .put("from", e["from"].str.orEmpty())
                    .put("to", row["to"].str.orEmpty())
                    .put("chainId", chainId)
                    .put("timestamp", e["timestamp"].long ?: (System.currentTimeMillis() / 1000))
                    .put("status", "confirmed")
                if (row.kind == "send") {
                    val token = row["token"].str
                    val (symbol, decimals) = tokenInfo(chainId, token)
                    base.put("value", MsRules.plainUnits(row["amount"].str ?: "0", decimals))
                        .put("symbol", symbol).put("decimals", decimals).put("type", "send")
                    if (token != null) base.put("token", token)
                } else {
                    val sends = row["sends"].str ?: "0"
                    val (symbol, decimals) = tokenInfo(chainId, null)
                    val some = sends.isNotEmpty() && sends != "0"
                    base.put("value", if (some) MsRules.plainUnits(sends, decimals) else "0")
                        .put("symbol", if (some) symbol else "").put("decimals", decimals).put("type", "dapp_tx")
                        .put("intent", titleOf(chainId, e["action"], e["nonce"].str ?: "0"))
                        .put(
                            "dappSummary",
                            org.json.JSONObject().put("action", "call").put("calls", row["calls"].int ?: 1).put("contract", row["contract"].str.orEmpty()),
                        )
                }
                out += base
            }
        }
        if (out.isEmpty()) return
        if (writeRows(out)) onRecorded?.invoke()
        // A rename executed: the record's new name is read from the chain now.
        for (e in executed) {
            if (e["action"].kind == "rename") followed?.let { accounts.dispatch(ev("recheck_record", "address" to it)) }
        }
        // Executed while away: said (M6).
        for (e in executed) {
            val chainId = e["chainId"].int ?: 0
            val hash = e["safeTxHash"].str.orEmpty()
            onExecuted?.invoke(titleOf(chainId, e["action"], e["nonce"].str ?: "0"), chainId, hash)
        }
    }

    fun dispose() {
        recordTimer?.cancel()
        proposalTimer?.cancel()
        othersTimer?.cancel()
        accounts.dispose()
        members.dispose()
        proposals.dispose()
    }

    // Last in the class: the views are followed only once every field above
    // exists (a view can land on another thread before construction ends).
    init {
        live.onNotice = { notice -> route(notice) }
        live.onAskRisk = { op -> askRisk(op) }
        scope.launch { accounts.view.collect { accountsMoved(it) } }
        scope.launch { proposals.view.collect { proposalsMoved(it) } }
    }

    private companion object {
        /** A marker: the draft is drawn. */
        const val DRAWN = "__drawn"

        /** r11-1: how long a background round waits for its reads (the worker gives it 60 s). */
        const val ROUND_WAIT_MS = 40_000L

        /** r11-1: how often a round looks whether its reads are in. */
        const val ROUND_POLL_MS = 500L

        /** r11-1: a tick still not the round's after this is asked again. */
        const val ROUND_ASK_AGAIN_MS = 5_000L
    }

    /**
     * r11-1: every multisig here has a list read since [since] (the core's
     * `read_at`), and the followed one is not loading — what a round may say
     * is now its own reads'. Pure.
     */
    object Round {
        fun read(accounts: MsAccountsView, proposals: MsProposalsView, since: Long): Boolean =
            !proposals.loading && accounts.accounts.isNotEmpty() && accounts.accounts.all { row ->
                proposals.presence[row.account.address.lowercase()]?.readAt?.let { it >= since } == true
            }
    }

    private fun roundRead(accounts: MsAccountsView, proposals: MsProposalsView, since: Long) = Round.read(accounts, proposals, since)
}
