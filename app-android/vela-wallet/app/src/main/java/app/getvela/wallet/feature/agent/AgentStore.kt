package app.getvela.wallet.feature.agent

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.AgentCore
import uniffi.vela_core_uniffi.AgentDone
import uniffi.vela_core_uniffi.AgentLink
import uniffi.vela_core_uniffi.agentBridgeUrlProblem
import uniffi.vela_core_uniffi.agentMcpEndpoint

/**
 * The AI agent, as the phone's screens see it (spec 103, spec 104 phase 7).
 *
 * The agent itself is the core's — `vela-agent`, the one copy the desktop and
 * the iPhone run too: the runner (check → examine → approve → perform →
 * observe), the transfers, the ClientMCP tools and the bridge connection, on a
 * thread of its own ([AgentCore]). What is this app's is what it does for it
 * ([AgentShellBridge]): its views, its controllers' calls ([AgentPerform]),
 * its screens ([AgentPage]), its send.
 *
 * One way in for the person and the agent: the person's own taps on the
 * commit points go through the same intents ([person]), so a tap and an
 * agent's request reach the controller that owns the change by one road
 * (spec 103 D3).
 *
 * Off unless the person turns it on, and answering only while the app is in
 * front: in the background the connection is hung up, and a request waiting
 * for the person is withdrawn.
 */
class AgentStore(
    private val store: KeyValueStore,
    /** The main thread's: what the screens collect is set here. */
    private val scope: CoroutineScope,
    /** The client token's place: the Keystore's, never the record's. */
    private val vault: AgentTokenVault,
    /** Spec 106: the agent's address's place — the Keystore's too. */
    private val agentUrlVault: AgentTokenVault = MemoryTokenVault(),
) {
    private val _config = MutableStateFlow(AgentConfig())
    val config: StateFlow<AgentConfig> = _config

    private val _link = MutableStateFlow<AgentLink>(AgentLink.Off)
    val link: StateFlow<AgentLink> = _link

    private val _approvals = MutableStateFlow<List<AgentRequest>>(emptyList())

    /** The agent's requests waiting for the person, oldest first. */
    val approvals: StateFlow<List<AgentRequest>> = _approvals

    private val _activity = MutableStateFlow<List<AgentCall>>(emptyList())

    /** The agent's recent calls, newest first. */
    val activity: StateFlow<List<AgentCall>> = _activity

    /**
     * A screen the agent asked to show, beyond the route the navigation host
     * has already opened: an asset, an activity record, a network's code (the
     * wallet route's), a contact or a group (the contacts route's). Taken once
     * by whichever is composed.
     */
    val detail = MutableStateFlow<JSONObject?>(null)

    /**
     * A wallet flow is on screen (a send, a receive, a record): its own button
     * sits low on the page, and the capsule floats above it.
     */
    val flowOpen = MutableStateFlow(false)

    /** Bumped when the agent cleared a store, so an open storage page reads again. */
    val storageCleared = MutableStateFlow(0L)

    /** What an erase left behind, for the erase sheet to name (`null` = nothing). */
    val eraseLeft = MutableStateFlow<List<String>?>(null)

    /** What only the navigation host can do. Registered by it. */
    @Volatile
    var page: AgentPage? = null

    private val bridge = AgentShellBridge(this)
    private val core = AgentCore(bridge, "phone")

    /**
     * Whether the app is in front: the agent answers only then. Not until an
     * activity has started — a process started for something else (a
     * broadcast, a job) dials nothing.
     */
    @Volatile
    private var active = false

    /** What the core was last configured with. */
    private var dialled: AgentConfig? = null

    /** The app's own controllers, read and changed for the agent. Set once, by the container. */
    @Volatile
    lateinit var wallet: AgentWallet
        internal set

    /** Read the settings, and connect as set up. */
    fun start(wallet: AgentWallet) {
        this.wallet = wallet
        scope.launch {
            load()
            connect()
        }
    }

    /**
     * The settings as stored, the token from its vault. A token an older
     * build kept in the record moves to the vault, and the record is written
     * again without it.
     */
    internal suspend fun load() {
        val stored = AgentConfig.from(runCatching { store.read(KEY) }.getOrNull())
        val withToken = if (stored.token.isEmpty()) {
            stored.copy(token = vault.read().orEmpty())
        } else {
            if (vault.write(stored.token)) store.write(KEY, stored.recordJson())
            stored
        }
        _config.value = if (withToken.linked != null) {
            withToken.copy(agentUrl = agentUrlVault.read().orEmpty())
        } else {
            withToken
        }
    }

    /**
     * Erase This Device: the token goes too — its sealed bytes are swept with
     * the `vela.*` values, and its key leaves the Keystore. The settings went
     * with the sweep, so the agent is off from here and hangs up: this app
     * outlives the erase, and the connection was dialled with that token.
     */
    suspend fun forgetToken() {
        vault.delete()
        agentUrlVault.delete()
        _config.value = AgentConfig()
        connect()
    }

    /** The app came to the front, or left it. */
    fun sceneActive(active: Boolean) {
        if (this.active == active) return
        this.active = active
        scope.launch { connect() }
    }

    private fun connect() {
        val now = _config.value
        val wanted = now.copy(enabled = now.enabled && active)
        if (wanted == dialled) return
        dialled = wanted
        core.configure(wanted.enabled, wanted.url, wanted.token)
        refresh()
    }

    /** Spec 106: connecting with clientmcp.dev. */
    val clientmcp = ClientMcpLink(store, scope, this)

    /**
     * Spec 106: after the person's `configure_agent` with what clientmcp.dev
     * issued, whose it is — kept so Settings can say so.
     */
    suspend fun setLinked(linked: AgentLinkedTo?, agentUrl: String? = null) {
        val url = if (linked == null) "" else agentUrl.orEmpty()
        if (url.isEmpty()) agentUrlVault.delete() else agentUrlVault.write(url)
        val next = _config.value.copy(linked = linked, agentUrl = url)
        _config.value = next
        if (!store.write(KEY, next.recordJson())) VelaLog.event("agent", "settings not saved")
    }

    /** The `configure_agent` intent, performed: what is `null` stays as it is. */
    suspend fun apply(enabled: Boolean?, url: String?, token: String?, wallets: List<String>? = null) {
        var next = _config.value
        if (wallets != null) next = next.copy(wallets = wallets.map { it.trim() })
        if (enabled != null) next = next.copy(enabled = enabled)
        if (url != null) next = next.copy(url = url.trim())
        if (token != null) {
            // A token from anywhere else is no longer clientmcp.dev's, nor is
            // the agent's address that came with it.
            next = next.copy(token = token.trim(), linked = null, agentUrl = "")
            agentUrlVault.delete()
            // The vault's; a token it would not keep is kept for this run only.
            if (next.token.isEmpty()) {
                vault.delete()
            } else if (!vault.write(next.token)) {
                VelaLog.event("agent", "token not kept")
            }
        }
        _config.value = next
        if (!store.write(KEY, next.recordJson())) VelaLog.event("agent", "settings not saved")
        connect()
    }

    /** Read again what the core holds — on its `changed`. */
    fun refresh() {
        _link.value = core.link()
        _approvals.value = requests(core.approvals())
        _activity.value = calls(core.activity())
    }

    /** The person answered the agent's request. */
    fun answer(id: ULong, approved: Boolean) = core.answer(id, approved)

    /** The person stopped the agent: nothing of its still waits, and it is switched off. */
    fun stop() = core.stop()

    /**
     * The person's own tap, through the same intents an agent's request takes
     * (spec 103 D3). [then] hears how it ended, on the main thread.
     */
    fun person(action: JSONObject, then: ((AgentOutcome) -> Unit)? = null) {
        val done = then?.let { heard ->
            object : AgentDone {
                override fun done(outcome: String) {
                    scope.launch { heard(AgentOutcome.from(outcome)) }
                }
            }
        }
        if (!core.invokePerson(action.toString(), done)) {
            VelaLog.event("agent", "not an action", "type" to action.optString("type"))
        }
    }

    /** [person], waited for. */
    suspend fun personAwait(action: JSONObject): AgentOutcome {
        val answer = CompletableDeferred<AgentOutcome>()
        person(action) { answer.complete(it) }
        return answer.await()
    }

    /** The passkey's ceremony moved: `started` (its prompt is up) or `signed`. */
    fun ceremony(event: String) = core.ceremony(event)

    /**
     * The transfer on the open send's review, by its operation id — adopted if
     * the person filled the form themselves, so their confirm goes through
     * `submit_transfer` as an agent's would.
     */
    fun reviewing(): String? = core.currentReview()?.let { runCatching { JSONObject(it).optString("id") }.getOrNull() }
        ?.takeIf { it.isNotEmpty() }

    /** The agent's request on show: the oldest still waiting. */
    val pending: AgentRequest? get() = _approvals.value.firstOrNull()

    internal fun changed() {
        // Never inline: the core may say so from inside a call this thread is making.
        scope.launch { refresh() }
    }

    companion object {
        /** Settings → AI agent, as the web and the desktop store it. Erase sweeps it with the rest. */
        const val KEY = "vela.agent"

        private fun list(json: String): List<JSONObject> = runCatching {
            val array = JSONArray(json)
            (0 until array.length()).map { array.getJSONObject(it) }
        }.getOrDefault(emptyList())

        internal fun requests(json: String): List<AgentRequest> = list(json).mapNotNull { row ->
            val action = row.optJSONObject("action") ?: return@mapNotNull null
            AgentRequest(id = row.optLong("id").toULong(), kind = action.optString("type"), action = action)
        }

        internal fun calls(json: String): List<AgentCall> = list(json).map { row ->
            val outcome = row.optJSONObject("outcome")
            AgentCall(
                id = row.optLong("id").toULong(),
                kind = row.optString("action"),
                stage = row.optString("stage").takeIf { !row.isNull("stage") && it.isNotEmpty() },
                status = outcome?.optString("status"),
                code = outcome?.optJSONObject("reason")?.optString("code"),
            )
        }
    }
}

/** Settings → AI agent: `vela.agent`, `{ enabled, url }` — and the client token, which is the vault's. */
data class AgentConfig(
    val enabled: Boolean = false,
    /** The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`. */
    val url: String = "",
    /** A ClientMCP client token for this wallet. */
    val token: String = "",
    /** Spec 106: whose the token is, when clientmcp.dev issued it. */
    val linked: AgentLinkedTo? = null,
    /** Spec 106: the MCP server address for the person's AI agent — the vault's, never the record's. */
    val agentUrl: String = "",
    /** Spec 107 E8: the wallets the agent may use; none unless the person says so. */
    val wallets: List<String> = emptyList(),
) {
    /** Whether the bridge can be dialled with what is entered. */
    val ready: Boolean get() = enabled && agentBridgeUrlProblem(url) == null && token.isNotBlank()

    /** Why the bridge URL cannot be used — `invalid`, `insecure` — once one is entered. */
    val urlProblem: String? get() = if (url.isEmpty()) null else agentBridgeUrlProblem(url)

    /** The MCP endpoint an agent adds, derived from the bridge URL. */
    val endpoint: String? get() = agentMcpEndpoint(url)

    /** The record as stored: no token — that is the vault's. */
    fun recordJson(): String = JSONObject().put("enabled", enabled).put("url", url)
        .put("wallets", org.json.JSONArray(wallets))
        .apply { linked?.let { put("linked", it.json()) } }
        .toString()

    companion object {
        /** What is stored; anything unreadable reads as off. A `token` in it is an older build's. */
        fun from(text: String?): AgentConfig {
            val json = text?.let { runCatching { JSONObject(it) }.getOrNull() } ?: return AgentConfig()
            return AgentConfig(
                enabled = json.optBoolean("enabled", false),
                url = json.optString("url"),
                token = json.optString("token"),
                linked = json.optJSONObject("linked")?.let(AgentLinkedTo::from),
                wallets = json.optJSONArray("wallets")?.let { list ->
                    (0 until list.length()).mapNotNull { list.optString(it).takeIf { s -> s.isNotEmpty() } }
                }.orEmpty(),
            )
        }
    }
}

/** Who clientmcp.dev issued the token to — stored as the web stores it (`{ site, login, clientId }`). */
data class AgentLinkedTo(val site: String, val login: String, val clientId: String) {
    fun json(): JSONObject = JSONObject().put("site", site).put("login", login).put("clientId", clientId)

    companion object {
        fun from(json: JSONObject): AgentLinkedTo? {
            val site = json.optString("site").takeIf { it.isNotEmpty() } ?: return null
            val login = json.optString("login").takeIf { it.isNotEmpty() } ?: return null
            val clientId = json.optString("clientId").takeIf { it.isNotEmpty() } ?: return null
            return AgentLinkedTo(site, login, clientId)
        }
    }
}

/** One of the agent's requests waiting for the person. */
data class AgentRequest(val id: ULong, val kind: String, val action: JSONObject)

/** One of the agent's recent calls. */
data class AgentCall(
    val id: ULong,
    /** The action's `type`. */
    val kind: String,
    /** `approving`, `performing`, `observing` while it runs. */
    val stage: String?,
    /** `completed`, `accepted`, `rejected` once it ended. */
    val status: String?,
    /** Why it was rejected. */
    val code: String?,
)

/** How a request ended. */
data class AgentOutcome(val status: String, val code: String?, val detail: String?) {
    val done: Boolean get() = status != "rejected"

    companion object {
        fun from(json: String): AgentOutcome {
            val outcome = runCatching { JSONObject(json) }.getOrDefault(JSONObject())
            val reason = outcome.optJSONObject("reason")
            return AgentOutcome(
                status = outcome.optString("status", "rejected"),
                code = reason?.optString("code"),
                detail = reason?.optString("detail"),
            )
        }
    }
}
