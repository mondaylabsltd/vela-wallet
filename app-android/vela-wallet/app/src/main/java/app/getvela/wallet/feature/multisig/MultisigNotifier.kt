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
import app.getvela.wallet.feature.multisig.core.MsWaiting
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    /**
     * What was said (or seen on screen), per multisig (lowercase address) and
     * transaction (`<chainId>:<safeTxHash lowercase>`) — r11-1: never a
     * count. A count says nothing of WHICH: one approved elsewhere and one
     * new left it the same, and the new one went unsaid; a round that read
     * the list a moment late recorded a proposal it never announced.
     */
    @Serializable
    data class Seen(
        val needsYou: Map<String, List<String>> = emptyMap(),
        val ready: Map<String, List<String>> = emptyMap(),
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

    /**
     * Who is looking (r11-1). [OnScreen]: the person — what the views hold
     * is recorded as seen, nothing said. [Away]: a round that began at
     * [Away.since] (ms) — it says what a read since then holds and was not
     * said or seen, and records exactly that; a list read before it began
     * (or kept from an earlier page) is not its to say or to record.
     */
    sealed interface Look {
        data object OnScreen : Look
        data class Away(val since: Long) : Look
    }

    private val lock = Mutex()

    suspend fun seen(): Seen = store.read(STATE_KEY)?.let { runCatching { Wire.json.decodeFromString(Seen.serializer(), it) }.getOrNull() } ?: Seen()

    /**
     * What the machines' views say now, against what was said or seen: the
     * notes to post (posted here when [look] is [Look.Away]).
     */
    suspend fun evaluate(accounts: MsAccountsView, proposals: MsProposalsView, look: Look): List<Note> = lock.withLock {
        val w = words()
        val before = seen()
        val needsYou = before.needsYou.toMutableMap()
        val ready = before.ready.toMutableMap()
        val approved = before.approved.toMutableMap()
        val notes = mutableListOf<Note>()
        val away = look as? Look.Away
        fun fresh(waiting: MsWaiting): Boolean = away == null || (waiting.readAt != null && waiting.readAt >= away.since)
        for (row in accounts.accounts) {
            val key = row.account.address.lowercase()
            val waiting = proposals.presence[key] ?: continue
            // Never from a read this round did not make (r11-1).
            if (!fresh(waiting)) continue
            val name = w.name(row.account, row.name)
            val saidNeeds = needsYou[key].orEmpty()
            val saidReady = ready[key].orEmpty()
            if (away != null && waiting.needsYouKeys.any { it !in saidNeeds }) {
                // One note per multisig, its count the number waiting now (replaced, never stacked).
                notes += Note("ms-needs-$key", Channel.NeedsYou, name, w.plural("presence.needsYou", waiting.needsYou), waiting.firstChainId, waiting.firstHash, row.account.address)
            }
            if (away != null && waiting.readyKeys.any { it !in saidReady }) {
                notes += Note("ms-ready-$key", Channel.Ready, name, w.plural("presence.ready", waiting.ready), waiting.firstChainId, waiting.firstHash, row.account.address)
            }
            needsYou[key] = remember(saidNeeds, waiting.needsYouKeys)
            ready[key] = remember(saidReady, waiting.readyKeys)
        }
        // The multisig followed: an approval counted (another member's, while away).
        val followedName = accounts.row(proposals.followed)?.let { w.name(it.account, it.name) }.orEmpty()
        val followedWaiting = proposals.followed?.let { proposals.presence[it.lowercase()] }
        val followedFresh = if (away == null) true else followedWaiting?.let(::fresh) ?: false
        if (followedFresh) {
            for (item in proposals.items.filter { it.status != "done" }) {
                val was = approved[item.id]
                if (away != null && was != null && item.approved > was) {
                    notes += Note(
                        "ms-counted-${item.id}-${item.approved}", Channel.Counted, followedName,
                        "${w.actionTitle(item.chainId, item.action, item.nonce)} · ${w.t("status.progress", "approved" to item.approved.toString(), "threshold" to item.threshold.toString())}",
                        item.chainId, item.safeTxHash, proposals.followed,
                    )
                }
                approved[item.id] = item.approved
            }
        }
        val after = Seen(needsYou, ready, approved)
        if (after != before) store.write(STATE_KEY, Wire.json.encodeToString(Seen.serializer(), after))
        notes.forEach(this.post)
        notes
    }

    /** What was said, with [now] added — the newest kept, bounded. */
    private fun remember(said: List<String>, now: List<String>): List<String> {
        val added = now.filter { it !in said }
        return if (added.isEmpty()) said else (said + added).takeLast(KEPT_PER_MULTISIG)
    }

    /** An executed transaction (the proposals machine's `activity_rows`), said when away. */
    fun executed(title: String, chainId: Int, safeTxHash: String, away: Boolean) {
        if (!away) return
        post(Note("ms-executed-$chainId-${safeTxHash.lowercase()}", Channel.Executed, title, words().t("proposal.executed"), chainId, safeTxHash))
    }

    companion object {
        /**
         * What was said, per multisig and transaction (so nothing is said
         * twice). v2 (r11-1): by transaction — v1's counts are not read.
         */
        const val STATE_KEY = "vela.multisigNotified.v2"

        /** How many transactions said are kept per multisig (the newest). */
        internal const val KEPT_PER_MULTISIG = 200
        const val EXTRA_CHAIN = "vela.multisig.chain"
        const val EXTRA_HASH = "vela.multisig.hash"
        const val EXTRA_ADDRESS = "vela.multisig.address"
        internal const val WORK_NAME = "vela.multisig.poll"

        /**
         * r11-1: who is looking now — the person ([Look.OnScreen], an
         * activity resumed), nobody ([Look.Away] since [since], none even
         * started), or neither (`null`: started but not in front — a system
         * sheet over the app): neither said over it nor recorded unseen.
         * [since] `null`: no round in this process, nothing to measure from.
         */
        fun lookNow(onScreen: Boolean, started: Boolean, since: Long?): Look? = when {
            onScreen -> Look.OnScreen
            !started && since != null -> Look.Away(since)
            else -> null
        }

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
                work.cancelUniqueWork(LEAVING_NAME)
                return
            }
            work.enqueueUniquePeriodicWork(WORK_NAME, ExistingPeriodicWorkPolicy.UPDATE, periodicRequest())
            VelaLog.event("multisig.poll", "scheduled", "everyMin" to PERIOD_MINUTES, "flexMin" to FLEX_MINUTES)
        }

        /**
         * The periodic round (real device, r10: on MIUI it never ran on its
         * own). What a scheduler that defers work keeps honouring:
         * - Android's floor period with a flex window, so the system can
         *   batch it with other work instead of waking for it alone;
         * - a network (the only thing a round needs) and a battery not low —
         *   the constraints a battery saver uses to hold work back anyway,
         *   said up front so the job is not judged costly;
         * - an exponential back-off for a round that could not finish;
         * - UPDATE, never KEEP: an install with an older request (its
         *   constraints, its period) gets this one, its schedule kept.
         */
        internal fun periodicRequest(): androidx.work.PeriodicWorkRequest =
            PeriodicWorkRequestBuilder<MultisigPollWorker>(PERIOD_MINUTES, TimeUnit.MINUTES, FLEX_MINUTES, TimeUnit.MINUTES)
                .setConstraints(constraints())
                .setBackoffCriteria(androidx.work.BackoffPolicy.EXPONENTIAL, 1, TimeUnit.MINUTES)
                .addTag(WORK_NAME)
                .build()

        internal fun constraints(): Constraints =
            Constraints.Builder()
                .anyInternet()
                .setRequiresBatteryNotLow(true)
                .build()

        /**
         * A network with internet — NOT one Android has "validated". The
         * system validates a network by reaching Google; where Google is out
         * of reach (much of this wallet's market, a VPN off) the network is
         * never validated, and a job asking for `CONNECTED` (which JobScheduler
         * reads as INTERNET + VALIDATED) never runs at all — measured on the
         * emulator behind such a network: CONNECTIVITY unsatisfied for good.
         * The round reads the multisig service and the chains, not Google.
         */
        private fun Constraints.Builder.anyInternet(): Constraints.Builder =
            if (Build.VERSION.SDK_INT >= 28) {
                setRequiredNetworkRequest(
                    android.net.NetworkRequest.Builder()
                        .addCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET)
                        .removeCapability(android.net.NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                        .build(),
                    NetworkType.CONNECTED,
                )
            } else {
                setRequiredNetworkType(NetworkType.CONNECTED)
            }

        /**
         * The app was put away (its last screen stopped) with a multisig to
         * follow: one round now, EXPEDITED — the work an OEM scheduler still
         * runs promptly (a periodic job it may hold for hours) — so what
         * changes next is measured from what the person last saw, and the
         * first round does not wait for the periodic one. Out of expedited
         * quota it runs as ordinary work. Foreground-safe: a round posts
         * nothing while a screen of the app is in front (the worker asks).
         * Replaced, never stacked: leaving twice is one round.
         */
        fun checkOnLeaving(context: Context) {
            val request = androidx.work.OneTimeWorkRequestBuilder<MultisigPollWorker>()
                .setExpedited(androidx.work.OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
                .setConstraints(Constraints.Builder().anyInternet().build())
                .addTag(LEAVING_NAME)
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(LEAVING_NAME, androidx.work.ExistingWorkPolicy.REPLACE, request)
            VelaLog.event("multisig.poll", "leaving round asked")
        }

        /**
         * Whether the system holds this app's background work back — the
         * honest fact behind "a notification may come late": the person
         * restricted it outright (Android 9+'s background restriction,
         * "Restricted" battery use), or the system put it in a standby
         * bucket that runs jobs rarely or never (`rare`, `restricted`).
         * Android's default "Optimized" battery use is NOT that: periodic
         * work still runs in its maintenance windows, and saying otherwise
         * would be a warning every phone shows. OEM schedulers (MIUI's
         * battery saver, autostart) add their own, which no API reports;
         * [oemHint] names the makers whose settings are known to.
         */
        fun backgroundLimited(context: Context): Boolean {
            if (Build.VERSION.SDK_INT < 28) return false
            val activity = context.getSystemService(android.app.ActivityManager::class.java)
            if (activity?.isBackgroundRestricted == true) return true
            val usage = context.getSystemService(android.app.usage.UsageStatsManager::class.java)
            val bucket = runCatching { usage?.appStandbyBucket }.getOrNull() ?: return false
            return bucket >= android.app.usage.UsageStatsManager.STANDBY_BUCKET_RARE
        }

        /** The phone's maker is one whose own battery manager holds background work beyond Android's (MIUI, HyperOS, ColorOS, …). */
        fun oemHint(manufacturer: String = Build.MANUFACTURER): Boolean =
            manufacturer.lowercase() in setOf("xiaomi", "redmi", "poco", "huawei", "honor", "oppo", "realme", "oneplus", "vivo", "iqoo", "meizu", "samsung")

        /**
         * The system's page for this app's battery use — where the person
         * lets it run in the background. The app's own details page when the
         * maker has no direct one (the battery entry is one tap from it).
         */
        fun batterySettingsIntent(context: Context): Intent =
            Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.fromParts("package", context.packageName, null))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

        internal const val PERIOD_MINUTES = 15L
        internal const val FLEX_MINUTES = 5L

        /** The name the leaving round goes under (never two at once). */
        internal const val LEAVING_NAME = "vela.multisig.poll.leaving"

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
    /**
     * An expedited round ([MultisigNotifier.checkOnLeaving]) before Android
     * 12 runs as a foreground service, which must show a notification: a
     * silent, low one in its own channel, named in the corpus's words
     * ("Checking for transactions waiting for approval…"), gone with the
     * round. From Android 12 an expedited job needs none and this is not asked.
     */
    override suspend fun getForegroundInfo(): androidx.work.ForegroundInfo {
        val words = MultisigWiring.words((applicationContext as VelaWalletApplication).container)
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            manager?.createNotificationChannel(
                NotificationChannel(CHECK_CHANNEL, words.t("panel.checking"), NotificationManager.IMPORTANCE_MIN).apply { setShowBadge(false) },
            )
        }
        val notification = NotificationCompat.Builder(applicationContext, CHECK_CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle(words.t("panel.checking"))
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setSilent(true)
            .setOngoing(true)
            .build()
        return androidx.work.ForegroundInfo(CHECK_NOTIFICATION_ID, notification)
    }

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
            val since = hub.backgroundRound()
            val look = MultisigNotifier.lookNow(onScreen = container.onScreen(), started = container.inForeground(), since = since)
            val accounts = hub.accountsView.value
            val proposals = hub.proposalsView.value
            val notes = look?.let { container.multisigNotifier.evaluate(accounts, proposals, it) }.orEmpty()
            VelaLog.event(
                "multisig.poll", "evaluated",
                "multisigs" to accounts.accounts.size,
                // r11-1: how many of them a read of this round's holds (the rest are not its to say).
                "read" to accounts.accounts.count { row -> proposals.presence[row.account.address.lowercase()]?.readAt?.let { it >= since } == true },
                "waiting" to accounts.accounts.sumOf { row -> proposals.presence[row.account.address.lowercase()]?.needsYou ?: 0 },
                "look" to (look?.let { if (it is MultisigNotifier.Look.Away) "away" else "onScreen" } ?: "neither"),
                "notes" to notes.size,
                "allowed" to MultisigNotifier.allowed(applicationContext),
                "foreground" to container.inForeground(),
                "limited" to MultisigNotifier.backgroundLimited(applicationContext),
                "tags" to tags.joinToString(","),
            )
            true
        }
        VelaLog.event("multisig.poll", if (done == true) "round" else "timed out", "attempt" to runAttemptCount)
        // A round that could not finish (no network answer in time) is tried
        // again on WorkManager's back-off, a few times — then the next period.
        return if (done == true || runAttemptCount >= 3) Result.success() else Result.retry()
    }

    private companion object {
        const val CHECK_CHANNEL = "multisig_check"
        const val CHECK_NOTIFICATION_ID = 0x5e1a
    }
}
