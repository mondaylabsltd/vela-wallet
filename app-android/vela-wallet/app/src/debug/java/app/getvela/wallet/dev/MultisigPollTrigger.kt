package app.getvela.wallet.dev

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.multisig.MultisigNotifier

/**
 * Debug builds only (r9-8): one background round of the multisig poll NOW,
 * the way the periodic job runs it — WorkManager's own worker, in whatever
 * process the system starts — so a tester can prove a notification without
 * waiting 15 minutes. WorkManager refuses a forced run of the PERIODIC job
 * before its period ("executed before schedule"); this asks for a one-off
 * round of the same worker instead:
 *
 *   adb shell am kill app.getvela.wallet          # the app away, its process gone
 *   adb shell am broadcast -n app.getvela.wallet/.dev.MultisigPollTrigger
 *
 * The receiver needs `android.permission.DUMP` of its sender (the shell has
 * it; no app does). The release source set has no such class or manifest
 * entry, and `scripts/check-release-apk.sh` refuses a package carrying it.
 */
class MultisigPollTrigger : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        VelaLog.event("multisig.poll", "debug trigger")
        MultisigNotifier.runOnce(context.applicationContext)
    }
}
