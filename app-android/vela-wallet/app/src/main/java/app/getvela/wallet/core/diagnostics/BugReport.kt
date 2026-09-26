package app.getvela.wallet.core.diagnostics

import app.getvela.wallet.core.net.VelaHttp
import java.io.IOException
import java.io.InterruptedIOException
import java.util.Base64
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject

/**
 * The in-app report, actually sent (spec 081 FR-016; Android's since 078
 * round 3) — the web's `services/bug-report.ts` and the desktop's
 * `executor/bug_report.rs`, field for field.
 *
 * Two roads. The first is `getvela.app/api/bug-report`: a server-side token
 * files the issue, so somebody without a GitHub account can still report a
 * bug. It answers 503 `not_configured` when the token is not provisioned, 429
 * after five reports in ten minutes, 413 over 16 000 characters. None of those
 * are the person's fault and none may end with their report on the floor, so
 * every refusal, fault and timeout falls back to the second road: the
 * prefilled GitHub issue form ([BugReportUrl.prefilled]).
 *
 * Screenshots ride in the same JSON body (spec 078 round 3, the founder's
 * ask): `screenshots`, at most [MAX_SCREENSHOTS] plain base64 JPEGs, each
 * already downscaled and re-encoded by [ScreenshotPrep] — which is what drops
 * EXIF and location. They are PUBLIC on the issue (the founder's ruling), and
 * the sheet says so before 发送. Not multipart on purpose: the site's CSRF
 * check refuses a cross-site multipart POST.
 *
 * What may never be in a report — addresses, balances, endpoint or RPC URLs
 * (a self-hosted endpoint carries its API key in its path), raw `vela.*`
 * values — is kept out by construction: the payload is ASSEMBLED from the
 * five fields of [DeviceFacts], which cannot reach any of them. [redact] is
 * the second line, for the one field a person can choose (a custom network's
 * display name reaches `unreachable`, and somebody can name a network after
 * its own RPC URL).
 */
object BugReport {

    /** The site's proxy. The token lives there; nothing here has one. */
    const val ENDPOINT: String = "https://getvela.app/api/bug-report"

    /** The endpoint's own cap, honoured before the request leaves. */
    const val MAX_REPORT_CHARS: Int = 16_000

    /** The web's `bundlerRest` budget: a report is small and the person waits. */
    const val TIMEOUT_MS: Long = 10_000

    /** With screenshots the body is megabytes, not bytes: the upload gets its own budget. */
    const val TIMEOUT_WITH_SCREENSHOTS_MS: Long = 30_000

    /** The endpoint's own caps (`MAX_SCREENSHOTS`, `MAX_SCREENSHOT_BYTES`). */
    const val MAX_SCREENSHOTS: Int = 5
    const val MAX_SCREENSHOT_BYTES: Int = 2_000_000

    /** The issue form's `area` option a settings-page report answers. */
    const val AREA_OTHER: String = BugReportUrl.AREA_OTHER

    /**
     * A stand-in endpoint for a verification pass (DEBUG builds only, set from
     * the launch intent) — so checking the flow on a device never files a real
     * issue. The desktop's `VELA_BUG_REPORT_ENDPOINT`.
     */
    @Volatile
    var endpointOverride: String? = null

    /**
     * DEBUG builds only, from the launch intent: pick with the document picker
     * — exactly the road `PickVisualMedia` takes on a phone without the system
     * photo picker (no GMS) — so that road, and its missing maximum, can be
     * walked on any device.
     */
    @Volatile
    var forceDocumentPicker: Boolean = false

    /** DEBUG builds only: hold each tile's "preparing" placeholder this long, so it can be looked at. */
    @Volatile
    var debugPrepareDelayMs: Long = 0

    /** Everything about the device a report is allowed to know — by name. */
    data class DeviceFacts(
        val version: String,
        val commit: String,
        val platform: String,
        val language: String,
        /** Display NAMES of networks whose RPC is unreachable, never their URLs. */
        val unreachable: List<String>,
        /** Failure counters' classes, never values. */
        val failures: List<String>,
    )

    /** The corpus labels the preview lines wear. */
    data class EnvironmentLabels(
        val version: String,
        val platform: String,
        val language: String,
        val rpc: String,
        val failures: String,
        val none: String,
    )

    /**
     * The five text fields the endpoint accepts, in the web's order, and the
     * screenshots — nothing else is sent.
     */
    @Serializable
    data class Payload(
        val what: String,
        val steps: String,
        val area: String,
        /** The preview lines, joined — identical to what the sheet showed. */
        val environment: String,
        val fingerprint: String,
        /**
         * Plain base64 (standard alphabet, padded, no line breaks, no `data:`
         * prefix) of each processed JPEG, in tile order. Absent — not `[]` —
         * when none is attached, so a text-only body is byte-for-byte the old one.
         */
        val screenshots: List<String>? = null,
    )

    /** How a send ended. */
    sealed class Outcome {
        /** Filed, as a new issue or a +1 on an open one. */
        data class Filed(val number: Long, val url: String, val deduped: Boolean, val screenshotsDropped: Int = 0) : Outcome()

        /** The endpoint could not file it; [fallbackUrl] is the road that still works. */
        data class Fallback(val reason: Reason, val fallbackUrl: String) : Outcome()
    }

    /** For the log and the tests; never shown raw to a person. */
    enum class Reason { NotConfigured, RateLimited, TooLarge, Rejected, Unreachable }

    /** What the transport observed. */
    sealed class Answer {
        data class Status(val code: Int, val body: String) : Answer()

        /** Never reached a server, or ran out of time. */
        data class Failed(val timedOut: Boolean) : Answer()
    }

    private val ADDRESS = Regex("0x[0-9a-fA-F]{40}(?![A-Za-z0-9_])")

    /** JavaScript's `\b` and `\S`, spelled out: ASCII word characters, Unicode whitespace. */
    private val URL = Regex("(?<![A-Za-z0-9_])[a-zA-Z][a-zA-Z0-9+.-]*://[^\\s\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000\\ufeff]+")

    /**
     * The second line of defence, on every generated line. Replaced rather
     * than dropped, so a reader of the issue sees something was removed.
     */
    fun redact(line: String): String = line.replace(ADDRESS, "[address]").replace(URL, "[url]")

    /**
     * The preview, and the payload's `environment`, as one list — one
     * function, so the consent sentence ("only what you see is sent") stays
     * literal.
     */
    fun environmentLines(labels: EnvironmentLabels, facts: DeviceFacts): List<String> {
        fun list(items: List<String>, separator: String) = if (items.isEmpty()) labels.none else items.joinToString(separator)
        return listOf(
            "${labels.version}: v${facts.version} (${facts.commit})",
            "${labels.platform}: ${facts.platform}",
            "${labels.language}: ${facts.language}",
            "${labels.rpc}: ${list(facts.unreachable, ", ")}",
            "${labels.failures}: ${list(facts.failures, "; ")}",
        ).map(::redact)
    }

    /**
     * A stable marker for "the same complaint": FNV-1a over the UTF-16 units
     * the web hashes, so the same words on the same build collide on every
     * shell — which is the point, and safe only because it says nothing about
     * the person.
     */
    fun fingerprintOf(what: String, area: String, version: String): String {
        val seed = "${what.trim().lowercase().take(120)}|$area|$version"
        var hash = 0x811c9dc5.toInt()
        for (unit in seed) {
            hash = hash xor unit.code
            hash *= 0x01000193
        }
        return hash.toUInt().toString(16).padStart(8, '0')
    }

    /**
     * The whole payload, from the allowlist and nothing else. [screenshots]
     * are processed JPEG bytes ([ScreenshotPrep]), in tile order; more than
     * [MAX_SCREENSHOTS] never leave.
     */
    fun build(
        what: String,
        steps: String,
        area: String,
        labels: EnvironmentLabels,
        facts: DeviceFacts,
        screenshots: List<ByteArray> = emptyList(),
    ): Payload {
        val trimmed = what.trim()
        return Payload(
            what = trimmed,
            steps = steps.trim(),
            area = area,
            environment = environmentLines(labels, facts).joinToString("\n"),
            fingerprint = fingerprintOf(trimmed, area, facts.version),
            screenshots = screenshots.take(MAX_SCREENSHOTS).map { Base64.getEncoder().encodeToString(it) }.ifEmpty { null },
        )
    }

    private val json = Json { encodeDefaults = true; explicitNulls = false }

    /** The body as it leaves — the web's `JSON.stringify(payload)`. */
    fun body(payload: Payload): String = json.encodeToString(Payload.serializer(), payload)

    /**
     * Send it, or hand back the road that still works. Every non-2xx and every
     * fault falls back, deliberately: 503 is the designed one, but a person
     * whose report hit a 500 is owed the same second chance.
     */
    suspend fun send(
        payload: Payload,
        endpoint: String = endpointOverride ?: ENDPOINT,
        post: suspend (url: String, body: String, timeoutMs: Long) -> Answer = ::okHttpPost,
    ): Outcome {
        val fallbackUrl = BugReportUrl.prefilled(payload)
        // Checked here as well as there: a 413 round trip costs a spinner to
        // learn what this line knows before the request leaves. The cap is
        // the TEXT's (16 000 characters); screenshots have their own.
        if (body(payload.copy(screenshots = null)).length > MAX_REPORT_CHARS) return Outcome.Fallback(Reason.TooLarge, fallbackUrl)
        val attached = payload.screenshots.orEmpty()
        if (attached.size > MAX_SCREENSHOTS || attached.any { it.length.toLong() * 3 / 4 > MAX_SCREENSHOT_BYTES + 2 }) {
            return Outcome.Fallback(Reason.TooLarge, fallbackUrl)
        }
        val timeout = if (attached.isEmpty()) TIMEOUT_MS else TIMEOUT_WITH_SCREENSHOTS_MS
        return when (val answer = post(endpoint, body(payload), timeout)) {
            is Answer.Failed -> {
                VelaLog.event("feedback", if (answer.timedOut) "timed out" else "network error", "fallback" to true)
                Outcome.Fallback(Reason.Unreachable, fallbackUrl)
            }
            is Answer.Status -> {
                if (answer.code !in 200..299) {
                    val reason = when (answer.code) {
                        503 -> Reason.NotConfigured
                        429 -> Reason.RateLimited
                        413 -> Reason.TooLarge
                        // 400 too_many / invalid_screenshot, 415 unsupported_screenshot, 5xx.
                        else -> Reason.Rejected
                    }
                    VelaLog.event("feedback", "endpoint answered", "status" to answer.code, "fallback" to true)
                    return Outcome.Fallback(reason, fallbackUrl)
                }
                // A 200 this client cannot read is not a filed report it can point at.
                val filed = runCatching { JSONObject(answer.body) }.getOrNull()
                    ?: return Outcome.Fallback(Reason.Rejected, fallbackUrl)
                val number = filed.opt("number")
                val url = filed.opt("url")
                if (number !is Number || url !is String) return Outcome.Fallback(Reason.Rejected, fallbackUrl)
                Outcome.Filed(
                    number = number.toLong(),
                    url = url,
                    deduped = filed.optBoolean("deduped", false),
                    screenshotsDropped = filed.optInt("screenshotsDropped", 0).coerceAtLeast(0),
                )
            }
        }
    }

    private suspend fun okHttpPost(url: String, body: String, timeoutMs: Long): Answer = withContext(Dispatchers.IO) {
        // The app's one HTTP client (its pool, its TLS), with the report's own budget.
        val client = VelaHttp.client.newBuilder()
            .callTimeout(timeoutMs, TimeUnit.MILLISECONDS)
            .writeTimeout(timeoutMs, TimeUnit.MILLISECONDS)
            // The app's client never retries (a chain call must not go out
            // twice). A report may: a pooled connection the server has already
            // closed failed 重试 pressed right after a refusal with "network
            // error" — the same fallback again, which read as a dead button
            // (device pass, 078 round 3). A duplicate is merged server-side by
            // its fingerprint.
            .retryOnConnectionFailure(true)
            .build()
        val request = Request.Builder()
            .url(url)
            .post(body.toRequestBody("application/json".toMediaType()))
            .build()
        try {
            client.newCall(request).execute().use { response -> Answer.Status(response.code, response.body?.string().orEmpty()) }
        } catch (timeout: InterruptedIOException) {
            Answer.Failed(timedOut = true)
        } catch (fault: IOException) {
            Answer.Failed(timedOut = false)
        } catch (fault: IllegalArgumentException) {
            Answer.Failed(timedOut = false)
        }
    }
}
