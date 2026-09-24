package app.getvela.wallet.core.diagnostics

import android.content.Context
import android.util.Log
import app.getvela.wallet.BuildConfig
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/**
 * An on-device flight recorder for the onboarding ceremonies.
 *
 * **Why a file and not Logcat.** A USB security key occupies the phone's only
 * port, so the machine that would read `adb logcat` cannot be attached while
 * the thing being diagnosed is happening (founder, 2026-08-26). The log has to
 * survive the session and be readable afterwards.
 *
 * Written to the app's own external files directory, which needs no permission
 * and is reachable once the key is unplugged:
 *
 *     adb pull /sdcard/Android/data/app.getvela.wallet/files/logs/
 *
 * **Debug builds only.** A shipping wallet does not write a trace of its
 * owner's ceremonies to shared storage. `BuildConfig.DEBUG` is the whole gate,
 * so a release build carries the calls and does nothing with them.
 *
 * **What is in it.** Operation names, credential ids, transports, provider
 * names, HTTP status codes, exception classes and messages, and timings. No
 * private key material exists outside the authenticator to log, and no
 * challenge or signature is written — those say nothing about a routing bug and
 * would only make the file worth stealing.
 */
object VelaLog {
    private const val TAG = "VelaLog"
    private const val MAX_BYTES = 512 * 1024
    private const val KEPT_ROTATIONS = 2

    private val timestamp = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS", Locale.US).apply {
        timeZone = TimeZone.getTimeZone("UTC")
    }
    private val day = SimpleDateFormat("yyyyMMdd", Locale.US).apply {
        timeZone = TimeZone.getTimeZone("UTC")
    }

    @Volatile
    private var directory: File? = null

    /** Called once from the composition root. A no-op in release builds. */
    fun install(context: Context) {
        if (!BuildConfig.DEBUG) return
        val dir = context.getExternalFilesDir("logs") ?: context.filesDir.resolve("logs")
        dir.mkdirs()
        directory = dir
        event(
            "app",
            "session start",
            "version" to BuildConfig.VERSION_NAME,
            "device" to "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}",
            "sdk" to android.os.Build.VERSION.SDK_INT,
            "log" to dir.absolutePath,
        )
    }

    /** Where the log lives, for a share sheet or a bug report. */
    fun currentFile(): File? = directory?.resolve("vela-${day.format(Date())}.log")

    /**
     * Delete every log file, and forget the failures held in memory
     * (spec 081 FR-017).
     *
     * These files carry credential ids, transports, provider names, HTTP
     * statuses and timings for the person's own ceremonies — nothing secret,
     * but unmistakably theirs, and written to **external** storage where the
     * files outlive an uninstall. An erase that swept the DataStore and left
     * `/sdcard/Android/data/app.getvela.wallet/files/logs/` intact was telling
     * somebody their device was clean while a trace of every sign-in they had
     * made sat there for the next app with storage access to read.
     *
     * A release build never wrote any, so there is nothing to delete and this
     * returns true — the honest answer for "is it gone".
     *
     * @return false only when a file that exists could not be removed.
     */
    fun eraseFiles(): Boolean {
        synchronized(recent) { recent.clear() }
        val dir = directory ?: return true
        return runCatching {
            dir.listFiles()?.all { it.deleteRecursively() } ?: true
        }.getOrDefault(false)
    }

    /** One line: a step that happened, with the fields that explain it. */
    fun event(scope: String, message: String, vararg fields: Pair<String, Any?>) {
        write(scope, message, fields.toList(), error = null)
    }

    /** One line: a step that failed, with the exception's own chain. */
    fun failure(scope: String, message: String, error: Throwable, vararg fields: Pair<String, Any?>) {
        write(scope, message, fields.toList(), error)
        if (scope.endsWith(".fault")) onFault?.invoke(scope, error)
        synchronized(recent) {
            recent.addLast("$scope: $message (${error.javaClass.simpleName})")
            if (recent.size > 8) recent.removeFirst()
        }
    }

    /** Spec 047 D10: a core fault reaches the failure sheet like a crash would. */
    @Volatile
    var onFault: ((String, Throwable) -> Unit)? = null

    /** The last few failures, for the bug report's preview (spec 047). */
    private val recent = ArrayDeque<String>()
    fun recentFailures(): List<String> = synchronized(recent) { recent.toList() }

    /**
     * Credential ids are long and the interesting part is whether two lines are
     * about the SAME key; head and tail answer that without a wall of hex.
     */
    fun shortId(hex: String?): String = when {
        hex.isNullOrEmpty() -> "-"
        hex.length <= 16 -> hex
        else -> "${hex.take(8)}…${hex.takeLast(6)}"
    }

    private fun write(
        scope: String,
        message: String,
        fields: List<Pair<String, Any?>>,
        error: Throwable?,
    ) {
        if (!BuildConfig.DEBUG) return
        val line = buildString {
            append(timestamp.format(Date()))
            append("  ")
            append(scope.padEnd(18))
            append(message)
            fields.forEach { (key, value) ->
                append("  ")
                append(key)
                append('=')
                append(value ?: "-")
            }
            if (error != null) {
                append("  error=")
                append(error.javaClass.name)
                append(" msg=")
                append(error.message?.replace('\n', ' ') ?: "-")
                // The chain, not just the top: Credential Manager wraps the
                // interesting failure two or three layers down.
                var cause = error.cause
                var depth = 0
                while (cause != null && depth < 4) {
                    append("  causedBy=")
                    append(cause.javaClass.name)
                    append(':')
                    append(cause.message?.replace('\n', ' ') ?: "-")
                    cause = cause.cause
                    depth += 1
                }
            }
        }
        logcat(line)
        appendToFile(line)
    }

    /**
     * **A log line must never become a failure.**
     *
     * `android.util.Log` is a stub on the JVM and throws "not mocked" — so the
     * first executor to log inside an operation threw instead of answering, and
     * the machine waiting on that answer hung until the test's own timeout. A
     * shell operation that never answers is the one contract violation this
     * codebase cannot recover from, and it arrived through a diagnostic.
     *
     * The file write below has always been guarded; Logcat was not.
     */
    private fun logcat(line: String) {
        try {
            Log.i(TAG, line)
        } catch (_: Throwable) {
            // No Android runtime (unit tests). The file, if any, still has it.
        }
    }

    @Synchronized
    private fun appendToFile(line: String) {
        val file = currentFile() ?: return
        try {
            if (file.length() > MAX_BYTES) rotate(file)
            file.appendText(line + "\n")
        } catch (error: Exception) {
            // A diagnostic that crashes the thing it is diagnosing is worse than
            // no diagnostic. Logcat still has the line.
            // Same rule as above: reporting the failure must not become one.
            try {
                Log.w(TAG, "could not write the log file", error)
            } catch (_: Throwable) {
            }
        }
    }

    private fun rotate(file: File) {
        for (index in KEPT_ROTATIONS downTo 1) {
            val older = File("${file.path}.$index")
            if (index == KEPT_ROTATIONS) older.delete() else older.renameTo(File("${file.path}.${index + 1}"))
        }
        file.renameTo(File("${file.path}.1"))
    }
}
