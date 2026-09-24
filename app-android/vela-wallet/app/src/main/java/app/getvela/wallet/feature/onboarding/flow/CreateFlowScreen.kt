package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.OnboardingViewModel
import app.getvela.wallet.feature.onboarding.core.CreateStage

/**
 * The create journey, end to end.
 *
 * This composable holds no flow state. It renders whatever view the core last
 * emitted and sends events back — the whole mapping from `CreateView` to a
 * screen is [screenFor], and it is the only place that decides which step is
 * showing (data-model §3).
 */
@Composable
fun CreateFlowScreen(
    model: OnboardingViewModel,
    onExit: () -> Unit,
    onOpenPrivacy: () -> Unit,
    onOpenTerms: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val view = model.createView

    // Start on arrival; do NOT stop on leaving composition.
    //
    // `onDispose { disposeCreate() }` was killing live ceremonies. Leaving
    // composition is not the same event as leaving the flow: an activity
    // relaunch (a USB key enumerating as a HID keyboard does exactly that), a
    // trip to the background, or the system credential UI taking over all pull
    // this screen out of the tree while the person is mid-ceremony — and
    // disposing the driver there cancels the coroutine awaiting the
    // authenticator. The key still mints the credential; nothing is left
    // listening for it (device-found 2026-08-26).
    //
    // The machine belongs to the ViewModel, which outlives the screen, so it is
    // disposed where the flow actually ENDS: the exit affordance and the
    // finished handler, both in the navigation host.
    DisposableEffect(Unit) {
        model.startCreate()
        onDispose { }
    }

    val screen = screenFor(view)
    val statusText = view?.status
        ?.takeIf { progressFor(it) == null }
        ?.let { strings.t(statusKeyToI18n(it)) }

    FlowShell(
        backLabel = strings.t(I18nKeys.Flow.BACK),
        // The one screen with no way back is the one where going back would
        // abandon work already in flight: a ceremony is running and a passkey
        // may already exist in the person's provider.
        canGoBack = screen != Screen.Progress,
        onBack = {
            // The core owns whether there is anywhere to go back TO — only the
            // key list has one. From the name screen it reports none, and back
            // then means leaving the flow, which is the host's because the core
            // has no idea what contains it.
            if (view?.canGoBack == true) model.goBack() else onExit()
        },
    ) {
        when (screen) {
            Screen.Loading -> Column(modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = strings.t(I18nKeys.Flow.CONFIRM_IN_PROMPT),
                    color = VelaTheme.colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }

            Screen.Name -> NameScreen(
                name = view!!.name,
                nameEditable = view.nameEditable,
                nameTooLong = view.nameTooLong,
                acks = view.acks,
                canSubmit = view.canSubmit,
                busy = view.busy,
                submitLabel = strings.t(submitLabelToI18n(view.submitLabel)),
                statusText = statusText,
                showStartOver = view.showStartOver,
                onName = model::nameChanged,
                onToggleAck = model::ackToggled,
                onSubmit = model::submit,
                onStartOver = model::startOver,
                onOpenPrivacy = onOpenPrivacy,
                onOpenTerms = onOpenTerms,
            )

            Screen.Keys -> KeysScreen(
                keys = view!!.keys,
                canAddKey = view.canAddKey,
                canFinish = view.canFinish,
                needsSecondKey = view.needsSecondKey,
                busy = view.busy,
                addMethods = view.addMethods,
                addBlocked = view.addBlocked,
                onAddKey = model::addKey,
                onConfirmKey = model::confirmKey,
                onRemoveKey = model::removeKey,
                onFinish = model::finishKeys,
            )

            Screen.Progress -> ProgressScreen(
                position = progressFor(view!!.status)!!,
                keyCount = view.keys.size,
            )

            Screen.Retry -> RetryScreen(
                detail = view!!.syncErrorDetail,
                busy = view.busy,
                onRetry = model::retryUpload,
                onStartOver = model::startOver,
                onEditEndpoint = model::openEndpointSheet,
            )

            Screen.Done -> DoneScreen(
                address = view!!.address.orEmpty(),
                walletName = view.keys.firstOrNull()?.name ?: view.name,
                keys = view.keys,
                // `Created` + busy IS `Completing`: the core collapses the two
                // stages into one view stage, and this is the seam that tells
                // them apart.
                entering = view.busy,
                onEnter = model::enterWallet,
            )
        }
    }
}

enum class Screen { Loading, Name, Keys, Progress, Retry, Done }

/**
 * Which screen is showing (data-model §3).
 *
 * The core's `stage` decides, with one refinement: a busy machine reporting a
 * progress status has left the key list and is deriving, so the progress screen
 * takes over until it lands. `setting_up_identity` is deliberately NOT a
 * progress status — it happens before the key list exists, and renders as the
 * Name screen's status line.
 */
fun screenFor(view: app.getvela.wallet.feature.onboarding.core.CreateView?): Screen = when {
    view == null -> Screen.Loading
    view.stage == CreateStage.Created -> Screen.Done
    view.stage == CreateStage.SyncFailed -> Screen.Retry
    view.busy && progressFor(view.status) != null -> Screen.Progress
    view.stage == CreateStage.AddKeys -> Screen.Keys
    else -> Screen.Name
}
