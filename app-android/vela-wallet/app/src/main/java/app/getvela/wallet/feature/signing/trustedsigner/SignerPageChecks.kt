package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.i18n.VelaStrings
import java.util.TimeZone
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.SignerCheckFailure
import uniffi.vela_core_uniffi.SignerIntegrityLine
import uniffi.vela_core_uniffi.SignerIntegrityState
import uniffi.vela_core_uniffi.SignerPageAdmission
import uniffi.vela_core_uniffi.SignerPageTarget
import uniffi.vela_core_uniffi.signerIntegrityLineWhileChecking
import uniffi.vela_core_uniffi.signerIntegrityTime
import uniffi.vela_core_uniffi.signerPageAdmit
import uniffi.vela_core_uniffi.signerPageCheckHeaders
import uniffi.vela_core_uniffi.signerPageHash
import uniffi.vela_core_uniffi.signerPageKeepOrReplace
import uniffi.vela_core_uniffi.signerPageRefreshDue
import uniffi.vela_core_uniffi.signingPageTrusted

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
 * So a page in use is checked again in the background well before then
 * ([refresh], spec 102 D-14: on start, on every return to the foreground and
 * hourly — the core says which pages are due), and a refresh that could not
 * complete keeps a check that still vouches (`signerPageKeepOrReplace`):
 * Open almost never waits, and a good line never flickers to "checking".
 *
 * **Trust is per page** (D-15): the versions a person trusted are the saved
 * page's own (`signingPageTrusted`), recorded by `SigningPagesCore`'s
 * `version_trusted` — never a device-wide list.
 */
class SignerPageChecks(
    /** GET [url] with the core's check headers, no redirects followed; see [Fetched]. */
    private val fetch: suspend (url: String) -> Fetched,
    /** The per-device blocked version list (`vela.signerPage.blocked`). */
    private val store: KeyValueStore?,
    private val clock: () -> Long = System::currentTimeMillis,
    /**
     * The saved pages as `SigningPagesCore` keeps them (its `saved`, JSON) —
     * what `signingPageTrusted` reads a page's trusted versions from. By
     * default the stored list itself.
     */
    private val savedPages: suspend () -> String? = { runCatching { store?.read(KeyValueStore.Keys.SIGNING_PAGES) }.getOrNull() },
    /**
     * Record "Trust this version" for a page (`SigningPagesEvent::VersionTrusted`)
     * and return once [savedPages] carries it. The pages core owns the list;
     * this only asks it.
     */
    private val recordTrust: suspend (url: String, version: String) -> Unit = { _, _ -> },
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
        /**
         * The check that rules for the page — present once one ruled, and kept
         * through a refresh that could not complete while it still vouches.
         * While [running], the previous one (or `null`).
         */
        val admission: SignerPageAdmission?,
        /** The version the check asked for (sha256 hex), empty when there was none. */
        val version: String,
        /** A custom page's own build, proposed by its index: the person decides. */
        val proposed: Boolean,
        /** A check of the page is in flight; [line] is the core's line while it runs. */
        val running: Boolean = false,
    )

    private val _checks = MutableStateFlow<Map<String, Check>>(emptyMap())

    /** Every page checked (or being checked) this run, by its base address. */
    val checks: StateFlow<Map<String, Check>> = _checks

    private val locks = ConcurrentHashMap<String, Mutex>()

    /** When a check of each page last STARTED, completed or not — the core spaces retries by it. */
    private val attempts = ConcurrentHashMap<String, Long>()

    /**
     * The line to draw for [base] now: while a check runs, the core's line for
     * that moment (`signerIntegrityLineWhileChecking` — the previous verdict
     * while it still vouches, else "checking"); else the check's, read at this
     * moment ("checking" once it is too old); "checking" while none has ruled.
     */
    fun line(base: String): SignerIntegrityLine {
        val check = _checks.value[key(base)] ?: return CHECKING
        val now = clock().toULong()
        if (check.running) return signerIntegrityLineWhileChecking(check.admission, now)
        return check.admission?.line(now) ?: check.line
    }

    /** The admission for [base] when it opens the page NOW (`isFresh`), else `null`. */
    fun admitted(base: String): SignerPageAdmission? =
        _checks.value[key(base)]?.admission?.takeIf { it.isFresh(clock().toULong()) }

    /**
     * The full version the person is asked to trust for [base] — `Some` only
     * while the page's check asks ([SignerIntegrityState.ASK_TO_TRUST]), which
     * only a self-hosted page can (`SignerPageAdmission.versionToTrust`).
     */
    fun versionToTrust(base: String): String? =
        _checks.value[key(base)]?.takeIf { !it.running }?.admission?.versionToTrust()

    /**
     * An admission for [base] that opens the page now — the one held when it is
     * still fresh, else a new check. `null` when the page may not be opened; the
     * line ([line]) says why.
     */
    suspend fun ensure(base: String): SignerPageAdmission? {
        admitted(base)?.let { return it }
        return check(base).admission?.takeIf { it.isFresh(clock().toULong()) }
    }

    /**
     * Is a background check of [base] due now? The core's rule
     * (`signerPageRefreshDue`): no check that admitted it, or one older than
     * half a day — and no attempt in the last few minutes.
     */
    fun refreshDue(base: String): Boolean {
        val key = key(base)
        val checkedAt = _checks.value[key]?.admission?.checkedAtMs()
        return signerPageRefreshDue(checkedAt, attempts[key]?.toULong(), clock().toULong())
    }

    /**
     * Spec 102 D-14: re-check every page in [pages] the core says is due —
     * each account's page venue and every saved page — side by side. Called on
     * start, on every return to the foreground, and hourly while the app runs
     * (`signerPageRefreshSchedule().pollMs`); a page not due costs nothing.
     */
    suspend fun refresh(pages: Collection<String>) = coroutineScope {
        pages.map(::key).distinct().filter(::refreshDue).forEach { page ->
            launch { runCatching { check(page) } }
        }
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
            _checks.value[key]?.takeIf { it !== before && !it.running && it.admission?.isFresh(clock().toULong()) == true }
                ?.let { return@withLock it }
            val held = _checks.value[key]
            val previous = held?.admission
            attempts[key] = clock()
            publish(
                key,
                Check(
                    key,
                    signerIntegrityLineWhileChecking(previous, clock().toULong()),
                    previous,
                    held?.version.orEmpty(),
                    held?.proposed ?: false,
                    running = true,
                ),
            )
            val trusted = trustedOf(key)
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
                val now = clock().toULong()
                val next = signerPageAdmit(target, hash, failure, trusted, blocked, false, now)
                // A refresh that could not complete keeps a check that still
                // vouches; one that completed replaces it, a mismatch included.
                val kept = signerPageKeepOrReplace(previous, next, now)
                if (held != null && previous != null && next.checkedAtMs() == null && kept.checkedAtMs() != null &&
                    kept.checkedAtMs() == previous.checkedAtMs()
                ) {
                    Check(key, kept.line(now), kept, held.version, held.proposed)
                } else {
                    Check(key, kept.line(now), kept, target.version().orEmpty(), target.proposedByIndex())
                }
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
     * "Trust this version" (spec 076 FR-009, spec 102 D-15): a self-hosted
     * page's own build, which its index proposed and the check asked about.
     * The version is the admission's own (`versionToTrust`), stored on THAT
     * page by the pages core — so it vouches for that deployment and nothing
     * else — and then the page is checked again, which now admits it. Nothing
     * is recorded when the page's check is not asking.
     */
    suspend fun trust(base: String): Check {
        val key = key(base)
        versionToTrust(key)?.let { version -> recordTrust(key, version) }
        return check(key)
    }

    /** The versions trusted for [base] on this device: the saved page's own list (`signingPageTrusted`). */
    private suspend fun trustedOf(base: String): List<String> {
        val saved = runCatching { savedPages() }.getOrNull() ?: return emptyList()
        return runCatching { signingPageTrusted(saved, base) }.getOrDefault(emptyList())
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

    /**
     * A page was removed from the list: what was checked about it goes with
     * it. Its trusted versions were stored on it and are gone, so a ruling
     * held from before — "trusted on this device" — would be a claim nothing
     * backs; added again, the page is checked afresh and asks again.
     */
    fun forget(base: String) {
        val key = key(base)
        _checks.update { it - key }
        attempts.remove(key)
    }

    private fun publish(key: String, check: Check) {
        _checks.update { it + (key to check) }
    }

    companion object {
        /** Where a deployment of `dist/` lists the versions it still publishes. */
        const val INDEX_PATH = "index.json"

        /** The page is one file of ~300 KB; this is room to grow, and a refusal to hash something absurd. */
        const val MAX_BYTES = 4 * 1024 * 1024

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
         * and `{{time}}`: the core's `signerIntegrityTime` (spec 102 D-13) —
         * the clock time in the person's format when the check ran today,
         * else the date and the time — over this device's offset now, the
         * person's date and time presets (`auto` resolved) and the app's
         * language.
         */
        fun words(
            line: SignerIntegrityLine,
            strings: VelaStrings,
            now: Long = System.currentTimeMillis(),
            formats: Formats = Formats.current,
            zone: TimeZone = TimeZone.getDefault(),
        ): String {
            val time = line.checkedAtMs?.let { at ->
                signerIntegrityTime(
                    checkedAtMs = at,
                    nowMs = now.toULong(),
                    utcOffsetMinutes = zone.getOffset(now) / 60_000,
                    dateFormat = formats.resolvedDate().wire,
                    timeFormat = formats.resolvedTime().wire,
                    language = strings.language,
                )
            }.orEmpty()
            return strings.t(line.key, mapOf("version" to line.version, "time" to time))
        }

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
            // What a browser sends when it navigates to the page — the core's
            // headers (`signerPageCheckHeaders`), so the host answers the check
            // with the bytes it answers the Custom Tab with. Measured on
            // 2026-10-09: `sign.getvela.app` injected a Cloudflare beacon into
            // `text/html` answers only, and a check that did not ask as a
            // browser asks passed while the tab loaded something else.
            val request = okhttp3.Request.Builder().url(url).get().apply {
                signerPageCheckHeaders().forEach { header(it.name, it.value) }
            }.build()
            val response = runCatching {
                client.newCall(request).execute()
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
