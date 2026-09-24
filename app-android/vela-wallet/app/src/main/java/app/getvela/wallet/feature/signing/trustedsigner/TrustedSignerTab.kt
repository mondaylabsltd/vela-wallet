package app.getvela.wallet.feature.signing.trustedsigner

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import java.lang.ref.WeakReference

/**
 * What the Trusted Signer needs from the activity (spec 071): the page in a
 * Custom Tab inside THIS task, and the way back over it.
 *
 * A Custom Tab is a `VIEW` intent carrying the session extra — the whole of
 * what `androidx.browser` adds for a tab with no callbacks — so a browser that
 * supports tabs opens one in this task and one that does not opens the page
 * normally. In this task, the app is still the foreground task's root while
 * the page is up, which is what lets [bringBack] close the tab when the
 * answer arrives.
 */
class TrustedSignerTab(activity: Activity) {
    private val activity = WeakReference(activity)

    fun open(url: String): Boolean {
        val host = activity.get() ?: return false
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).apply {
            putExtras(Bundle().apply { putBinder(EXTRA_SESSION, null) })
            // A browser, named — never the chooser, where apps that merely
            // claim https links sit beside the browsers (device-found).
            browser(host)?.let(::setPackage)
        }
        return runCatching { host.startActivity(intent) }.isSuccess
    }

    /** The person's default browser if it hosts Custom Tabs, else Chrome, else any that does. */
    private fun browser(host: Activity): String? {
        val pm = host.packageManager
        val tabs = pm.queryIntentServices(Intent(ACTION_TABS_SERVICE), 0).map { it.serviceInfo.packageName }
        val default = pm.resolveActivity(
            Intent(Intent.ACTION_VIEW, Uri.parse("https://")).addCategory(Intent.CATEGORY_BROWSABLE),
            android.content.pm.PackageManager.MATCH_DEFAULT_ONLY,
        )?.activityInfo?.packageName
        return default?.takeIf { it in tabs } ?: tabs.firstOrNull { it == CHROME } ?: tabs.firstOrNull()
    }

    /**
     * Put the wallet back in front of the page.
     *
     * **NEW_TASK is what does the work, and it took a device to find that.**
     * CLEAR_TOP finishes a tab that is above this activity in THIS task, which
     * is what the loopback channel needed — but Chrome runs the Custom Tab in a
     * task of its own, so there is nothing above us to clear and our task is
     * simply behind Chrome's. Without NEW_TASK the system delivers the intent to
     * an activity that is already top-of-its-own-task and moves nothing:
     * measured as `START … LAUNCH_SINGLE_TOP … result code=3`
     * (`START_DELIVERED_TO_TOP`), with the wallet still PAUSED behind the page.
     *
     * With NEW_TASK the system finds the task this activity already lives in and
     * brings THAT forward. SINGLE_TOP keeps the instance and its state — an
     * intent with no data routes nothing in `onNewIntent` — so this is a resume
     * and never a rebuild.
     */
    fun bringBack() {
        val host = activity.get() ?: return
        host.runOnUiThread {
            host.startActivity(
                Intent(host, host.javaClass).addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or
                        Intent.FLAG_ACTIVITY_CLEAR_TOP or
                        Intent.FLAG_ACTIVITY_SINGLE_TOP,
                ),
            )
        }
    }

    private companion object {
        const val EXTRA_SESSION = "android.support.customtabs.extra.SESSION"
        const val ACTION_TABS_SERVICE = "android.support.customtabs.action.CustomTabsService"
        const val CHROME = "com.android.chrome"
    }
}
