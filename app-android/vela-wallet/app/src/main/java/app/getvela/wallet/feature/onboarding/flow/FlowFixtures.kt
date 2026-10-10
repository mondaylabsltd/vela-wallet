package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.CreateStage
import app.getvela.wallet.feature.onboarding.core.CreateView
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.core.StatusKey
import app.getvela.wallet.feature.onboarding.core.SubmitLabel

/**
 * Gallery fixtures for the v2 flow.
 *
 * Spec 014's fixtures were `CreatePanelState` / `LoginPanelState` — presentation
 * types this app owned. Those types are gone: the screens now render `CreateView`
 * straight from the core, so a fixture has to be a `CreateView` too. That is the
 * point of rewriting them rather than adapting them — a fixture in a shape the
 * production path cannot produce is a picture of a screen that cannot happen.
 *
 * The same list the desktop client walks (results.md, Phase 5), so a state that
 * looks wrong on one is checkable against the other.
 */

sealed interface Fixture {
    /** A step of the create journey, rendered by the real flow screens. */
    data class Flow(val view: CreateView) : Fixture

    /** The failure sheet, one entry per outcome the catalog names. */
    data class Sheet(val kind: PromptKind, val confirmable: Boolean) : Fixture

    /** The app-owned USB path's PIN keypad — the sheet a large system font
     *  clipped (device-found 2026-08-28); in the gallery so a layout that
     *  hides its own confirm button is visible without a key in hand. */
    data class UsbPin(val retries: Int, val isRetry: Boolean) : Fixture

    /** The "insert your security key" waiter, with and without the OTG hint. */
    data class InsertKey(val otgLooksOff: Boolean) : Fixture

    /**
     * "Phone or tablet · scan a code" (issue #480). No board drew it — it was
     * only ever seen mid-ceremony, on a device — so a sheet too tall for a
     * tablet or a landscape window (#447) had nowhere to be noticed.
     */
    data class CableQr(val chooser: KeyChooser) : Fixture
}

data class StateFixture(val group: String, val code: String, val fixture: Fixture)

object FlowFixtures {

    /** Spec 102: a self-hosted signing page, and the domain its keys belong to. */
    const val OWN_PAGE = "https://sign.example.com/"
    const val OWN_DOMAIN = "sign.example.com"

    /**
     * The chosen page as the boards draw it — a fixture line ("matches … ·
     * checked 14:02"), since a gallery checks nothing.
     */
    fun ownPageItem(view: CreateView, strings: app.getvela.wallet.core.i18n.VelaStrings) =
        view.signingPage?.let { url ->
            app.getvela.wallet.feature.settings.SettingsLive.pageItem(
                url = url,
                name = "",
                domain = view.signingDomain,
                official = false,
                line = uniffi.vela_core_uniffi.SignerIntegrityLine(
                    state = uniffi.vela_core_uniffi.SignerIntegrityState.TRUSTED_HERE,
                    version = "6ffe9ef2",
                    checkedAtMs = FIXTURE_CHECKED_AT.toULong(),
                    key = "componentsUi.signing.integrity.trusted",
                    opens = true,
                ),
                s = strings,
            )
        }

    /** 2026-10-09 14:02 local — a fixed "checked" time for the boards. */
    val FIXTURE_CHECKED_AT: Long = java.util.Calendar.getInstance().apply {
        set(2026, java.util.Calendar.OCTOBER, 9, 14, 2, 0)
    }.timeInMillis

    /** A funded-looking address; the identicon and the strip both derive from it. */
    const val FIXTURE_ADDRESS = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"

    private fun base(): CreateView = CreateView(
        stage = CreateStage.Form,
        name = "",
        nameEditable = true,
        nameTooLong = false,
        acks = listOf(false, false),
        canSubmit = false,
        submitLabel = SubmitLabel.Create,
        showStartOver = false,
        busy = false,
        status = null,
        keys = emptyList(),
        canAddKey = true,
        canFinish = false,
        needsSecondKey = false,
        canGoBack = true,
        address = null,
        syncErrorDetail = null,
    )

    /**
     * A security key is deliberately given NO provider: hardware models live in
     * the FIDO metadata service, not the app's catalog, so the gallery shows
     * both the named case and the degradation on one screen.
     */
    private fun key(
        name: String,
        method: KeyMethod = KeyMethod.Platform,
        confirmed: Boolean = true,
        synced: Boolean = true,
    ): CreateKeyRow {
        val platformKey = method == KeyMethod.Platform || method == KeyMethod.Hybrid
        return CreateKeyRow(
            name = name,
            authenticatorAttachment = if (method == KeyMethod.SecurityKey) {
                "cross-platform"
            } else {
                "platform"
            },
            transports = if (method == KeyMethod.SecurityKey) "usb,nfc" else "internal,hybrid",
            confirmed = confirmed,
            synced = synced,
            aaguid = if (platformKey) "fbfc3007-154e-4ecc-8c0b-6e020557d7bd" else "",
            providerName = if (platformKey) "Apple Passwords" else "",
            method = method,
            kind = method,
        )
    }

    /**
     * The two words the core sends with a keys screen (issue #475), for a
     * hand-built board: the heading by the count (`create_wallet::add_heading_key`)
     * and `methods_pinned` (no key yet, and one may be added). Boards only —
     * the live screen reads both off `CreateView` and decides neither.
     */
    private fun CreateView.withKeysHeading(): CreateView = copy(
        addHeadingKey = when {
            keys.isEmpty() -> I18nKeys.Create.KEY_PLACE_HEADING
            keys.size < MAX_KEYS -> I18nKeys.Create.ADD_METHOD_LABEL
            else -> I18nKeys.Create.KEY_LIMIT_REACHED
        },
        methodsPinned = keys.isEmpty() && canAddKey,
        // The core's `key_count_shown`: from the first key on.
        keyCountShown = keys.isNotEmpty(),
    )

    val all: List<StateFixture> = buildList {
        fun flow(code: String, view: CreateView) = add(StateFixture("Create", code, Fixture.Flow(view.withKeysHeading())))
        fun sheet(code: String, kind: String, detail: String? = null, confirmable: Boolean = false, local: Boolean = false) =
            add(StateFixture("Failures", code, Fixture.Sheet(PromptKind(kind, detail, local = local), confirmable)))

        flow("name · empty", base())
        flow(
            "name · filled",
            base().copy(name = "Everyday wallet", acks = listOf(true, true), canSubmit = true),
        )
        flow(
            "name · too long",
            base().copy(
                name = "A wallet name that will not fit a WebAuthn user handle",
                nameTooLong = true,
            ),
        )
        // A draft waiting for its signature: the name is frozen, the button
        // changed word, and the status line says why — the state spec 014 drew
        // as a modal "verification cancelled" sheet.
        flow(
            "name · draft waiting",
            base().copy(
                name = "Everyday wallet",
                nameEditable = false,
                acks = listOf(true, true),
                canSubmit = true,
                submitLabel = SubmitLabel.FinishVerify,
                showStartOver = true,
                status = StatusKey.VerifyCancelled,
            ),
        )
        flow(
            "keys · one, needs a second",
            base().copy(
                stage = CreateStage.AddKeys,
                keys = listOf(key("Everyday wallet", synced = false)),
                needsSecondKey = true,
            ),
        )
        flow(
            "keys · two, ready",
            base().copy(
                stage = CreateStage.AddKeys,
                keys = listOf(key("Everyday wallet", synced = false), key("Key 2")),
                canFinish = true,
            ),
        )
        flow(
            "keys · unconfirmed row",
            base().copy(
                stage = CreateStage.AddKeys,
                keys = listOf(key("Everyday wallet"), key("Key 2", confirmed = false)),
            ),
        )
        // Spec 102: three places, and — until the first key commits the set
        // to one domain — "Use a trusted signing page". Once a page is chosen the
        // entry IS that page: its domain and its integrity line.
        flow(
            "keys · signing page offered",
            base().copy(stage = CreateStage.AddKeys, canChoosePage = true),
        )
        flow(
            "keys · signing page chosen",
            base().copy(
                stage = CreateStage.AddKeys,
                canChoosePage = true,
                signingDomain = OWN_DOMAIN,
                signingPage = OWN_PAGE,
            ),
        )
        flow(
            "keys · a page's own set",
            base().copy(
                stage = CreateStage.AddKeys,
                keys = listOf(key("Everyday wallet", synced = false)),
                needsSecondKey = true,
                signingDomain = OWN_DOMAIN,
                signingPage = OWN_PAGE,
            ),
        )
        flow(
            "keys · at the cap",
            base().copy(
                stage = CreateStage.AddKeys,
                keys = (1..MAX_KEYS).map { key("Key $it") },
                canAddKey = false,
                canFinish = true,
            ),
        )
        listOf(
            "progress · verify" to StatusKey.VerifyingIdentity,
            "progress · derive" to StatusKey.ComputingAddress,
            "progress · publish" to StatusKey.SyncingKey,
        ).forEach { (code, status) ->
            flow(
                code,
                base().copy(
                    stage = CreateStage.AddKeys,
                    busy = true,
                    status = status,
                    keys = listOf(key("Everyday wallet"), key("Key 2")),
                ),
            )
        }
        flow(
            "retry · publish failed",
            base().copy(
                stage = CreateStage.SyncFailed,
                syncErrorDetail = "Register failed: 503 · p256-index-v2.getvela.app",
                keys = listOf(key("Everyday wallet")),
            ),
        )
        flow(
            "done",
            base().copy(
                stage = CreateStage.Created,
                address = FIXTURE_ADDRESS,
                keys = listOf(key("Everyday wallet"), key("Key 2", synced = false)),
            ),
        )

        sheet("unsupported", "not_supported_create")
        sheet("unsupported · login", "not_supported_login")
        sheet("not discoverable", "not_discoverable")
        sheet("incompatible", "incompatible_create")
        sheet("incompatible · login", "incompatible_login")
        sheet("recover offer", "recover_offer", confirmable = true)
        sheet("recover failed", "recover_failed")
        // Sign-in could not look the passkey up: a free retry, never the rebuild.
        sheet("registry unreachable", "registry_unreachable", confirmable = true)
        sheet("registry unreachable · local", "registry_unreachable", confirmable = true, local = true)
        // The two prompts that carry a detail string are driven THROUGH the
        // refinement rather than around it, so this list is also a check on it:
        // a `create_failed` whose message is empty renders an empty sheet, and
        // that is exactly the bug this row would show.
        sheet(
            "create failed · unknown",
            "create_failed",
            detail = "the credential provider returned no attestation",
        )
        sheet(
            "create failed · network",
            "create_failed",
            detail = "Register failed: failed to connect",
        )
        sheet("create failed · server", "create_failed", detail = "Register failed: 503")
        sheet("create failed · timeout", "create_failed", detail = "Register timed out after 120s")
        sheet(
            "sign-in failed",
            "sign_in_failed",
            detail = "No passkey for getvela.app on this device",
        )

        add(StateFixture("Usb prompts", "usb pin", Fixture.UsbPin(retries = -1, isRetry = false)))
        add(StateFixture("Usb prompts", "usb pin · retry", Fixture.UsbPin(retries = 5, isRetry = true)))
        add(StateFixture("Usb prompts", "insert key", Fixture.InsertKey(otgLooksOff = false)))
        add(StateFixture("Usb prompts", "insert key · otg off", Fixture.InsertKey(otgLooksOff = true)))
        add(StateFixture("Phone or tablet", "scan a code · create", Fixture.CableQr(KeyChooser.Create)))
        add(StateFixture("Phone or tablet", "scan a code · sign in", Fixture.CableQr(KeyChooser.SignIn)))
    }

    /**
     * A code the SIZE of a real hybrid one (`FIDO:/` and ~170 digits) for the
     * scan-a-code boards. It opens no ceremony: its digits are a count, not a
     * handshake.
     */
    val CABLE_PAYLOAD: String = "FIDO:/" + (0 until 168).joinToString("") { ((it * 7 + 3) % 10).toString() }

    fun byCode(code: String): StateFixture? = all.firstOrNull { it.code == code }
}
