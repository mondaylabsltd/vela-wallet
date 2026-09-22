package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.core.StatusKey
import app.getvela.wallet.feature.onboarding.core.SubmitLabel

/**
 * The whole translation surface of the onboarding cores.
 *
 * The Rust machines emit semantic variants (`syncing_key`, `recover_offer`) and
 * never a word of user-facing text — which is what keeps fifteen locales out of
 * the shared core and makes "the copy did not change" a diff a reviewer can read
 * on one screen.
 *
 * Every mapping below is exhaustive over a Kotlin enum, so adding a variant in
 * Rust without adding its copy here is a compile error rather than a blank
 * status line in production. `PromptKind` is the one exception — it arrives as a
 * JSON tag, so its `else` branch is loud instead.
 */

/** The transient status line under the create form. */
fun statusKeyToI18n(status: StatusKey): String = when (status) {
    StatusKey.SettingUpIdentity -> I18nKeys.Create.STATUS_SETTING_UP_IDENTITY
    StatusKey.VerifyingIdentity -> I18nKeys.Create.STATUS_VERIFYING_IDENTITY
    StatusKey.ExtractingKey -> I18nKeys.Create.STATUS_EXTRACTING_KEY
    StatusKey.ComputingAddress -> I18nKeys.Create.STATUS_COMPUTING_ADDRESS
    StatusKey.SyncingKey -> I18nKeys.Create.STATUS_SYNCING_KEY
    StatusKey.SetupCancelled -> I18nKeys.Create.STATUS_SETUP_CANCELLED
    StatusKey.VerifyCancelled -> I18nKeys.Create.STATUS_VERIFY_CANCELLED
}

/** The create form's primary button. */
fun submitLabelToI18n(label: SubmitLabel): String = when (label) {
    SubmitLabel.Create -> I18nKeys.Create.NEXT_BTN
    SubmitLabel.FinishVerify -> I18nKeys.Create.FINISH_VERIFY_BTN
}

/**
 * The progress screen's three task rows.
 *
 * `setting_up_identity` is absent on purpose — it happens before the key list
 * exists, so it belongs to the form's status line rather than to this screen.
 */
val PROGRESS_TASKS: List<String> = listOf(
    I18nKeys.Create.TASK_VERIFY_KEY,
    I18nKeys.Create.TASK_DERIVE_ADDRESS,
    I18nKeys.Create.TASK_WRITE_INDEX,
)

/**
 * How far along, and which row is live.
 *
 * Derived from the stage the core reported, never from elapsed time: a bar that
 * advances on a timer tells the person something the wallet does not know, and
 * the moment they are most owed the truth is while their key set is being
 * frozen. The percentage exists because the design shows one; it is a rendering
 * of the same three-step fact, not a second source of truth.
 */
data class ProgressPosition(val activeTask: Int, val percent: Int)

fun progressFor(status: StatusKey?): ProgressPosition? = when (status) {
    StatusKey.VerifyingIdentity, StatusKey.ExtractingKey -> ProgressPosition(0, 33)
    StatusKey.ComputingAddress -> ProgressPosition(1, 62)
    StatusKey.SyncingKey -> ProgressPosition(2, 100)
    else -> null
}

/**
 * A method's title and caption in the add-key picker.
 *
 * Spec 075: the Clear Signer is the fourth, from the signing corpus rather than
 * the create corpus — it is the same option the signing sheet and Settings
 * offer, so it must read the same in all three places.
 */
fun methodCopy(method: KeyMethod): Pair<String, String> = when (method) {
    KeyMethod.Platform ->
        I18nKeys.Create.METHOD_PLATFORM_TITLE to I18nKeys.Create.METHOD_PLATFORM_BODY
    KeyMethod.Hybrid ->
        I18nKeys.Create.METHOD_HYBRID_TITLE to I18nKeys.Create.METHOD_HYBRID_BODY
    KeyMethod.SecurityKey ->
        I18nKeys.Create.METHOD_SECURITY_KEY_TITLE to I18nKeys.Create.METHOD_SECURITY_KEY_BODY
    KeyMethod.ClearSigner -> CLEAR_SIGNER_TITLE to CLEAR_SIGNER_BODY
}

/** The Clear Signer's own words (`componentsUi.signing.*`), one spelling. */
const val CLEAR_SIGNER_TITLE = "componentsUi.signing.clearSignerTitle"
const val CLEAR_SIGNER_BODY = "componentsUi.signing.clearSignerBody"

/**
 * The provider line under a key's name.
 *
 * Keyed off the METHOD the person chose, deliberately — the alternative is
 * `transports`, which is a comma-joined machine list ("internal,hybrid"). What
 * an authenticator reports about its wire protocols is not a sentence, and a
 * person reading their own key list is owed one. The design draws a richer line
 * still ("Pixel · Google Password Manager"), which needs the AAGUID resolved to
 * a provider name; that lookup is a network call the flow does not make, so this
 * is the honest version of the same fact.
 */
fun providerLineFor(method: KeyMethod): String = when (method) {
    KeyMethod.Platform -> I18nKeys.Create.PROVIDER_PLATFORM
    KeyMethod.Hybrid -> I18nKeys.Create.PROVIDER_GENERIC
    KeyMethod.SecurityKey -> I18nKeys.Create.PROVIDER_SECURITY_KEY
    // The page is what holds it — the row says so rather than naming a vault
    // this device cannot see.
    KeyMethod.ClearSigner -> CLEAR_SIGNER_TITLE
}

/**
 * One entry per notice or question the core can raise.
 *
 * [confirm] is present only for the prompt whose answer changes the flow. Every
 * other prompt has one button, because dismissing it and "answering" it are the
 * same act — and offering a second button would imply a choice that does not
 * exist.
 */
data class PromptCopy(
    val title: String,
    val message: String,
    val confirmLabel: String? = null,
    val cancelLabel: String? = null,
) {
    val confirmable: Boolean get() = confirmLabel != null
}

fun promptCopy(kind: PromptKind, strings: VelaStrings): PromptCopy = when (kind.type) {
    "not_supported_create" -> PromptCopy(
        title = strings.t(I18nKeys.Create.ALERT_NOT_SUPPORTED_TITLE),
        message = strings.t(I18nKeys.Create.ALERT_NOT_SUPPORTED_BODY),
    )
    "not_supported_login" -> PromptCopy(
        title = strings.t(I18nKeys.Login.ALERT_NOT_SUPPORTED_TITLE),
        message = strings.t(I18nKeys.Login.ALERT_NOT_SUPPORTED_BODY),
    )
    "not_discoverable" -> PromptCopy(
        title = strings.t(I18nKeys.Flow.NOT_DISCOVERABLE_TITLE),
        message = strings.t(I18nKeys.Flow.NOT_DISCOVERABLE_BODY),
    )
    "incompatible_create" -> PromptCopy(
        title = strings.t(I18nKeys.Login.ALERT_INCOMPATIBLE_TITLE),
        message = strings.t(I18nKeys.Login.ALERT_INCOMPATIBLE_BODY_CREATE),
    )
    "incompatible_login" -> PromptCopy(
        title = strings.t(I18nKeys.Login.ALERT_INCOMPATIBLE_TITLE),
        message = strings.t(I18nKeys.Login.ALERT_INCOMPATIBLE_BODY),
    )
    // The platform's own words. Opaque by nature — they go straight into the bug
    // report, and inventing friendlier text here would lose the detail that
    // makes the report worth filing.
    "create_failed" -> PromptCopy(
        title = strings.t(I18nKeys.Create.ALERT_ERROR_TITLE),
        message = kind.detail.orEmpty(),
    )
    "recover_offer" -> PromptCopy(
        title = strings.t(I18nKeys.Login.RECOVER_OFFER_TITLE),
        message = strings.t(I18nKeys.Login.RECOVER_OFFER_BODY),
        confirmLabel = strings.t(I18nKeys.Login.RECOVER_CONFIRM),
        cancelLabel = strings.t(I18nKeys.Login.RECOVER_CANCEL),
    )
    "recover_failed" -> PromptCopy(
        title = strings.t(I18nKeys.Login.RECOVER_FAILED_TITLE),
        message = strings.t(I18nKeys.Login.RECOVER_FAILED_BODY),
    )
    "sign_in_failed" -> PromptCopy(
        title = strings.t(I18nKeys.Login.SIGN_IN_FAILED_TITLE),
        message = strings.t(
            I18nKeys.Login.ALERT_SIGN_IN_FAILED_BODY,
            mapOf("message" to kind.detail.orEmpty()),
        ),
    )
    // Cannot happen while the core and this file agree; if a variant was added
    // in Rust and not here, say so loudly rather than showing an empty sheet.
    else -> error("unhandled prompt kind: ${kind.type}")
}
