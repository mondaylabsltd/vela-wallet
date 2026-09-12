package app.getvela.wallet

import android.app.Application
import app.getvela.wallet.core.data.ThemePreferenceRepository
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.dev.ParallelSpaceBinding
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocaleResolver
import app.getvela.wallet.feature.contacts.core.ContactsController
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.settings.core.SettingsController
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.platform.Haptics
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.NetworkEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.WalletController
import java.util.Locale
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob

/**
 * Manual composition root (research D8) — no DI framework at this scale.
 * Engine construction is blocking file+FFI work, so it runs on a dedicated
 * single-thread executor; locale updates queue behind it, which makes the
 * ready-before-setLocale ordering structural.
 */
class AppContainer(private val app: Application) {

    val i18nRuntime = I18nRuntime { tag ->
        app.assets.open("i18n/$tag.json").use { it.readBytes() }
    }

    val themeRepository = ThemePreferenceRepository(app)

    val accountStore = AccountStore(app)

    /**
     * The session machine lives HERE, not in a ViewModel.
     *
     * It is the route guard for the whole app and it outlives every screen. A
     * guard rebuilt whenever a screen is rebuilt would spend the first frame
     * after each rotation reporting `loading` and bouncing a signed-in person
     * back to onboarding.
     *
     * `SupervisorJob` so one failed effect cannot take the session down with it:
     * every operation already answers with its own failure variant, and a scope
     * that cancelled on the first exception would leave the app permanently in
     * `loading`.
     */
    val session = SessionController(
        store = accountStore,
        scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
    )

    /**
     * The settings machines, built on first use and never torn down.
     *
     * Lazy for a reason the session machine does not share: the route guard is
     * needed before the first frame, while a person who never opens Settings
     * should not pay for its storage read. Never torn down for the reason the
     * session machine *does* share: a machine rebuilt whenever a screen is
     * rebuilt spends the frame after every rotation reporting its placeholder
     * over a settled choice.
     */
    val settings: SettingsController by lazy {
        SettingsController(
            context = app,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            pool = pool,
        )
    }

    /**
     * The one way this app reads a chain.
     *
     * App-resident, and app-resident is not a style choice: the ban map and the
     * endpoint statistics are facts about the network that every read-path
     * machine shares. Two pools would mean two opinions about a dead endpoint,
     * and the second one would keep asking.
     *
     * Its endpoint list comes from the settings machine — which in turn needs
     * this pool to price a currency. The source reads that list lazily, which
     * is what lets both be built without either waiting for the other.
     *
     * `Dispatchers.IO`: the transport blocks, and confines itself, but the
     * driver's own JSON work has no business on the main thread either.
     */
    val pool: RpcPool by lazy {
        RpcPool(
            store = VelaStore(app),
            endpoints = NetworkEndpointSource { settings.networks.value },
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
        )
    }

    /**
     * The wallet home's machines — the RPC pool and the balance dashboard — on
     * the same terms as [settings].
     *
     * The pool lives here rather than on a screen because its ban map and
     * endpoint statistics are facts about the network that every read-path
     * machine shares. Two pools would mean two opinions about a dead endpoint.
     *
     * It reads its endpoints from the settings machine's own view, so there is
     * exactly one reader of what networks this person has.
     */
    val wallet: WalletController by lazy {
        WalletController(
            context = app,
            networks = settings.networks,
            pool = pool,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
            // A payment from one of this person's OWN wallets should say so by
            // name, and that costs no network call at all.
            ownAccounts = {
                session.view.value.accounts.map {
                    FeedExecutor.FeedOwnAccount(address = it.address, name = it.name)
                }
            },
            haptic = { Haptics.moneyIn(app) },
            foreground = { foregroundActivities > 0 },
        )
    }

    /** The address book, on the same terms as [settings]. */
    val contacts: ContactsController by lazy {
        ContactsController(
            context = app,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            // Paying another Vela user should show their name, not forty hex
            // characters. The index this asks is the same one onboarding
            // publishes to, through the same client.
            registryName = { address -> RegistryClient().nameForAddress(address) },
            code = { chainId, address ->
                (pool.call(chainId, "eth_getCode", listOf(address, "latest")) as? RpcResult.Body)
                    ?.json?.optString("result")?.takeIf { it.startsWith("0x") }
            },
        )
    }

    private val i18nExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "vela-i18n")
    }

    /**
     * How many of this app's screens are in front of somebody.
     *
     * Counted rather than a boolean, because a rotation stops one activity and
     * starts another and the count never reaches zero — a boolean set on stop
     * would report "backgrounded" for the instant in between, and the receive
     * watcher would end its session every time somebody turned their phone.
     */
    @Volatile
    private var foregroundActivities = 0

    /**
     * The one place this app learns it has been put away.
     *
     * `registerActivityLifecycleCallbacks` rather than a lifecycle observer in
     * a composable: the question is about the PROCESS, and a composable that
     * leaves the composition answers nothing at all.
     */
    private val activityCounter = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityStarted(activity: android.app.Activity) {
            foregroundActivities += 1
            wallet.focused()
        }

        override fun onActivityStopped(activity: android.app.Activity) {
            foregroundActivities = (foregroundActivities - 1).coerceAtLeast(0)
            if (foregroundActivities == 0) wallet.backgrounded()
        }

        override fun onActivityCreated(activity: android.app.Activity, state: android.os.Bundle?) = Unit
        override fun onActivityResumed(activity: android.app.Activity) = Unit
        override fun onActivityPaused(activity: android.app.Activity) = Unit
        override fun onActivitySaveInstanceState(activity: android.app.Activity, out: android.os.Bundle) = Unit
        override fun onActivityDestroyed(activity: android.app.Activity) = Unit
    }

    fun start() {
        // The balance machine has had a focus-driven auto-refresh since spec
        // 041 phase 4 and nothing was telling it; the receive watcher needs the
        // same signal to stop polling from a pocket.
        app.registerActivityLifecycleCallbacks(activityCounter)
        i18nExecutor.execute {
            i18nRuntime.initialize(LocaleResolver.resolve(currentLocales()))
        }
        session.boot()
    }

    /** Re-resolves the system locale (activity recreation on locale change). */
    fun applySystemLocale() {
        i18nExecutor.execute {
            i18nRuntime.setLocale(LocaleResolver.resolve(currentLocales()))
        }
    }

    private fun currentLocales(): List<Locale> {
        val localeList = app.resources.configuration.locales
        return (0 until localeList.size()).map { localeList[it] }
    }
}

class VelaWalletApplication : Application() {

    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        // First, so a crash in composition is itself on the record. Debug
        // builds only; see VelaLog.
        VelaLog.install(this)
        // The parallel space's door, if this build type has one (spec 043,
        // research D2): a release build installs nothing here.
        ParallelSpaceBinding.install(this)
        container = AppContainer(this)
        container.start()
    }
}
