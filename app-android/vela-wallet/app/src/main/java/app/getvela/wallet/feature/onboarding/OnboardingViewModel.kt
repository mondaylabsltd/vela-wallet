package app.getvela.wallet.feature.onboarding

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.crux.CoreDriver
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.HybridCeremony
import app.getvela.wallet.feature.onboarding.core.CreateView
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.LoginView
import app.getvela.wallet.feature.onboarding.core.OnboardingExecutor
import app.getvela.wallet.feature.onboarding.core.PasskeyExecutor
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.core.RegistryResolver
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.onboarding.core.UsbSecurityKeyCeremony
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uniffi.vela_core_uniffi.CtapCredentialChoice
import uniffi.vela_core_uniffi.CreateWalletCore
import uniffi.vela_core_uniffi.LoginCore

/**
 * Both onboarding machines, and the one Activity-scoped thing they need.
 *
 * `AndroidViewModel` rather than `ViewModel` because a passkey ceremony needs an
 * Activity context — Credential Manager raises system UI, which cannot be done
 * from an application context. The Activity is supplied per call by the screen
 * (see [attach]) rather than held here, because holding one in a ViewModel that
 * survives rotation is a leaked Activity.
 *
 * The two machines are separate cores with separate drivers and ONE executor
 * between them: six of the eighteen operations are used by both flows, and the
 * contract is a single vocabulary. Two executors would be two places for those
 * six to drift.
 */
class OnboardingViewModel(application: Application) : AndroidViewModel(application) {

    private val container = (application as VelaWalletApplication).container
    private val store: AccountStore = container.accountStore
    private val session: SessionController = container.session

    /** The relying party and every ceremony. Rebuilt per attached Activity. */
    private var passkey: PasskeyExecutor? = null
    // Through the three layers (064): the index's answers are PROVED against the
    // registry contract, and the contract answers when the index cannot.
    private val registry = RegistryClient(resolver = RegistryResolver(container::rawEthCall))

    private var createDriver: CoreDriver? = null
    private var loginDriver: CoreDriver? = null

    /** The create machine's view; null until the flow has started. */
    var createView by mutableStateOf<CreateView?>(null)
        private set

    var loginView by mutableStateOf(LoginView(busy = false, endpointUnreachable = false))
        private set

    /** The prompt currently on screen, and the answer it is waiting for. */
    var pending by mutableStateOf<PendingPrompt?>(null)
        private set

    /** Non-null once onboarding is over — the host navigates and clears it. */
    var finished by mutableStateOf(false)
        private set

    /** The endpoint surface, opened by `endpoint_unreachable` or by hand. */
    var endpointSheetOpen by mutableStateOf(false)
        private set

    var endpointUrl by mutableStateOf(RegistryClient.DEFAULT_REGISTRY_URL)
        private set

    /** A shell fault. Never a user error — it means this app has a bug. */
    var fault by mutableStateOf<String?>(null)
        private set

    data class PendingPrompt(
        val kind: PromptKind,
        val confirmable: Boolean,
        val answer: CompletableDeferred<Boolean>,
    )

    /** The security key's PIN prompt (app-owned USB path only), and its answer. */
    var pendingPin by mutableStateOf<PendingPin?>(null)
        private set

    data class PendingPin(
        val product: String,
        val retries: Int,
        val isRetry: Boolean,
        val answer: CompletableDeferred<String?>,
    )

    /** Several wallets on one key — the picker, and its answer. */
    var pendingWalletPick by mutableStateOf<PendingWalletPick?>(null)
        private set

    data class PendingWalletPick(
        val choices: List<CtapCredentialChoice>,
        val answer: CompletableDeferred<Int?>,
    )

    /** Set while a USB key is blinking; the screen shows "touch your key". Kind
     *  is "presence" / "fingerprint" / "select", null when nothing is waiting. */
    var usbTouchWaiting by mutableStateOf<UsbTouch?>(null)
        private set

    data class UsbTouch(val kind: String, val product: String)

    /** The caBLE QR to show (the OTHER phone scans it), or null when none is up. */
    var cableQr by mutableStateOf<String?>(null)
        private set

    /** The "scan needs Location on" explainer (API ≤30), and its answer. */
    var pendingLocationAsk by mutableStateOf<PendingLocationAsk?>(null)
        private set

    data class PendingLocationAsk(val answer: CompletableDeferred<Boolean>)

    /** The "insert your security key" waiter, with the OEM OTG-off hint. */
    var pendingInsertKey by mutableStateOf<PendingInsertKey?>(null)
        private set

    data class PendingInsertKey(
        val otgLooksOff: Boolean,
        val answer: CompletableDeferred<Boolean>,
    )

    /**
     * The app-owned USB ceremony's UI seam. Every method BLOCKS the calling
     * (IO) thread until the person answers on the main thread — a synchronous
     * CTAP host callback cannot suspend, so it waits on a [CompletableDeferred]
     * the UI completes.
     */
    private val usbPrompts = object : UsbSecurityKeyCeremony.Prompts {
        override fun askPin(product: String, retries: Int, isRetry: Boolean): String? {
            val answer = CompletableDeferred<String?>()
            viewModelScope.launch { pendingPin = PendingPin(product, retries, isRetry, answer) }
            return runBlocking { answer.await() }
        }

        override fun askWhichWallet(choices: List<CtapCredentialChoice>): Int? {
            val answer = CompletableDeferred<Int?>()
            viewModelScope.launch { pendingWalletPick = PendingWalletPick(choices, answer) }
            return runBlocking { answer.await() }
        }

        override fun touchWaiting(kind: String?, product: String) {
            viewModelScope.launch {
                usbTouchWaiting = kind?.let { UsbTouch(it, product) }
            }
        }

        override fun askEnableLocation(): Boolean {
            val answer = CompletableDeferred<Boolean>()
            viewModelScope.launch { pendingLocationAsk = PendingLocationAsk(answer) }
            return runBlocking { answer.await() }
        }

        override fun awaitKeyInsertion(otgLooksOff: Boolean, probe: () -> Boolean): Boolean {
            val answer = CompletableDeferred<Boolean>()
            viewModelScope.launch {
                pendingInsertKey = PendingInsertKey(otgLooksOff, answer)
                // Hot-plug broadcasts aren't wired (ACTION_USB_DEVICE_ATTACHED),
                // so the sheet polls; the moment the key enumerates, it answers
                // itself and the ceremony carries on.
                while (answer.isActive) {
                    if (probe()) {
                        answer.complete(true)
                        break
                    }
                    kotlinx.coroutines.delay(800)
                }
                pendingInsertKey = null
            }
            return runBlocking { answer.await() }
        }
    }

    fun answerPin(pin: String?) {
        val prompt = pendingPin ?: return
        pendingPin = null
        prompt.answer.complete(pin)
    }

    fun answerWalletPick(index: Int?) {
        val prompt = pendingWalletPick ?: return
        pendingWalletPick = null
        prompt.answer.complete(index)
    }

    fun answerLocationAsk(agree: Boolean) {
        val prompt = pendingLocationAsk ?: return
        pendingLocationAsk = null
        prompt.answer.complete(agree)
    }

    fun cancelInsertKey() {
        // The poll loop clears the state after completion.
        pendingInsertKey?.answer?.complete(false)
    }

    init {
        viewModelScope.launch {
            // The stored override, applied before any machine can ask a
            // question: a flow that started against the default and then
            // switched mid-way would query two different registries for one
            // wallet.
            endpointUrl = session.registryUrl()
            registry.baseUrl = endpointUrl
        }
    }

    /**
     * Bind the ceremonies to a live Activity.
     *
     * Called from the composition on every entry. Cheap and idempotent — it
     * rebuilds only the passkey executor, which holds the context.
     */
    /**
     * Spec 043: the send path signs with the same ceremony sign-in uses —
     * this executor, attached to the real activity. `null` until attached.
     */
    fun signer(): app.getvela.wallet.feature.send.core.UserOpSigner? =
        passkey?.let { app.getvela.wallet.feature.send.core.PasskeyUserOpSigner(it) }

    fun attach(activityContext: android.content.Context) {
        if (passkey == null) {
            val isRealActivity = activityContext is app.getvela.wallet.MainActivity
            passkey = PasskeyExecutor(
                context = activityContext,
                // Present only from the real activity: the gallery and the
                // previews have no launcher to run a ceremony through, and a
                // null one simply means "Credential Manager for everything",
                // which is what those surfaces want anyway.
                securityKey = (activityContext as? app.getvela.wallet.MainActivity)
                    ?.securityKeyCeremony,
                // The app-owned USB path: available from any real activity,
                // needs no launcher (it talks to the key directly). It probes
                // for a plugged-in key per ceremony and steps aside when none
                // is there.
                usbSecurityKey = if (isRealActivity) {
                    UsbSecurityKeyCeremony(activityContext, usbPrompts)
                } else {
                    null
                },
                // The caBLE "sign in with your phone" path. Reuses the USB touch
                // prompt ("look at your phone") and needs a real activity for
                // the Bluetooth-permission launcher.
                hybrid = if (isRealActivity) {
                    HybridCeremony(activityContext, usbPrompts)
                } else {
                    null
                },
                showQr = { qr -> viewModelScope.launch { cableQr = qr } },
            )
        }
    }

    // -- create --------------------------------------------------------------

    fun startCreate() {
        if (createDriver != null) return
        val driver = CoreDriver(
            bridge = CreateWalletCore().asBridge(),
            scope = viewModelScope,
            perform = { operation -> executor().perform(operation) },
            onView = { json ->
                val next = CreateView.from(json)
                // Where the machine ACTUALLY is, per update. A create that
                // "does not work" always stops at some stage, and this is the
                // line that says which one it stopped at, with what running.
                VelaLog.event(
                    "create.view",
                    next.stage.name,
                    "busy" to next.busy,
                    "status" to (next.status ?: "-"),
                    "keys" to next.keys.size,
                    "confirmed" to next.keys.count { it.confirmed },
                    "canFinish" to next.canFinish,
                    "needsSecondKey" to next.needsSecondKey,
                )
                createView = next
            },
            escapedFailure = OnboardingExecutor::escapedFailure,
            onFault = { error ->
                VelaLog.failure("create.fault", "core fault", error)
                fault = error.message ?: error.toString()
            },
        )
        createDriver = driver
        driver.dispatch(event("start"))
    }

    fun nameChanged(name: String) =
        send(createDriver, JSONObject().put("type", "name_changed").put("name", name))

    fun ackToggled(index: Int) =
        send(createDriver, JSONObject().put("type", "ack_toggled").put("index", index))

    fun submit() = send(createDriver, JSONObject().put("type", "submit"))

    fun addKey(method: KeyMethod) = send(
        createDriver,
        JSONObject().put("type", "add_key").put("name", "").put("method", method.wire),
    )

    fun confirmKey(index: Int) =
        send(createDriver, JSONObject().put("type", "confirm_key").put("index", index))

    fun removeKey(index: Int) =
        send(createDriver, JSONObject().put("type", "remove_key").put("index", index))

    fun finishKeys() = send(createDriver, JSONObject().put("type", "finish_keys"))

    fun startOver() = send(createDriver, JSONObject().put("type", "start_over"))

    fun retryUpload() = send(createDriver, JSONObject().put("type", "retry_upload"))

    fun enterWallet() = send(createDriver, JSONObject().put("type", "enter_wallet"))

    fun goBack() = send(createDriver, JSONObject().put("type", "go_back"))

    // -- sign in -------------------------------------------------------------

    fun startLogin() {
        if (loginDriver != null) return
        val driver = CoreDriver(
            bridge = LoginCore().asBridge(),
            scope = viewModelScope,
            perform = { operation -> executor().perform(operation) },
            onView = { json ->
                val next = LoginView.from(json)
                // The endpoint surface opens the moment the health probe says
                // the index is unreachable — and sign-in stays permitted while
                // it is open. It is a warning with a fix attached, not a gate.
                if (next.endpointUnreachable && !loginView.endpointUnreachable) {
                    endpointSheetOpen = true
                }
                loginView = next
            },
            escapedFailure = OnboardingExecutor::escapedFailure,
            onFault = { error ->
                VelaLog.failure("login.fault", "core fault", error)
                fault = error.message ?: error.toString()
            },
        )
        loginDriver = driver
        driver.dispatch(event("start"))
    }

    fun signIn(method: KeyMethod = KeyMethod.Platform) =
        send(loginDriver, JSONObject().put("type", "sign_in").put("method", method.wire))

    /**
     * Start a sign-in, creating the machine if the last one finished.
     *
     * [startLogin] is a no-op when a driver already exists, which is what makes
     * a second tap during a live ceremony harmless — and what would make the
     * button dead forever after a completed sign-in, if `complete` did not drop
     * the finished machine.
     */
    fun beginSignIn(method: KeyMethod = KeyMethod.Platform) {
        startLogin()
        signIn(method)
    }

    // -- prompts -------------------------------------------------------------

    fun answerPrompt(accepted: Boolean) {
        val prompt = pending ?: return
        pending = null
        prompt.answer.complete(accepted)
    }

    // -- endpoint ------------------------------------------------------------

    fun openEndpointSheet() {
        endpointSheetOpen = true
    }

    fun dismissEndpointSheet() {
        endpointSheetOpen = false
    }

    fun saveEndpoint(url: String) {
        endpointSheetOpen = false
        val normalized = RegistryClient.normalize(url)
        endpointUrl = normalized
        registry.baseUrl = normalized
        viewModelScope.launch { session.setRegistryUrl(normalized) }
    }

    // -- lifecycle -----------------------------------------------------------

    /**
     * Leave the create flow.
     *
     * The driver is disposed and dropped rather than kept for a later re-entry:
     * a create machine holds drafted passkeys, and reusing one across an exit
     * would show the person a half-built wallet they thought they had abandoned.
     * Re-entering starts a fresh core — which finds any real draft in storage.
     */
    fun disposeCreate() {
        createDriver?.dispose()
        createDriver = null
        createView = null
    }

    fun disposeLogin() {
        loginDriver?.dispose()
        loginDriver = null
    }

    fun consumeFinished() {
        finished = false
    }

    private fun executor(): OnboardingExecutor {
        val ceremonies = passkey ?: error("onboarding executor used before attach()")
        return OnboardingExecutor(
            passkey = ceremonies,
            registry = registry,
            store = store,
            deps = object : OnboardingExecutor.Deps {
                override suspend fun prompt(kind: PromptKind, confirmable: Boolean): Boolean {
                    val answer = CompletableDeferred<Boolean>()
                    pending = PendingPrompt(kind, confirmable, answer)
                    return answer.await()
                }

                override suspend fun complete(mode: JSONObject) {
                    // Straight through to the session machine, untouched. The
                    // onboarding core is finished; whether there is a wallet to
                    // route to is the session machine's ruling, not this
                    // ViewModel's.
                    session.accountEstablished(mode)
                    finished = true

                    // A FINISHED machine is not a BUSY one.
                    //
                    // `login.rs` parks in `Stage::Completing` forever after a
                    // successful sign-in — deliberately, because it is done and
                    // will never act again — and `busy` is derived as
                    // `stage != Idle`, so it reads `true` from then on. Welcome
                    // renders that as a disabled 我已有钱包 button.
                    //
                    // Device-found 2026-08-25: sign in, sign out, and BOTH
                    // Welcome buttons are dead — the one-way door replaced by a
                    // dead end. The machine is right; rendering "done" as
                    // "working" was the bug.
                    loginView = LoginView(busy = false, endpointUnreachable = false)
                    disposeLogin()
                }
            },
        )
    }

    private fun send(driver: CoreDriver?, event: JSONObject) {
        driver?.dispatch(event.toString())
    }

    private fun event(type: String): String = JSONObject().put("type", type).toString()
}
