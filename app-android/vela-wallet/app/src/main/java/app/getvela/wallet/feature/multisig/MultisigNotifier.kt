package app.getvela.wallet.feature.multisig

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import app.getvela.wallet.MainActivity
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.Serializable

/**
 * The multisig's notifications (M6, AUDIT §6 decision 3): local background
 * polling now — a WorkManager periodic job wakes the app, the proposals
 * machine reads every multisig here as it does on screen (`others_tick`, the
 * followed list's `refresh`) — and a local notification when something
 * changed for this device: a transaction needs its approval, an approval was
 * counted, one is ready to execute, one was executed. No push service and no
 * server learns anything: what changed is read from the mailbox and the chain
 * exactly as the screen reads it, and decided by the same core.
 *
 * Asked for at a sensible moment — the first multisig created or added here,
 * or the first proposal made — once per install (`NotificationAsk`).
 * Only when away: while the app is up the views say it; a banner over them is
 * noise.
 */
class MultisigNotifier(
    private val store: KeyValueStore,
    private val words: () -> MultisigWords,
    /** Posts a note (the system's; a test's list). */
    var post: (Note) -> Unit = {},
) {
    @Serializable
    data class Seen(
        val needsYou: Map<String, Int> = emptyMap(),
        val ready: Map<String, Int> = emptyMap(),
        /** Approvals counted per `chainId:hash`, for the multisig followed. */
        val approved: Map<String, Int> = emptyMap(),
    )

    /** One notification to post (testable without the system). */
    data class Note(
        val id: String,
        val channel: Channel,
        val title: String,
        val body: String,
        val chainId: Int? = null,
        val safeTxHash: String? = null,
        val address: String? = null,
    )

    /** The four channels a person can turn off apart in the system's settings. */
    enum class Channel(val id: String) { NeedsYou("multisig_needs_you"), Counted("multisig_counted"), Ready("multisig_ready"), Executed("multisig_executed") }

    suspend fun seen(): Seen = store.read(STATE_KEY)?.let { runCatching { Wire.json.decodeFromString(Seen.serializer(), it) }.getOrNull() } ?: Seen()

    /**
     * What the machines' views say now, against what was said last: the notes
     * to post. [post] `false` records the state and says nothing (the app is on
     * screen: the views said it).
     */
    suspend fun evaluate(accounts: MsAccountsView, proposals: MsProposalsView, post: Boolean): List<Note> {
        val w = words()
        val before = seen()
        val needsYou = before.needsYou.toMutableMap()
        val ready = before.ready.toMutableMap()
        val approved = before.approved.toMutableMap()
        val notes = mutableListOf<Note>()
        for (row in accounts.accounts) {
            val key = row.account.address.lowercase()
            val waiting = proposals.presence[key] ?: continue
            val name = w.name(row.account, row.name)
            if (waiting.needsYou > (needsYou[key] ?: 0)) {
                notes += Note("ms-needs-$key-${waiting.firstHash}", Channel.NeedsYou, name, w.plural("presence.needsYou", waiting.needsYou), waiting.firstChainId, waiting.firstHash, row.account.address)
            }
            if (waiting.ready > (ready[key] ?: 0)) {
                notes += Note("ms-ready-$key-${waiting.firstHash}", Channel.Ready, name, w.plural("presence.ready", waiting.ready), waiting.firstChainId, waiting.firstHash, row.account.address)
            }
            needsYou[key] = waiting.needsYou
            ready[key] = waiting.ready
        }
        // The multisig followed: an approval counted (another member's, while away).
        val followedName = accounts.row(proposals.followed)?.let { w.name(it.account, it.name) }.orEmpty()
        for (item in proposals.items.filter { it.status != "done" }) {
            val was = approved[item.id]
            if (was != null && item.approved > was) {
                notes += Note(
                    "ms-counted-${item.id}-${item.approved}", Channel.Counted, followedName,
                    "${w.actionTitle(item.chainId, item.action, item.nonce)} · ${w.t("status.progress", "approved" to item.approved.toString(), "threshold" to item.threshold.toString())}",
                    item.chainId, item.safeTxHash, proposals.followed,
                )
            }
            approved[item.id] = item.approved
        }
        store.write(STATE_KEY, Wire.json.encodeToString(Seen.serializer(), Seen(needsYou, ready, approved)))
        if (!post) return emptyList()
        notes.forEach(this.post)
        return notes
    }

    /** An executed transaction (the proposals machine's `activity_rows`), said when away. */
    fun executed(title: String, chainId: Int, safeTxHash: String, away: Boolean) {
        if (!away) return
        post(Note("ms-executed-$chainId-${safeTxHash.lowercase()}", Channel.Executed, title, words().t("proposal.executed"), chainId, safeTxHash))
    }

    companion object {
        /** What was last said, per multisig and transaction (so nothing is said twice). */
        const val STATE_KEY = "vela.multisigNotified.v1"
        const val EXTRA_CHAIN = "vela.multisig.chain"
        const val EXTRA_HASH = "vela.multisig.hash"
        const val EXTRA_ADDRESS = "vela.multisig.address"
        internal const val WORK_NAME = "vela.multisig.poll"

        /** The first time there is something to say: ask once (Android 13+; once per install). */
        fun askOnce(context: Context) {
            // The layer's launcher asks; this is the no-UI fallback (a worker, a test).
            VelaLog.event("multisig.notify", "ask requested", "sdk" to Build.VERSION.SDK_INT)
        }

        /** Post [note] through the system, on its channel (named in the person's language). */
        fun postToSystem(context: Context, words: MultisigWords, note: Note) {
            if (Build.VERSION.SDK_INT >= 33 &&
                ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                VelaLog.event("multisig.notify", "no permission", "id" to note.id.take(24))
                return
            }
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            if (Build.VERSION.SDK_INT >= 26) createChannels(manager, words)
            val open = Intent(context, MainActivity::class.java)
                .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .apply {
                    note.chainId?.let { putExtra(EXTRA_CHAIN, it) }
                    note.safeTxHash?.let { putExtra(EXTRA_HASH, it) }
                    note.address?.let { putExtra(EXTRA_ADDRESS, it) }
                }
            val pending = PendingIntent.getActivity(context, note.id.hashCode(), open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            val notification = NotificationCompat.Builder(context, note.channel.id)
                .setSmallIcon(android.R.drawable.stat_notify_more)
                .setContentTitle(note.title)
                .setContentText(note.body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(note.body))
                .setGroup("multisig")
                .setAutoCancel(true)
                .setContentIntent(pending)
                .build()
            manager.notify(note.id.hashCode(), notification)
            VelaLog.event("multisig.notify", "posted", "channel" to note.channel.id, "id" to note.id.take(24))
        }

        /**
         * r9-8: the channels exist from the app's start (named in the
         * person's language, renamed when it changes) — a person can see and
         * set them in the system's settings before the first one is posted,
         * and a post never meets a missing channel.
         */
        fun ensureChannels(context: Context, words: MultisigWords) {
            if (Build.VERSION.SDK_INT < 26) return
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            createChannels(manager, words)
            VelaLog.event("multisig.notify", "channels", "count" to Channel.entries.size)
        }

        /** r9-8: may this app post now (Android 13+ asks; before it, the app's switch in Settings). */
        fun allowed(context: Context): Boolean =
            (Build.VERSION.SDK_INT < 33 ||
                ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) &&
                androidx.core.app.NotificationManagerCompat.from(context).areNotificationsEnabled()

        /** The channels, named from the corpus (the screens' own words for each state). */
        fun createChannels(manager: NotificationManager, words: MultisigWords) {
            val named = listOf(
                Channel.NeedsYou to words.t("status.yourTurn"),
                Channel.Counted to words.t("proposal.approved"),
                Channel.Ready to words.t("status.ready"),
                Channel.Executed to words.t("proposal.executed"),
            )
            for ((channel, name) in named) {
                val importance = if (channel == Channel.NeedsYou || channel == Channel.Ready) NotificationManager.IMPORTANCE_HIGH else NotificationManager.IMPORTANCE_DEFAULT
                manager.createNotificationChannel(NotificationChannel(channel.id, name, importance))
            }
        }

        /**
         * The background poll, while there is a multisig to follow: every 15
         * minutes (Android's floor), on a network. `false` cancels it.
         */
        fun schedule(context: Context, on: Boolean) {
            val work = WorkManager.getInstance(context)
            if (!on) {
                work.cancelUniqueWork(WORK_NAME)
                return
            }
            val request = PeriodicWorkRequestBuilder<MultisigPollWorker>(15, TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .addTag(WORK_NAME)
                .build()
            work.enqueueUniquePeriodicWork(WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }

        /** The name a one-off round goes under (a debug build's trigger; never two at once). */
        internal const val ONCE_NAME = "vela.multisig.poll.once"

        /**
         * One round now, as the periodic job runs it — WorkManager's own
         * worker, in a process the system may have just started. A forced run
         * of the PERIODIC job is refused by WorkManager before its period
         * ("executed before schedule"), so a tester's trigger (debug builds
         * only, `dev/MultisigPollTrigger`) asks for this instead.
         */
        fun runOnce(context: Context) {
            val request = androidx.work.OneTimeWorkRequestBuilder<MultisigPollWorker>()
                .addTag(ONCE_NAME)
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(ONCE_NAME, androidx.work.ExistingWorkPolicy.REPLACE, request)
        }
    }
}

/**
 * One background round (M6): the machines read every multisig here, then
 * what changed is said — bounded, as a worker's time is.
 */
class MultisigPollWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val container = (applicationContext as VelaWalletApplication).container
        val hub = container.multisig
        val done = withTimeoutOrNull(60_000) {
            // r8-26: a cold process (the app swiped away, the worker woke it)
            // has not read its accounts yet: the machines booted on an empty
            // list judge this device a member of nothing — nothing "needs
            // you", nothing said. The session first, then the machines, told
            // this device's accounts again (a boot from before counts too).
            withTimeoutOrNull(15_000) { container.session.view.first { !it.loading } }
            container.i18nRuntime.include(app.getvela.wallet.core.i18n.I18nCatalogs.MULTISIG)
            hub.boot()
            hub.refreshLocal()
            if (hub.accountsView.value.accounts.isEmpty()) {
                MultisigNotifier.schedule(applicationContext, on = false)
                return@withTimeoutOrNull true
            }
            hub.backgroundRound()
            val notes = container.multisigNotifier.evaluate(hub.accountsView.value, hub.proposalsView.value, post = !container.inForeground())
            VelaLog.event(
                "multisig.poll", "evaluated",
                "multisigs" to hub.accountsView.value.accounts.size,
                "notes" to notes.size,
                "allowed" to MultisigNotifier.allowed(applicationContext),
                "foreground" to container.inForeground(),
            )
            true
        }
        VelaLog.event("multisig.poll", if (done == true) "round" else "timed out")
        return Result.success()
    }
}
