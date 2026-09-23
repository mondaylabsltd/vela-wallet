package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.PayLink
import kotlinx.coroutines.flow.first
import android.graphics.Color
import android.os.Bundle
import app.getvela.wallet.dev.ParallelSpaceHook
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.designsystem.components.VelaLaunchAnimation
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.theme.isDarkEffective
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.SecurityKeyCeremony
import app.getvela.wallet.feature.onboarding.gallery.GalleryScreen
import app.getvela.wallet.navigation.VelaDestinations
import app.getvela.wallet.navigation.VelaNavHost
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    /**
     * The security-key ceremony, built HERE because an activity-result launcher
     * must be registered before the activity is STARTED — `attach()` runs from
     * composition, which is already too late and throws.
     */
    lateinit var securityKeyCeremony: SecurityKeyCeremony
        private set

    /**
     * Runtime Bluetooth permission for the caBLE scan/connect (spec 019). Like
     * [securityKeyCeremony] it must be registered before STARTED, so it lives
     * here and the onboarding flow calls [requestBluetoothPermission].
     */
    /** Spec 046: the camera, for the scanner; registered before STARTED like the others. */
    private lateinit var cameraPermissionLauncher:
        androidx.activity.result.ActivityResultLauncher<String>
    private var cameraPermissionAnswer:
        kotlinx.coroutines.CompletableDeferred<Boolean>? = null

    suspend fun requestCameraPermission(): Boolean {
        if (checkSelfPermission(android.Manifest.permission.CAMERA) == android.content.pm.PackageManager.PERMISSION_GRANTED) return true
        val answer = kotlinx.coroutines.CompletableDeferred<Boolean>()
        cameraPermissionAnswer = answer
        cameraPermissionLauncher.launch(android.Manifest.permission.CAMERA)
        return answer.await()
    }

    private lateinit var bluetoothPermissionLauncher:
        androidx.activity.result.ActivityResultLauncher<Array<String>>
    private var bluetoothPermissionAnswer:
        kotlinx.coroutines.CompletableDeferred<Boolean>? = null
    private lateinit var bluetoothEnableLauncher:
        androidx.activity.result.ActivityResultLauncher<android.content.Intent>
    private var bluetoothEnableAnswer:
        kotlinx.coroutines.CompletableDeferred<Boolean>? = null
    private lateinit var locationSettingsLauncher:
        androidx.activity.result.ActivityResultLauncher<android.content.Intent>
    private var locationSettingsAnswer:
        kotlinx.coroutines.CompletableDeferred<Unit>? = null

    override fun onDestroy() {
        super.onDestroy()
    }

    /** The permissions the caBLE scan needs on this API level. */
    private fun bluetoothPermissions(): Array<String> =
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) {
            arrayOf(
                android.Manifest.permission.BLUETOOTH_SCAN,
                android.Manifest.permission.BLUETOOTH_CONNECT,
            )
        } else {
            arrayOf(android.Manifest.permission.ACCESS_FINE_LOCATION)
        }

    /** Grant (or confirm) the Bluetooth permissions; `true` if all are held. */
    suspend fun requestBluetoothPermission(): Boolean =
        requestPermissions(bluetoothPermissions())

    /**
     * Grant (or confirm) exactly these permissions; `true` if all are held.
     *
     * Spec 075 T040 asks for a different pair from the caBLE scan above —
     * `BLUETOOTH_ADVERTISE` and `BLUETOOTH_CONNECT`, because there the phone
     * is the peripheral — so the launcher is shared and the set is the
     * caller's.
     */
    suspend fun requestPermissions(permissions: Array<String>): Boolean {
        val needed = permissions.filter {
            checkSelfPermission(it) != android.content.pm.PackageManager.PERMISSION_GRANTED
        }
        if (needed.isEmpty()) return true
        val answer = kotlinx.coroutines.CompletableDeferred<Boolean>()
        bluetoothPermissionAnswer = answer
        bluetoothPermissionLauncher.launch(needed.toTypedArray())
        return answer.await()
    }

    /**
     * Ask the SYSTEM to turn Bluetooth on (its own localized dialog), rather
     * than dead-ending the scan method on a radio that is merely off. `true`
     * once the adapter is on. Ordering matters on API 31+: the caller has
     * already granted BLUETOOTH_CONNECT ([requestBluetoothPermission]), which
     * ACTION_REQUEST_ENABLE requires.
     *
     * Device-found on a OnePlus 5T (2026-08-28): Bluetooth off surfaced as
     * "this device does not support biometrics" — a NotSupported alert for a
     * state the person can fix with one tap, and a sentence about the wrong
     * subject entirely.
     */
    suspend fun requestEnableBluetooth(): Boolean {
        val answer = kotlinx.coroutines.CompletableDeferred<Boolean>()
        bluetoothEnableAnswer = answer
        bluetoothEnableLauncher.launch(
            android.content.Intent(android.bluetooth.BluetoothAdapter.ACTION_REQUEST_ENABLE),
        )
        return answer.await()
    }

    /**
     * Take the person straight to the system Location page and return when they
     * come back. AOSP has no in-place enable dialog for location (that is a
     * Play-services SettingsClient exclusive, and GMS-less devices are exactly
     * where the API ≤30 scan gate bites), so the deepest link available IS the
     * settings page — the caller re-checks the toggle on return.
     */
    suspend fun openLocationSettings() {
        val answer = kotlinx.coroutines.CompletableDeferred<Unit>()
        locationSettingsAnswer = answer
        locationSettingsLauncher.launch(
            android.content.Intent(android.provider.Settings.ACTION_LOCATION_SOURCE_SETTINGS),
        )
        answer.await()
    }

    /**
     * Deterministic disable for instrumented tests (FR-029).
     *
     * An intent extra rather than a build flag: existing tests must be able to
     * skip the animation without a separate build, and a sleep long enough to
     * outlast it is exactly the flaky waiting this replaces.
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.skipLaunchAnimation true
     */
    private fun launchAnimationDisabled(): Boolean =
        intent?.getBooleanExtra("vela.skipLaunchAnimation", false) == true

    /**
     * The system's reduce-motion setting (FR-019). Read here rather than inside
     * the component so tests can drive both paths without touching Settings.
     */
    private fun reduceMotion(): Boolean =
        Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f

    /**
     * Dev-only state gallery (spec 014 FR-013), same intent-extra house pattern
     * as the launch-animation switch — release builds simply never receive it:
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.gallery true
     */
    private fun galleryRequested(): Boolean =
        intent?.getBooleanExtra("vela.gallery", false) == true

    /**
     * Launch-time start-route override (spec 015, research D4): keeps the
     * gallery/wallet reachable without touching production navigation.
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity --es vela.startDestination gallery
     */
    /**
     * Settings-gallery state pin (spec 023). The gallery's chip row scrolls, so
     * driving it by tap is 21 fragile coordinate guesses; this makes every state
     * a one-line launch, which is how the desktop's VELA_SETTINGS_STATE and the
     * iOS pin already work.
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity \
     *     --es vela.startDestination settings-gallery --es vela.settingsState ST7
     */
    private fun settingsState(): String? = intent?.getStringExtra("vela.settingsState")

    /** Forces the gallery's theme, so a sweep can cover light and dark. */
    private fun settingsDark(): Boolean? =
        if (intent?.hasExtra("vela.settingsDark") == true) {
            intent?.getBooleanExtra("vela.settingsDark", false)
        } else {
            null
        }

    private fun startDestination(): String =
        intent?.getStringExtra("vela.startDestination")
            ?.takeIf { it in VelaDestinations.ALL }
            ?: VelaDestinations.WELCOME

    /**
     * Opens a gallery straight onto one state (spec 021). Same family as the
     * route extra, and the same reason the desktop has `VELA_FLOW`: walking a
     * thirty-state chip strip by hand is not a repeatable device pass.
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity \
     *     --es vela.startDestination flows-gallery --es vela.flowState SD2B
     */
    private fun startFlowState(): String? = intent?.getStringExtra("vela.flowState")

    /**
     * The notification's door (spec 043 phase 4): a send's verdict landed while
     * the app was away; open the wallet on that row.
     */
    private fun receiptRequested(): String? = intent?.getStringExtra("vela.receipt")?.takeIf { it.startsWith("0x") }

    /**
     * The parallel space's door (spec 043, research D2). Debug builds only —
     * release builds have no provider behind the hook, and the extra does
     * nothing there. Remembered across relaunches; sign-out leaves.
     *
     *   adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true
     *   … --ez vela.parallelSpace false   # leave without signing the device out
     */
    private fun parallelSpaceRequested(): Boolean? =
        if (intent?.hasExtra("vela.parallelSpace") == true) {
            intent?.getBooleanExtra("vela.parallelSpace", false)
        } else {
            null
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        // A fresh process, not a configuration change or a restored activity.
        // The gallery skips the launch animation for deterministic walkthroughs.
        val coldStart = savedInstanceState == null && !launchAnimationDisabled() && !galleryRequested()
        super.onCreate(savedInstanceState)
        securityKeyCeremony = SecurityKeyCeremony(this)
        cameraPermissionLauncher = registerForActivityResult(
            androidx.activity.result.contract.ActivityResultContracts.RequestPermission(),
        ) { granted ->
            cameraPermissionAnswer?.complete(granted)
            cameraPermissionAnswer = null
        }
        (application as VelaWalletApplication).container.documents = app.getvela.wallet.feature.documents.ActivityDocumentPorts(this)
        (application as VelaWalletApplication).container.clearSignerTab = app.getvela.wallet.feature.signing.clearsigner.ClearSignerTab(this)
        bluetoothPermissionLauncher = registerForActivityResult(
            androidx.activity.result.contract.ActivityResultContracts.RequestMultiplePermissions(),
        ) { grants ->
            bluetoothPermissionAnswer?.complete(grants.values.all { it })
            bluetoothPermissionAnswer = null
        }
        bluetoothEnableLauncher = registerForActivityResult(
            androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult(),
        ) { result ->
            bluetoothEnableAnswer?.complete(result.resultCode == RESULT_OK)
            bluetoothEnableAnswer = null
        }
        locationSettingsLauncher = registerForActivityResult(
            androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult(),
        ) {
            locationSettingsAnswer?.complete(Unit)
            locationSettingsAnswer = null
        }
        enableEdgeToEdge()

        val container = (application as VelaWalletApplication).container
        // Spec 047: the stored language wins over the system once the preferences are read.
        lifecycleScope.launch {
            val prefs = container.preferences.view.first { it.loaded }
            container.applyLanguage(prefs.language)
        }
        receiptRequested()?.let { container.pendingReceipt.value = it }
        intent?.getStringExtra("vela.openUrl")?.let { container.browser.open(it, fromOutside = true) }
        routeDeepLink(intent, container)
        // Spec 047 US4: a forced crash for the device pass — debug builds only.
        if (BuildConfig.DEBUG && intent?.getBooleanExtra("vela.testPanic", false) == true) {
            android.os.Handler(mainLooper).postDelayed({ throw IllegalStateException("vela.testPanic") }, 2_000)
        }
        when (parallelSpaceRequested()) {
            true -> ParallelSpaceHook.enter()
            false -> ParallelSpaceHook.leave()
            null -> Unit
        }

        // Null until the persisted preference is actually read — the splash stays up,
        // so the first frame can never render the wrong palette (data-model MainUiState).
        val themePreferenceState: StateFlow<ThemePreference?> = container.themeRepository
            .themePreference
            .map { preference -> preference as ThemePreference? }
            .stateIn(lifecycleScope, SharingStarted.Eagerly, null)

        splash.setKeepOnScreenCondition {
            !container.i18nRuntime.state.value.ready || themePreferenceState.value == null
        }

        setContent {
            val i18nState by container.i18nRuntime.state.collectAsStateWithLifecycle()
            val themePreference by themePreferenceState.collectAsStateWithLifecycle()

            val preference = themePreference
            if (!i18nState.ready || preference == null) return@setContent

            val darkTheme = preference.isDarkEffective()

            // enableEdgeToEdge's defaults key bar-icon appearance off the SYSTEM uiMode
            // only; the FR-006 override must re-style the bars for the EFFECTIVE theme.
            DisposableEffect(darkTheme) {
                enableEdgeToEdge(
                    statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { darkTheme },
                    navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { darkTheme },
                )
                onDispose {}
            }

            // New provider identity per resolved language: every t() reader recomposes,
            // but no state (nav back stack, pager page) is discarded — unlike key().
            val strings = remember(i18nState.language) {
                LanguageSnapshot(container.i18nRuntime)
            }
            // Engine dir() drives layout direction so a future RTL locale cannot
            // silently render LTR (spec edge case).
            val layoutDirection =
                if (i18nState.direction == "rtl") LayoutDirection.Rtl else LayoutDirection.Ltr

            val prefs by container.preferences.view.collectAsStateWithLifecycle()
            VelaTheme(darkTheme = darkTheme, fontScale = prefs.textScale.factor) {
                val colors = VelaTheme.colors

                // Spec 012. `coldStart` is true only for a fresh process — a
                // configuration change or a restored activity must not replay it
                // (FR-008). This runs AFTER the existing splash gate releases, so
                // the animation never competes with i18n/theme readiness.
                var launching by rememberSaveable { mutableStateOf(coldStart) }
                var pageAlpha by remember { mutableFloatStateOf(if (coldStart) 0f else 1f) }

                CompositionLocalProvider(
                    LocalVelaStrings provides strings,
                    LocalLayoutDirection provides layoutDirection,
                ) {
                    // One continuous surface. Both the launch screen and Welcome
                    // sit on this exact colour, which is what lets them
                    // cross-dissolve without a washed-out middle (FR-012).
                    Box(modifier = Modifier.fillMaxSize().background(colors.bgBase)) {
                        if (galleryRequested()) {
                            // Spec 014: dev-only state gallery replaces the NavHost;
                            // it manages its own in-gallery theme toggle. Spec 015's
                            // wallet/gallery routes ride the startDestination extra.
                            GalleryScreen(initialDarkTheme = darkTheme)
                        } else {
                            // Welcome is composed from the first frame, hidden by the
                            // opaque overlay, so the hand-off has nothing left to
                            // build (FR-013a).
                            Box(modifier = Modifier.alpha(pageAlpha)) {
                                VelaNavHost(
                                    darkTheme = darkTheme,
                                    themePreference = preference,
                                    onThemeSelected = { selected ->
                                        lifecycleScope.launch {
                                            container.themeRepository.setThemePreference(selected)
                                        }
                                    },
                                    startDestination = startDestination(),
                                    startFlowState = startFlowState(),
                                    settingsState = settingsState(),
                                    settingsDark = settingsDark(),
                                )
                            }

                            if (launching) {
                                VelaLaunchAnimation(
                                    darkTheme = darkTheme,
                                    reduceMotion = reduceMotion(),
                                    backgroundColor = colors.bgBase,
                                    onProgress = { pageAlpha = it },
                                    onFinished = {
                                        pageAlpha = 1f
                                        launching = false
                                    },
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val container = (application as VelaWalletApplication).container
        intent.getStringExtra("vela.openUrl")?.let { container.browser.open(it, fromOutside = true) }
        routeDeepLink(intent, container)
        // Spec 070: the page in front's renderer dies on purpose — debug builds only.
        if (BuildConfig.DEBUG && intent.getBooleanExtra("vela.crashRenderer", false)) container.browser.debugCrashRenderer()
    }

    /** Spec 047 D8: `velawallet://` and `/pay` links, tokenized here, validated by the core on the wallet route. */
    private fun routeDeepLink(intent: android.content.Intent?, container: AppContainer) {
        val data = intent?.data ?: return
        when (val link = PayLink.parse(data.toString())) {
            is PayLink.Open -> container.browser.open(link.url, fromOutside = true)
            is PayLink.Pay -> container.pendingPayLink.value = data
            null -> Unit
        }
    }
}

/** Identity-per-language wrapper; delegates every lookup to the live runtime. */
private class LanguageSnapshot(delegate: VelaStrings) : VelaStrings by delegate
