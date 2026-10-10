//
//  FlowFixtures.swift
//  VelaWallet
//
//  Gallery fixtures for the v2 flow.
//
//  Spec 014's fixtures were `CreatePanelState` / `LoginPanelState` —
//  presentation types this app owned. Those types are gone: the screens now
//  render `CreateView` straight from the core, so a fixture has to be a
//  `CreateView` too. That is the point of rewriting them rather than adapting
//  them — a fixture in a shape the production path cannot produce is a picture
//  of a screen that cannot happen.
//
//  The same list the desktop and Android clients walk, so a state that looks
//  wrong on one is checkable against the others.
//

import Foundation

enum Fixture {
    /// A step of the create journey, rendered by the real flow screens.
    case flow(CreateView)
    /// The failure sheet, one entry per outcome the catalog names.
    case sheet(kind: PromptKind, confirmable: Bool)
}

struct StateFixture: Identifiable {
    let group: String
    let code: String
    let fixture: Fixture

    var id: String { code }
}

enum FlowFixtures {

    /// A funded-looking address; the identicon and the strip both derive from it.
    static let fixtureAddress = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"

    static func base(
        stage: CreateStage = .form,
        name: String = "",
        nameEditable: Bool = true,
        nameTooLong: Bool = false,
        acks: [Bool] = [false, false],
        canSubmit: Bool = false,
        submitLabel: SubmitLabel = .create,
        showStartOver: Bool = false,
        busy: Bool = false,
        status: StatusKey? = nil,
        keys: [CreateKeyRow] = [],
        canAddKey: Bool = true,
        canFinish: Bool = false,
        needsSecondKey: Bool = false,
        canGoBack: Bool = true,
        address: String? = nil,
        syncErrorDetail: String? = nil,
        addMethods: [KeyMethod] = KeyMethod.allCases,
        signingDomain: String = "getvela.app",
        signingPage: String? = nil,
        canChoosePage: Bool = false
    ) -> CreateView {
        // What the core sends for this many keys (issue #475, `add_heading_key`
        // and `methods_pinned` in `create_wallet.rs`) — written out, since a
        // drawing has no machine behind it.
        let heading = keys.isEmpty
            ? "onboarding.create.addKeyBtn"
            : keys.count < maxKeys ? "onboarding.create.addMethodLabel" : "onboarding.create.keyLimitReached"
        return CreateView(
            stage: stage,
            name: name,
            nameEditable: nameEditable,
            nameTooLong: nameTooLong,
            acks: acks,
            canSubmit: canSubmit,
            submitLabel: submitLabel,
            showStartOver: showStartOver,
            busy: busy,
            status: status,
            keys: keys,
            canAddKey: canAddKey,
            canFinish: canFinish,
            needsSecondKey: needsSecondKey,
            canGoBack: canGoBack,
            address: address,
            syncErrorDetail: syncErrorDetail,
            addMethods: addMethods,
            signingDomain: signingDomain,
            signingPage: signingPage,
            canChoosePage: canChoosePage,
            addHeadingKey: heading,
            methodsPinned: keys.isEmpty && canAddKey
        )
    }

    /// A security key is deliberately given NO provider: hardware models live
    /// in the FIDO metadata service, not the app's catalog, so the gallery
    /// shows both the named case and the degradation in one screen.
    static func key(
        _ name: String,
        method: KeyMethod = .platform,
        confirmed: Bool = true,
        synced: Bool = true
    ) -> CreateKeyRow {
        let platformKey = method == .platform || method == .hybrid
        return CreateKeyRow(
            name: name,
            authenticatorAttachment: method == .securityKey ? "cross-platform" : "platform",
            transports: method == .securityKey ? "usb,nfc" : "internal,hybrid",
            confirmed: confirmed,
            synced: synced,
            syncedKnown: true,
            aaguid: platformKey ? "fbfc3007-154e-4ecc-8c0b-6e020557d7bd" : "",
            providerName: platformKey ? "Apple Passwords" : "",
            method: method,
            kind: method
        )
    }

    static let all: [StateFixture] = {
        var out: [StateFixture] = []
        func flow(_ code: String, _ view: CreateView) {
            out.append(StateFixture(group: "Create", code: code, fixture: .flow(view)))
        }
        func sheet(
            _ code: String, _ type: String, detail: String? = nil, confirmable: Bool = false,
            local: Bool = false
        ) {
            out.append(StateFixture(
                group: "Failures",
                code: code,
                fixture: .sheet(kind: PromptKind(type: type, detail: detail, local: local), confirmable: confirmable)
            ))
        }

        flow("name · empty", base())
        flow("name · filled", base(name: "Everyday wallet", acks: [true, true], canSubmit: true))
        flow("name · too long", base(
            name: "A wallet name that will not fit a WebAuthn user handle",
            nameTooLong: true
        ))
        // A draft waiting for its signature: the name is frozen, the button
        // changed word, and the status line says why — the state spec 014 drew
        // as a modal "verification cancelled" sheet.
        flow("name · draft waiting", base(
            name: "Everyday wallet",
            nameEditable: false,
            acks: [true, true],
            canSubmit: true,
            submitLabel: .finishVerify,
            showStartOver: true,
            status: .verifyCancelled
        ))
        // Issue #475: no key yet — "Add a passkey" over the three places,
        // open; one key or more — they fold under "+ Add another".
        flow("keys · none", base(
            stage: .addKeys
        ))
        flow("keys · one, needs a second", base(
            stage: .addKeys,
            keys: [key("Everyday wallet", synced: false)],
            needsSecondKey: true
        ))
        flow("keys · two, ready", base(
            stage: .addKeys,
            keys: [key("Everyday wallet", synced: false), key("Key 2")],
            canFinish: true
        ))
        // Spec 102: before the first key a person may choose a trusted
        // signing page; once a self-hosted one is chosen, the screen says
        // which domain the keys will belong to. Three places either way —
        // never a fourth.
        flow("keys · signing page offered", base(
            stage: .addKeys,
            canChoosePage: true
        ))
        flow("keys · on a self-hosted page", base(
            stage: .addKeys,
            signingDomain: "sign.example.com",
            signingPage: "https://sign.example.com/",
            canChoosePage: true
        ))
        flow("keys · unconfirmed row", base(
            stage: .addKeys,
            keys: [key("Everyday wallet"), key("Key 2", confirmed: false)]
        ))
        flow("keys · at the cap", base(
            stage: .addKeys,
            keys: (1...maxKeys).map { key("Key \($0)") },
            canAddKey: false,
            canFinish: true
        ))
        for (code, status) in [
            ("progress · verify", StatusKey.verifyingIdentity),
            ("progress · derive", StatusKey.computingAddress),
            ("progress · publish", StatusKey.syncingKey),
        ] {
            flow(code, base(
                stage: .addKeys,
                busy: true,
                status: status,
                keys: [key("Everyday wallet"), key("Key 2")]
            ))
        }
        flow("retry · publish failed", base(
            stage: .syncFailed,
            keys: [key("Everyday wallet")],
            syncErrorDetail: "Register failed: 503 · p256-index-v2.getvela.app"
        ))
        flow("done", base(
            stage: .created,
            keys: [key("Everyday wallet"), key("Key 2", synced: false)],
            address: fixtureAddress
        ))

        sheet("unsupported", "not_supported_create")
        sheet("unsupported · login", "not_supported_login")
        sheet("not discoverable", "not_discoverable")
        sheet("incompatible", "incompatible_create")
        sheet("incompatible · login", "incompatible_login")
        sheet("recover offer", "recover_offer", confirmable: true)
        // PR 2: the registry could not be asked — a free retry, no new
        // passkey; and the same when nothing left the device.
        sheet("registry unreachable", "registry_unreachable", confirmable: true)
        sheet("registry unreachable · offline", "registry_unreachable", confirmable: true, local: true)
        sheet("recover failed", "recover_failed")
        // The two prompts that carry a detail string are driven THROUGH the
        // refinement rather than around it, so this list is also a check on it:
        // a `create_failed` whose message is empty renders an empty sheet, and
        // that is exactly the bug this row would show.
        sheet("create failed · unknown", "create_failed",
              detail: "the authenticator returned no attestation")
        sheet("create failed · network", "create_failed",
              detail: "Register failed: The Internet connection appears to be offline.")
        sheet("create failed · server", "create_failed", detail: "Register failed: 503")
        sheet("create failed · timeout", "create_failed", detail: "Register timed out after 120s")
        sheet("sign-in failed", "sign_in_failed",
              detail: "No passkey for getvela.app on this device")

        return out
    }()

    static func byCode(_ code: String) -> StateFixture? {
        all.first { $0.code == code }
    }
}
