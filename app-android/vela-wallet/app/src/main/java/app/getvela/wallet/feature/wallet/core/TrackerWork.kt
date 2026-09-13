package app.getvela.wallet.feature.wallet.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import app.getvela.wallet.MainActivity
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.delay

/**
 * Money in flight outlives the screen (spec 043 US2, research D5/D6).
 *
 * While the app is in front, the wallet controller ticks the tracker every
 * three seconds. When it leaves with a hash still pending, this worker keeps
 * supplying the clock — every fifteen seconds for the core's two-minute wait
 * window, then it stops and leaves the rest to the next resume (the core
 * abandons at a day; nothing is lost, only delayed). The core still owns the
 * cadence: the worker only says "now".
 *
 * A verdict that lands while the app is away becomes a notification that
 * opens the wallet on that row.
 */
class TrackerWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val container = (applicationContext as VelaWalletApplication).container
        val wallet = container.wallet
        repeat(ROUNDS) { round ->
            if (!wallet.trackerHasPending()) return Result.success()
            wallet.trackerTick()
            VelaLog.event("tracker.worker", "tick", "round" to round)
            delay(INTERVAL_MS)
        }
        return Result.success()
    }

    companion object {
        private const val INTERVAL_MS = 15_000L

        /** 8 × 15 s ≈ the core's `WAIT_WINDOW_MS`. */
        private const val ROUNDS = 8
        private const val WORK_NAME = "vela.tracker"

        /** Enqueue (or replace) the background clock. Idempotent. */
        fun enqueue(context: Context) {
            val request = OneTimeWorkRequestBuilder<TrackerWorker>()
                .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(WORK_NAME, ExistingWorkPolicy.REPLACE, request)
        }
    }
}

/** The one notification this app posts: a send's verdict, when nobody is looking. */
object TrackerNotifier {
    private const val CHANNEL = "transactions"
    const val EXTRA_RECEIPT = "vela.receipt"

    fun notifyConfirmed(context: Context, userOpHash: String, chainId: Int, txHash: String, title: String, body: String) {
        if (ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            VelaLog.event("tracker.notify", "no permission", "hash" to userOpHash.take(12))
            return
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, title, NotificationManager.IMPORTANCE_DEFAULT),
        )
        val open = Intent(context, MainActivity::class.java)
            .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(EXTRA_RECEIPT, userOpHash)
        val pending = PendingIntent.getActivity(
            context,
            userOpHash.hashCode(),
            open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_upload_done)
            .setContentTitle(title)
            .setContentText(body)
            .setAutoCancel(true)
            .setContentIntent(pending)
            .build()
        manager.notify(userOpHash.hashCode(), notification)
        VelaLog.event("tracker.notify", "posted", "hash" to userOpHash.take(12), "chain" to chainId, "tx" to txHash.take(12))
    }
}
