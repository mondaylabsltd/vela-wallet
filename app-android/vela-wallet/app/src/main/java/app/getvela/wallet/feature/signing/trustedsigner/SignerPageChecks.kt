package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.i18n.VelaStrings
import java.util.Calendar
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.SignerCheckFailure
import uniffi.vela_core_uniffi.SignerIntegrityLine
import uniffi.vela_core_uniffi.SignerIntegrityState
import uniffi.vela_core_uniffi.SignerPageAdmission
import uniffi.vela_core_uniffi.SignerPageTarget
import uniffi.vela_core_uniffi.signerPageAdmit
import uniffi.vela_core_uniffi.signerPageHash

/**
 * Is the signing page the page it is supposed to be? — the phone's half of
 * spec 102 R6 (and spec 076's T034, which the phones never did).
 *
 * The core decides; this fetches. `SignerPageTarget.choose` picks the version
 * of a page and, with it, the ONE URL that is both fetched to be checked and
 * opened; `signerPageAdmit` rules on the bytes that URL served; and only the
 * admission it returns can build a launch URL (`urlLaunch`). So a page whose
 * check failed — or never ran — is not opened, and the version opened is the
 * version checked. What is platform work, and therefore here, is three things:
 * ask the deployment what it publishes (`index.json`), fetch the target's URL,
 * and hash the bytes that came back.
 *
 * **A plain HTTPS request, on purpose** (owner, 2026-09-23): not a hidden
 * WebView, which would RUN the bytes it is checking. No redirects are
 * followed (the shared client's rule): the official host serves
 * `/b/<sha>/sign` itself.
 *
 * **What it catches, and what it cannot.** A replaced build served to
 * everyone — a hijacked bucket, a bad CDN config. Not a server that serves the
 * browser one thing and this check another; the words a person reads say
 * "matches Vela's published build list · checked …", never "certified".
 *
 * A check vouches for 24 hours (the core's `MAX_CHECK_AGE_MS`): after that the
 * line reads "checking" and nothing opens until the page is checked again.
 */
class SignerPageChecks(
    /** GET [url], no redirects followed; see [Fetched]. */
    private val fetch: suspend (url: String) -> Fetched,
    /** The per-device trusted / blocked version lists. */
    private val store: KeyValueStore?,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    /** What one GET came back as. */
    sealed interface Fetched {
        /** The server answered: its status and the whole body. */
        class Body(val status: Int, val bytes: ByteArray) : Fetched

        /** Nothing answered — no connection, a timeout, a name that does not resolve. */
        data object Unreachable : Fetched

        /** It answered and the body could not be read (or was absurdly large). */
        data object Unreadable : Fetched
    }

    /** One page's state: its line, and the admission when one was made. */
    class Check internal constructor(
        val base: String,
        /** The line as of the check (a fresh read of [admission] is [SignerPageChecks.line]). */
        val line: SignerIntegrityLine,
        /** Present once a check ruled; opens only when [SignerPageAdmission.opens]. */
        val admission: SignerPageAdmission?,
        /** The version the check asked for (sha256 hex), empty when there was none. */
        val version: String,
        /** A custom page's own build, proposed by its index: the person decides. */
        val proposed: Boolean,
    )

    private val _checks = MutableStateFlow<Map<String, Check>>(emptyMap())

    /** Every page checked (or being checked) this run, by its base address. */
    val checks: StateFlow<Map<String, Check>> = _checks

    private val locks = ConcurrentHashMap<String, Mutex>()

    /**
     * The line to draw for [base] now: the check's, read at this moment ("checking"
     * once it is too old), or "checking" while none has ruled.
     */
    fun line(base: String): SignerIntegrityLine {
        val check = _checks.value[key(base)] ?: return CHECKING
        return check.admission?.line(clock().toULong()) ?: check.line
    }

    /** The admission for [base] when it opens the page NOW, else `null`. */
    fun admitted(base: String): SignerPageAdmission? =
        _checks.value[key(base)]?.admission?.takeIf { it.line(clock().toULong()).opens }

    /**
     * An admission for [base] that opens the page now — the one held when it is
     * still fresh, else a new check. `null` when the page may not be opened; the
     * line ([line]) says why.
     */
    suspend fun ensure(base: String): SignerPageAdmission? {
        admitted(base)?.let { return it }
        return check(base).admission?.takeIf { it.line(clock().toULong()).opens }
    }

    /**
     * Check [base] now, whatever is held: R6 through this phone's own I/O.
     * Coalesced per page — a second caller waits for the check in flight
     * rather than fetching the same bytes twice.
     */
    suspend fun check(base: String): Check {
        val key = key(base)
        val lock = locks.getOrPut(key) { Mutex() }
        val before = _checks.value[key]
        return lock.withLock {
            // Someone else's check landed while this one waited: theirs is as new.
            _checks.value[key]?.takeIf { it !== before && it.admission?.line(clock().toULong())?.opens == true }
                ?.let { return@withLock it }
            publish(key, Check(key, CHECKING, null, before?.version.orEmpty(), proposed = false))
            val trusted = hashes(TRUSTED)
            val blocked = hashes(BLOCKED)
            val target = SignerPageTarget.choose(key, published(key), trusted, blocked)
            val url = target.url()
            val check = if (url == null) {
                Check(key, target.line(), null, "", proposed = false)
            } else {
                val fetched = fetchSafely(url)
                if (fetched is Fetched.Body && fetched.status != 200) {
                    VelaLog.event("signer page", "not served", "host" to key.substringAfter("://").substringBefore('/'), "status" to fetched.status)
                }
                val (hash, failure) = when (fetched) {
                    is Fetched.Body ->
                        if (fetched.status == 200) signerPageHash(fetched.bytes) to SignerCheckFailure.NOT_CHECKED
                        else null to SignerCheckFailure.UNREACHABLE
                    Fetched.Unreachable -> null to SignerCheckFailure.UNREACHABLE
                    Fetched.Unreadable -> null to SignerCheckFailure.NO_BYTES
                }
                val now = clock()
                val admission = signerPageAdmit(target, hash, failure, trusted, blocked, false, now.toULong())
                Check(key, admission.line(now.toULong()), admission, target.version().orEmpty(), target.proposedByIndex())
            }
            VelaLog.event(
                "signer page",
                "checked",
                "host" to key.substringAfter("://").substringBefore('/'),
                "state" to check.line.state.name,
                "version" to check.line.version,
            )
            publish(key, check)
            check
        }
    }

    /**
     * "Trust this version on this device" (FR-009): a custom page's own build,
     * which its index proposed and nobody decided about yet. Per device, never
     * synced. Then the page is checked again — which now admits it.
     */
    suspend fun trust(base: String, version: String): Check {
        val store = store ?: return check(base)
        if (version.isNotBlank()) {
            val now = hashes(TRUSTED)
            if (now.none { it.equals(version, ignoreCase = true) }) {
                store.write(TRUSTED, JSONArray(now + version.lowercase()).toString())
            }
        }
        return check(base)
    }

    /** What the deployment says it publishes, or `null` when it could not be read at all. */
    private suspend fun published(base: String): List<String>? =
        when (val fetched = fetchSafely(base + INDEX_PATH)) {
            // An answer, even a refusal, is an answer: no index (which narrows
            // nothing) is not the same as a network that reached nobody.
            is Fetched.Body -> if (fetched.status == 200) listed(fetched.bytes) else emptyList()
            Fetched.Unreachable, Fetched.Unreadable -> {
                VelaLog.event("signer page", "could not fetch the index", "host" to base.substringAfter("://").substringBefore('/'))
                null
            }
        }

    private suspend fun fetchSafely(url: String): Fetched =
        runCatching { fetch(url) }.getOrElse { error ->
            if (error is kotlinx.coroutines.CancellationException) throw error
            Fetched.Unreachable
        }

    private suspend fun hashes(storeKey: String): List<String> {
        val raw = runCatching { store?.read(storeKey) }.getOrNull() ?: return emptyList()
        return runCatching {
            val rows = JSONArray(raw)
            (0 until rows.length()).mapNotNull { rows.optString(it).trim().lowercase().takeIf(String::isNotEmpty) }
        }.getOrDefault(emptyList())
    }

    private fun publish(key: String, check: Check) {
        _checks.update { it + (key to check) }
    }

    companion object {
        /** Where a deployment of `dist/` lists the versions it still publishes. */
        const val INDEX_PATH = "index.json"

        /** The page is one file of ~300 KB; this is room to grow, and a refusal to hash something absurd. */
        const val MAX_BYTES = 4 * 1024 * 1024

        private const val TRUSTED = KeyValueStore.Keys.SIGNER_PAGE_TRUSTED
        private const val BLOCKED = KeyValueStore.Keys.SIGNER_PAGE_BLOCKED

        /** The line while a check runs: never opens. */
        val CHECKING = SignerIntegrityLine(
            state = SignerIntegrityState.CHECKING,
            version = "",
            checkedAtMs = null,
            key = "componentsUi.signing.integrity.checking",
            opens = false,
        )

        /** A page's base as the checks are keyed: trimmed, with its trailing slash. */
        fun key(base: String): String = base.trim().let { if (it.endsWith('/')) it else "$it/" }

        /** The index's hashes: `{"versions":[…]}` or a bare array; anything else lists nothing. */
        fun listed(bytes: ByteArray): List<String> {
            val text = runCatching { String(bytes, Charsets.UTF_8) }.getOrNull() ?: return emptyList()
            val array = runCatching { JSONObject(text).optJSONArray("versions") }.getOrNull()
                ?: runCatching { JSONArray(text) }.getOrNull()
                ?: return emptyList()
            return (0 until array.length()).mapNotNull { array.optString(it).trim().takeIf(String::isNotEmpty) }
        }

        /**
         * The integrity line in words — `{{version}}` (eight hex characters)
         * and `{{time}}` (when the bytes arrived: the time today, else the date
         * and time; a check is at most a day old).
         */
        fun words(line: SignerIntegrityLine, strings: VelaStrings, now: Long = System.currentTimeMillis()): String {
            val at = line.checkedAtMs?.toLong()
            val time = when {
                at == null -> ""
                sameDay(at, now) -> Formats.current.time(at)
                else -> Formats.current.dateTime(at)
            }
            return strings.t(line.key, mapOf("version" to line.version, "time" to time))
        }

        private fun sameDay(a: Long, b: Long): Boolean {
            val x = Calendar.getInstance().apply { timeInMillis = a }
            val y = Calendar.getInstance().apply { timeInMillis = b }
            return x.get(Calendar.YEAR) == y.get(Calendar.YEAR) && x.get(Calendar.DAY_OF_YEAR) == y.get(Calendar.DAY_OF_YEAR)
        }

        /**
         * What a browser says it accepts when it navigates to a page — and so
         * what the page is fetched with here. The check is only worth anything
         * if it sees the bytes the browser will be served, and a host can
         * serve them differently by this header: measured on 2026-10-09,
         * `sign.getvela.app` answers `Accept: text/html` with its published
         * build PLUS a Cloudflare Web Analytics `<script>` injected before
         * `</body>` (sha256 `b014f9b3…`, not `0ba8ee8c…`), and a request that
         * accepts anything with the published bytes. A check that did not ask
         * as a browser asks would pass while the Custom Tab loaded something
         * else.
         */
        const val BROWSER_ACCEPT = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"

        /** The real GET, over the app's one HTTP client (no redirects followed). */
        suspend fun httpGet(url: String): Fetched = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            val client = app.getvela.wallet.core.net.VelaHttp.client.newBuilder()
                .callTimeout(10, java.util.concurrent.TimeUnit.SECONDS)
                // A GET of a static page is idempotent: a pooled connection the
                // server (or a proxy) already closed is retried on a fresh one
                // rather than read as "couldn't check" (measured: the index and
                // the page back to back over one keep-alive connection).
                .retryOnConnectionFailure(true)
                .build()
            val accept = if (url.endsWith(INDEX_PATH)) "application/json" else BROWSER_ACCEPT
            val response = runCatching {
                client.newCall(okhttp3.Request.Builder().url(url).header("Accept", accept).get().build()).execute()
            }.getOrElse { error ->
                VelaLog.event("signer page", "fetch failed", "host" to url.substringAfter("://").substringBefore('/'), "why" to error.javaClass.simpleName, "detail" to error.message)
                return@withContext Fetched.Unreachable
            }
            response.use { answered ->
                val body = answered.body ?: return@withContext Fetched.Body(answered.code, ByteArray(0))
                val length = body.contentLength()
                if (length > MAX_BYTES) return@withContext Fetched.Unreadable
                val bytes = runCatching {
                    body.byteStream().use { stream ->
                        val out = java.io.ByteArrayOutputStream()
                        val buffer = ByteArray(16 * 1024)
                        while (true) {
                            val read = stream.read(buffer)
                            if (read < 0) break
                            out.write(buffer, 0, read)
                            if (out.size() > MAX_BYTES) return@use null
                        }
                        out.toByteArray()
                    }
                }.getOrNull() ?: return@withContext Fetched.Unreadable
                Fetched.Body(answered.code, bytes)
            }
        }
    }
}
