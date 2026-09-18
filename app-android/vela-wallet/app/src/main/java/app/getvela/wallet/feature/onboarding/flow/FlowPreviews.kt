package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.feature.onboarding.core.CreateView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Preview-only translation fake (never shipped): the tooling process cannot
 * load the native engine (WelcomePreviews precedent). Sample copy mirrors
 * locales/en; unknown keys echo their leaf so previews stay legible.
 */
private object PreviewStrings : VelaStrings {
    private val sample = mapOf(
        I18nKeys.Create.HEADER to "Create Wallet",
        I18nKeys.Create.ACCOUNT_NAME_PLACEHOLDER to "Enter a name for your account",
        I18nKeys.Create.NAME_TOO_LONG to
            "This name is too long to fit in a passkey — please shorten it.",
        I18nKeys.Create.ACK0 to
            "My public key and wallet name are written into the on-chain contract.",
        I18nKeys.Create.ACK1 to
            "My private key stays in my device's password manager or security key. " +
            "Vela never sees it.",
        I18nKeys.Create.ACK2 to "I have read and agree to the ",
        I18nKeys.Create.ACK2_PRIVACY_POLICY to "Privacy Policy",
        I18nKeys.Create.ACK2_AND to " and ",
        I18nKeys.Create.ACK2_TERMS to "Terms of Service",
        I18nKeys.Create.ACK2_PERIOD to ".",
        I18nKeys.Create.CREATE_WALLET_BTN to "Create Wallet",
        I18nKeys.Create.STATUS_SETTING_UP_IDENTITY to "Setting up secure identity...",
        I18nKeys.Create.SUCCESS_TITLE to "Your wallet is ready!",
        I18nKeys.Create.SUCCESS_MESSAGE to "Your address works on all {{count}} supported networks.",
        I18nKeys.Create.VERIFY_HINT to
            "Your passkey is verified and your key is synced — you're all set.",
        I18nKeys.Create.ENTER_WALLET_BTN to "Enter Wallet",
        I18nKeys.Create.TECHNICAL_DETAILS to "Technical details",
        I18nKeys.Flow.STEP_COUNTER to "Step {{current}} of {{total}}",
        I18nKeys.Flow.CONFIRM_IN_PROMPT to "Confirm in the system prompt.",
        I18nKeys.Flow.WAITED_SECONDS to "Waited {{seconds}} seconds",
        I18nKeys.Flow.SERVER_TITLE to "Service temporarily unavailable",
        I18nKeys.Flow.SERVER_BODY to
            "The passkey index service is unreachable — both creating and signing in need it.",
        I18nKeys.Flow.RETRY to "Retry",
        I18nKeys.Flow.EDIT_INDEX_ENDPOINT to "Change index service address",
        I18nKeys.Flow.REPORT_ERROR to "Report this error",
        I18nKeys.Flow.COPY_ADDRESS to "Copy address",
        I18nKeys.Flow.COPIED to "Copied",
        I18nKeys.Flow.CLOSE to "Close",
        I18nKeys.Login.HEADER to "Sign In",
        I18nKeys.Login.STATUS_AWAITING_PASSKEY to "Waiting for your passkey",
        I18nKeys.Login.STATUS_AWAITING_PASSKEY_HINT to
            "Confirm with Face ID or your fingerprint in the system prompt.",
        I18nKeys.Login.RECOVER_OFFER_TITLE to "Recover Your Wallet",
        I18nKeys.Login.RECOVER_OFFER_BODY to
            "The key server has no record of this passkey yet. Confirm one more signature " +
            "to rebuild your wallet address on this device.",
        I18nKeys.Login.RECOVER_CONFIRM to "Recover Now",
        I18nKeys.Login.RECOVER_CANCEL to "Not Now",
    )

    override fun t(key: String): String = sample[key] ?: key.substringAfterLast('.')

    override fun t(key: String, vars: Map<String, String>): String =
        vars.entries.fold(t(key)) { acc, (name, value) ->
            acc.replace("{{$name}}", value)
        }
}

@Composable
private fun FlowPreviewSurface(darkTheme: Boolean, content: @Composable ColumnScope.() -> Unit) {
    VelaTheme(darkTheme = darkTheme) {
        CompositionLocalProvider(LocalVelaStrings provides PreviewStrings) {
            // The v2 journey is a full screen on its own background, not a panel
            // on a sheet surface — previewing it on bgRaised would show a
            // contrast pairing the app never renders.
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .background(VelaTheme.colors.bgBase)
                    .padding(horizontal = VelaSizing.screenPaddingX),
                content = content,
            )
        }
    }
}

private fun view(code: String): CreateView {
    val fixture = FlowFixtures.byCode(code)?.fixture
    // A renamed fixture must say which name went missing. The bare cast would
    // fail as an unattributed NPE inside the tooling process, where there is no
    // stack trace worth reading.
    check(fixture is Fixture.Flow) { "no flow fixture named \"$code\"" }
    return fixture.view
}

@Preview(name = "Name - empty (dark)", heightDp = 900)
@Composable
private fun NameEmptyDark() = NamePreview(darkTheme = true, code = "name · empty")

@Preview(name = "Name - empty (light)", heightDp = 900)
@Composable
private fun NameEmptyLight() = NamePreview(darkTheme = false, code = "name · empty")

@Preview(name = "Name - draft waiting", heightDp = 900)
@Composable
private fun NameDraftWaiting() = NamePreview(darkTheme = true, code = "name · draft waiting")

@Preview(name = "Keys - needs a second (dark)", heightDp = 900)
@Composable
private fun KeysBlockedDark() = KeysPreview(darkTheme = true, code = "keys · one, needs a second")

@Preview(name = "Keys - two, ready (light)", heightDp = 900)
@Composable
private fun KeysReadyLight() = KeysPreview(darkTheme = false, code = "keys · two, ready")

@Preview(name = "Progress - derive", heightDp = 900)
@Composable
private fun ProgressDerive() {
    val v = view("progress · derive")
    FlowPreviewSurface(darkTheme = true) {
        ProgressScreen(position = progressFor(v.status)!!, keyCount = v.keys.size)
    }
}

@Preview(name = "Retry - publish failed", heightDp = 900)
@Composable
private fun RetryPreview() {
    val v = view("retry · publish failed")
    FlowPreviewSurface(darkTheme = true) {
        RetryScreen(
            detail = v.syncErrorDetail,
            busy = false,
            onRetry = {},
            onStartOver = {},
            onEditEndpoint = {},
        )
    }
}

@Preview(name = "Done (dark)", heightDp = 900)
@Composable
private fun DonePreviewDark() = DonePreview(darkTheme = true)

@Preview(name = "Done (light)", heightDp = 900)
@Composable
private fun DonePreviewLight() = DonePreview(darkTheme = false)

@Composable
private fun NamePreview(darkTheme: Boolean, code: String) {
    val v = view(code)
    FlowPreviewSurface(darkTheme = darkTheme) {
        NameScreen(
            name = v.name,
            nameEditable = v.nameEditable,
            nameTooLong = v.nameTooLong,
            acks = v.acks,
            canSubmit = v.canSubmit,
            busy = v.busy,
            submitLabel = PreviewStrings.t(submitLabelToI18n(v.submitLabel)),
            statusText = v.status?.let { PreviewStrings.t(statusKeyToI18n(it)) },
            showStartOver = v.showStartOver,
            onName = {},
            onToggleAck = {},
            onSubmit = {},
            onStartOver = {},
            onOpenPrivacy = {},
            onOpenTerms = {},
        )
    }
}

@Composable
private fun KeysPreview(darkTheme: Boolean, code: String) {
    val v = view(code)
    FlowPreviewSurface(darkTheme = darkTheme) {
        KeysScreen(
            keys = v.keys,
            canAddKey = v.canAddKey,
            canFinish = v.canFinish,
            needsSecondKey = v.needsSecondKey,
            busy = v.busy,
            onAddKey = {},
            onConfirmKey = {},
            onRemoveKey = {},
            onFinish = {},
        )
    }
}

@Composable
private fun DonePreview(darkTheme: Boolean) {
    val v = view("done")
    FlowPreviewSurface(darkTheme = darkTheme) {
        DoneScreen(
            address = v.address.orEmpty(),
            walletName = v.keys.first().name,
            keys = v.keys,
            entering = v.busy,
            onEnter = {},
        )
    }
}
