package app.getvela.wallet.feature.agent

import app.getvela.wallet.BuildConfig
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import java.security.SecureRandom
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import uniffi.vela_core_uniffi.clientmcpAppRedirect
import uniffi.vela_core_uniffi.clientmcpInstallationId
import uniffi.vela_core_uniffi.clientmcpLinkAccept
import uniffi.vela_core_uniffi.clientmcpLinkBegin
import uniffi.vela_core_uniffi.clientmcpLinkExchange
import uniffi.vela_core_uniffi.clientmcpLinkReturned

/** Why connecting with clientmcp.dev did not work. */
sealed interface LinkProblem {
    /** The site said no, in its words. */
    data class Refused(val reason: String) : LinkProblem
    data object Unavailable : LinkProblem
    data object Expired : LinkProblem
    data object NotOurs : LinkProblem
    data object Network : LinkProblem
    data object Failed : LinkProblem

    companion object {
        /** The core's `{kind, detail}`. */
        fun of(core: JSONObject): LinkProblem = when (core.optString("kind")) {
            "refused" -> Refused(core.optString("detail"))
            "unavailable" -> Unavailable
            "expired" -> Expired
            "not_ours" -> NotOurs
            else -> Failed
        }
    }
}

/**
 * Connecting the AI agent with clientmcp.dev (spec 106): the person signs in
 * there, in a Custom Tab, and the site sends the browser back to
 * `app.getvela.wallet:/clientmcp` — `MainActivity`'s, handed to [returned].
 *
 * The protocol's every decision is the core's (`clientmcpLink*`, the same
 * functions every shell calls). This is the phone's part: the pending sign-in
 * kept in the store (the process may be killed while the browser is in
 * front), the tab, and the one request that trades the code. What comes back
 * is applied as the person's own `configure_agent`.
 */
class ClientMcpLink(
    private val store: KeyValueStore,
    private val scope: CoroutineScope,
    private val agent: AgentStore,
    private val http: OkHttpClient = VelaHttp.client,
) {
    private val _linking = MutableStateFlow(false)
    /** On the way to clientmcp.dev and back. */
    val linking: StateFlow<Boolean> = _linking

    private val _problem = MutableStateFlow<LinkProblem?>(null)
    /** Why the last attempt did not connect. */
    val problem: StateFlow<LinkProblem?> = _problem

    /** Opens a page in a Custom Tab: `MainActivity`'s, set while it lives. */
    var openPage: ((String) -> Boolean)? = null

    /** Off to clientmcp.dev. */
    fun start() {
        if (_linking.value) return
        _problem.value = null
        _linking.value = true
        scope.launch {
            val begun = answer(
                clientmcpLinkBegin(
                    SITE,
                    clientmcpAppRedirect(),
                    LABEL,
                    installation(),
                    random(64),
                    System.currentTimeMillis().toULong(),
                ),
            )
            val pending = begun.getOrNull() as? JSONObject
            val opened = pending != null &&
                store.write(PENDING, pending.toString()) &&
                openPage?.invoke(pending.optString("authorize_url")) == true
            if (!opened) {
                store.remove(PENDING)
                _linking.value = false
                _problem.value = (begun.exceptionOrNull() as? LinkFailure)?.problem ?: LinkProblem.Failed
            }
        }
    }

    /** The person gave up waiting (the tab closed without coming back). */
    fun cancel() {
        _linking.value = false
        scope.launch { store.remove(PENDING) }
    }

    /** The browser came back to the return address with [url]. */
    fun returned(url: String) {
        _linking.value = true
        _problem.value = null
        scope.launch {
            val pending = store.read(PENDING)
            store.remove(PENDING)
            val outcome = if (pending == null) {
                Result.failure(LinkFailure(LinkProblem.NotOurs))
            } else {
                finish(pending, url)
            }
            outcome.onFailure { _problem.value = (it as? LinkFailure)?.problem ?: LinkProblem.Failed }
            _linking.value = false
        }
    }

    private suspend fun finish(pending: String, url: String): Result<Unit> = runCatching {
        val now = System.currentTimeMillis().toULong()
        val code = answer(clientmcpLinkReturned(pending, url, now)).getOrThrow() as String
        val request = answer(clientmcpLinkExchange(pending, code)).getOrThrow() as JSONObject
        val (status, body) = withContext(Dispatchers.IO) {
            try {
                http.newCall(
                    Request.Builder()
                        .url(request.getString("url"))
                        .post(request.getString("body").toRequestBody("application/json".toMediaType()))
                        .build(),
                ).execute().use { it.code to (it.body?.string() ?: "") }
            } catch (_: java.io.IOException) {
                throw LinkFailure(LinkProblem.Network)
            }
        }
        val linked = answer(clientmcpLinkAccept(status.toUShort(), body)).getOrThrow() as JSONObject
        val change = JSONObject()
            .put("type", "configure_agent")
            .put("enabled", true)
            .put("bridge_url", linked.getString("bridge_url"))
            .put("client_token", linked.getString("client_token"))
        // E8: the first connection offers the wallet in front; a list the
        // person already chose is kept.
        val active = agent.wallet.session.view.value.address
        if (agent.config.value.wallets.isEmpty() && active.isNotEmpty()) {
            change.put("wallets", org.json.JSONArray(listOf(active)))
        }
        val outcome = agent.personAwait(change)
        if (!outcome.done) throw LinkFailure(LinkProblem.Failed)
        agent.setLinked(
            AgentLinkedTo(
                site = JSONObject(pending).optString("site"),
                login = linked.getString("login"),
                clientId = linked.getString("client_id"),
            ),
            agentUrl = linked.optString("agent_url").takeIf { it.isNotEmpty() },
        )
        VelaLog.event("agent", "connected with clientmcp.dev")
    }

    /** This installation's ID: made once, kept (not a secret). */
    private suspend fun installation(): String {
        store.read(INSTALLATION)?.takeIf { it.isNotEmpty() }?.let { return it }
        val made = clientmcpInstallationId(random(16))
        store.write(INSTALLATION, made)
        return made
    }

    private class LinkFailure(val problem: LinkProblem) : Exception(problem.toString())

    companion object {
        /** clientmcp.dev, unless a development build names another. */
        val SITE: String = BuildConfig.CLIENTMCP_SITE
        /** What clientmcp.dev lists this installation as (the person can rename it there). */
        const val LABEL = "Android phone"
        /** The scheme of the return address. */
        const val SCHEME = "app.getvela.wallet"
        private const val PENDING = "vela.agent.link"
        private const val INSTALLATION = "vela.agent.installation"

        private fun random(count: Int): ByteArray = ByteArray(count).also { SecureRandom().nextBytes(it) }

        /** The core's `{"ok": …}`, or its `{"error": …}` as a failure. */
        private fun answer(json: String): Result<Any> {
            val obj = runCatching { JSONObject(json) }.getOrNull()
                ?: return Result.failure(LinkFailure(LinkProblem.Failed))
            obj.optJSONObject("error")?.let { return Result.failure(LinkFailure(LinkProblem.of(it))) }
            return obj.opt("ok")?.let { Result.success(it) } ?: Result.failure(LinkFailure(LinkProblem.Failed))
        }
    }
}
