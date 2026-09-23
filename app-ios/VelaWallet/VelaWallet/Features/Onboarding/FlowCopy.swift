//
//  FlowCopy.swift
//  VelaWallet
//
//  The whole translation surface of the onboarding cores.
//
//  The Rust machines emit semantic variants (`syncingKey`, `recover_offer`) and
//  never a word of user-facing text — which is what keeps fifteen locales out of
//  the shared core and makes "the copy did not change" a diff a reviewer can
//  read on one screen.
//
//  Every mapping below is exhaustive over a Swift enum, so adding a variant in
//  Rust without adding its copy here is a compile error rather than a blank
//  status line in production. `PromptKind` is the one exception — it arrives as
//  a JSON tag, so its default branch is loud instead.
//

import Foundation

/// The transient status line under the create form.
func statusKeyToI18n(_ status: StatusKey) -> String {
    switch status {
    case .settingUpIdentity: I18nKeys.Create.statusSettingUpIdentity
    case .verifyingIdentity: I18nKeys.Create.statusVerifyingIdentity
    case .extractingKey: I18nKeys.Create.statusExtractingKey
    case .computingAddress: I18nKeys.Create.statusComputingAddress
    case .syncingKey: I18nKeys.Create.statusSyncingKey
    case .setupCancelled: I18nKeys.Create.statusSetupCancelled
    case .verifyCancelled: I18nKeys.Create.statusVerifyCancelled
    }
}

/// The create form's primary button.
func submitLabelToI18n(_ label: SubmitLabel) -> String {
    switch label {
    case .create: I18nKeys.Create.nextBtn
    case .finishVerify: I18nKeys.Create.finishVerifyBtn
    }
}

/// The progress screen's three task rows.
///
/// `settingUpIdentity` is absent on purpose — it happens before the key list
/// exists, so it belongs to the form's status line rather than to this screen.
let progressTasks: [String] = [
    I18nKeys.Create.taskVerifyKey,
    I18nKeys.Create.taskDeriveAddress,
    I18nKeys.Create.taskWriteIndex,
]

/// How far along, and which row is live.
///
/// Derived from the stage the core reported, never from elapsed time: a bar that
/// advances on a timer tells the person something the wallet does not know, and
/// the moment they are most owed the truth is while their key set is being
/// frozen.
struct ProgressPosition: Equatable {
    let activeTask: Int
    let percent: Int
}

func progressFor(_ status: StatusKey?) -> ProgressPosition? {
    switch status {
    case .verifyingIdentity, .extractingKey: ProgressPosition(activeTask: 0, percent: 33)
    case .computingAddress: ProgressPosition(activeTask: 1, percent: 62)
    case .syncingKey: ProgressPosition(activeTask: 2, percent: 100)
    default: nil
    }
}

/// A method's title and caption in the add-key picker.
///
/// The Trusted Signer's two lines come from `componentsUi.signing.*` rather
/// than from `onboarding.create.*`: they are the SAME two sentences the
/// signing sheet's "Sign with" shows, and the route is one thing whether a
/// person meets it while creating a wallet or while spending from it (spec
/// 075). Minting a second pair would let the two drift.
func methodCopy(_ method: KeyMethod) -> (title: String, body: String) {
    switch method {
    case .platform: (I18nKeys.Create.methodPlatformTitle, I18nKeys.Create.methodPlatformBody)
    case .hybrid: (I18nKeys.Create.methodHybridTitle, I18nKeys.Create.methodHybridBody)
    case .securityKey: (I18nKeys.Create.methodSecurityKeyTitle, I18nKeys.Create.methodSecurityKeyBody)
    case .trustedSigner: (I18nKeys.TrustedSigner.title, I18nKeys.TrustedSigner.body)
    }
}

/// The provider line under a key's name, when the AAGUID catalog cannot name
/// the vault: WHERE THIS KEY LIVES, in the method picker's own words.
///
/// Takes `key.kind` — what the AUTHENTICATOR reported — never `key.method`, the
/// tap in the picker (issue #207): a hybrid tap answered by a USB key is a
/// security key, and a row that said "Phone or tablet" beside it was wrong.
/// The web's `providerLineFor` (`onboarding/core/copy.ts`).
func providerLineFor(_ kind: KeyMethod) -> String {
    switch kind {
    case .platform: I18nKeys.Create.methodPlatformTitle
    case .hybrid: I18nKeys.Create.methodHybridTitle
    case .securityKey: I18nKeys.Create.providerSecurityKey
    // The row says where the key lives, and a Trusted Signer key lives behind
    // a page — which is what its own title says.
    case .trustedSigner: I18nKeys.TrustedSigner.title
    }
}

/// The one badge a key row wears (issue #207): is this passkey cloud-synced or
/// device-bound? `nil` when nobody can vouch for the answer — an unreadable
/// attestation makes `synced` fail open to `true` for the second-key GATE, and
/// a gate is not a badge. The web's `keyBadge`.
func keyBadge(_ key: CreateKeyRow) -> (text: String, synced: Bool)? {
    guard key.syncedKnown else { return nil }
    return key.synced
        ? (I18nKeys.Create.keySyncedBadge, true)
        : (I18nKeys.Create.keyDeviceOnlyBadge, false)
}

/// One entry per notice or question the core can raise.
///
/// `confirmLabel` is present only for the prompt whose answer changes the flow.
/// Every other prompt has one button, because dismissing it and "answering" it
/// are the same act — and offering a second would imply a choice that does not
/// exist.
struct PromptCopy: Equatable {
    let title: String
    let message: String
    var confirmLabel: String?
    var cancelLabel: String?

    var confirmable: Bool { confirmLabel != nil }
}

func promptCopy(_ kind: PromptKind, loc: Loc) -> PromptCopy {
    switch kind.type {
    case "not_supported_create":
        PromptCopy(
            title: loc.t(I18nKeys.Create.alertNotSupportedTitle),
            message: loc.t(I18nKeys.Create.alertNotSupportedBody)
        )
    case "not_supported_login":
        PromptCopy(
            title: loc.t(I18nKeys.Login.alertNotSupportedTitle),
            message: loc.t(I18nKeys.Login.alertNotSupportedBody)
        )
    case "not_discoverable":
        PromptCopy(
            title: loc.t(I18nKeys.Flow.notDiscoverableTitle),
            message: loc.t(I18nKeys.Flow.notDiscoverableBody)
        )
    case "incompatible_create":
        PromptCopy(
            title: loc.t(I18nKeys.Login.alertIncompatibleTitle),
            message: loc.t(I18nKeys.Login.alertIncompatibleBodyCreate)
        )
    case "incompatible_login":
        PromptCopy(
            title: loc.t(I18nKeys.Login.alertIncompatibleTitle),
            message: loc.t(I18nKeys.Login.alertIncompatibleBody)
        )
    // The platform's own words. Opaque by nature — they go straight into the
    // bug report, and inventing friendlier text here would lose the detail that
    // makes the report worth filing.
    case "create_failed":
        PromptCopy(title: loc.t(I18nKeys.Create.alertErrorTitle), message: kind.detail ?? "")
    case "recover_offer":
        PromptCopy(
            title: loc.t(I18nKeys.Login.recoverOfferTitle),
            message: loc.t(I18nKeys.Login.recoverOfferBody),
            confirmLabel: loc.t(I18nKeys.Login.recoverConfirm),
            cancelLabel: loc.t(I18nKeys.Login.recoverCancel)
        )
    case "recover_failed":
        PromptCopy(
            title: loc.t(I18nKeys.Login.recoverFailedTitle),
            message: loc.t(I18nKeys.Login.recoverFailedBody)
        )
    case "sign_in_failed":
        PromptCopy(
            title: loc.t(I18nKeys.Login.alertSignInFailedTitle),
            message: loc.t(I18nKeys.Login.alertSignInFailedBody, vars: ["message": kind.detail ?? ""])
        )
    default:
        // Cannot happen while the core and this file agree. Rather than showing
        // an empty sheet, name the variant nobody handled — it is a shell bug
        // and belongs in the bug report.
        PromptCopy(
            title: loc.t(I18nKeys.Create.alertErrorTitle),
            message: "unhandled prompt kind: \(kind.type)"
        )
    }
}
