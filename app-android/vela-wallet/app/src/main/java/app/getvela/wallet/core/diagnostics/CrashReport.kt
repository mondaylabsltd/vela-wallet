package app.getvela.wallet.core.diagnostics

import android.content.Context
import org.json.JSONObject

/**
 * A crash written FIRST, then rethrown (spec 047 D10, the desktop's
 * `panic_report`): the next launch raises it as the ordinary failure sheet
 * with Report, rather than a silent restart. Plain SharedPreferences on
 * purpose — the process is dying, and DataStore's write is asynchronous.
 * A core fault (`onFault`) records the same shape without dying.
 */
object CrashReport {
    /** The preferences file's name — reported by the erase when it survives. */
    const val FILE = "vela_crash"
    private const val KEY = "report"

    class Record(val atMs: Long, val thread: String, val message: String, val stack: String, val version: String, val fatal: Boolean)

    fun install(context: Context, version: String) {
        val app = context.applicationContext
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching { write(app, Record(System.currentTimeMillis(), thread.name, error.toString(), stackOf(error), version, fatal = true)) }
            previous?.uncaughtException(thread, error) ?: run {
                android.os.Process.killProcess(android.os.Process.myPid())
            }
        }
    }

    fun recordFault(context: Context, scope: String, error: Throwable, version: String) {
        runCatching { write(context.applicationContext, Record(System.currentTimeMillis(), scope, error.toString(), stackOf(error), version, fatal = false)) }
    }

    fun read(context: Context): Record? {
        val raw = context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE).getString(KEY, null) ?: return null
        val json = runCatching { JSONObject(raw) }.getOrNull() ?: return null
        return Record(json.optLong("at_ms"), json.optString("thread"), json.optString("message"), json.optString("stack"), json.optString("version"), json.optBoolean("fatal"))
    }

    fun clear(context: Context) {
        context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().remove(KEY).commit()
    }

    /**
     * The whole file, for the erase (spec 081 FR-017).
     *
     * [clear] removes the ONE key the sheet has shown; this removes the store.
     * The difference is the point: a crash record is a stack trace from the
     * person's own session, and 抹除此设备 that left it behind would be telling
     * them nothing of theirs is here while their last failure still is. Whole
     * store, so a field added to this file later goes without an edit here.
     *
     * @return false when the platform refused the commit.
     */
    fun eraseAll(context: Context): Boolean = runCatching {
        context.applicationContext
            .getSharedPreferences(FILE, Context.MODE_PRIVATE)
            .edit()
            .clear()
            .commit()
    }.getOrDefault(false)

    private fun write(context: Context, record: Record) {
        val json = JSONObject()
            .put("at_ms", record.atMs).put("thread", record.thread).put("message", record.message)
            .put("stack", record.stack).put("version", record.version).put("fatal", record.fatal)
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().putString(KEY, json.toString()).commit()
    }

    private fun stackOf(error: Throwable): String =
        error.stackTrace.take(12).joinToString("\n") { "  at $it" }
}
