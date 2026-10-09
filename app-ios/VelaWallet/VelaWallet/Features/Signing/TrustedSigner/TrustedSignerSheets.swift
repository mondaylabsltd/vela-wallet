//
//  TrustedSignerSheets.swift
//  VelaWallet
//
//  What the app puts on screen while a signing page holds the request (specs
//  071, 075, 102).
//
//  ONE sheet, two stages:
//
//  - **hand-off** (spec 102 D4): "Review and sign on your trusted page", the
//    key the person will confirm with, the page's integrity line, and Open —
//    enabled only when the core admitted the bytes this phone fetched. A
//    refusal takes the line's place and nothing opens.
//  - **waiting**: the page has the request. "Open the page again" and the way
//    out; when the page could not open at all (spec 079), that, with a retry.
//
//  It stays a sheet with a model rather than a view with arguments: presenting
//  a second sheet while a first is dismissing fails silently on iOS, and a
//  session that reopens its page must not risk that.
//
//  The model is driven from `TrustedSigner`; nothing here decides anything.
//

import SwiftUI
import VelaCore

/// What the signing page's sheet is showing.
@Observable
final class TrustedSignerSheetModel {
    enum Stage: Equatable {
        /// The card: which page, which key, whether the page may open.
        case handoff
        /// The page has the request.
        case waiting
    }

    var stage: Stage = .handoff
    /// The page's base address.
    var page: String = ""
    /// The key's name or its place's title — "Confirm with {{key}}".
    var keyLabel: String?
    /// The page's integrity line, as the core ruled on it.
    var line: SignerIntegrityLine = SignerPageChecks.checking
    /// The card's Open — set while the card waits for it.
    var open: (() -> Void)?
    /// Check the page again — offered when the check itself could not run.
    var recheck: (() -> Void)?
    /// Only while a page is open on this device.
    var reopen: (() -> Void)?
    var cancel: () -> Void = {}
    /// Spec 079: the page could not open (the channel's verdict, after a HEAD
    /// to its address). The card says so, and its button is a retry.
    var unreachable = false

    /// What the waiting card says, in the corpus's words.
    struct Copy: Equatable {
        let title: String
        /// `nil`: no hint line — a page that could not open needs none.
        let hint: String?
        let reopen: String
        /// The reopen button is the card's primary action (the retry).
        let reopenPrimary: Bool
        let cancel: String
        /// The waiting spinner.
        let busy: Bool
    }

    /// The waiting card's words (Android's `SigningLive.trustedSignerWait`):
    /// waiting for the page, or — the page never opened — "签名页没能打开，
    /// 请检查网络。" with Retry. Cancel either way; the request stays open.
    static func copy(unreachable: Bool, loc: Loc) -> Copy {
        unreachable
            ? Copy(
                title: loc.t("componentsUi.signing.signerDown"), hint: nil,
                reopen: loc.t("connect.browser.retry"), reopenPrimary: true,
                cancel: loc.t("common.cancel"), busy: false
            )
            : Copy(
                title: loc.t("componentsUi.signing.trustedSignerWaiting"),
                hint: loc.t("componentsUi.signing.trustedSignerWaitingHint"),
                reopen: loc.t("componentsUi.signing.trustedSignerReopen"), reopenPrimary: false,
                cancel: loc.t("common.cancel"), busy: true
            )
    }

    /// Whether "try again" belongs on the card: the check itself did not run
    /// to a verdict (a network, a server). A page that was checked and refused
    /// gets no retry — checking the same bytes again says the same thing.
    static func offersRecheck(_ line: SignerIntegrityLine) -> Bool {
        line.state == .couldNotCheck
    }
}

struct TrustedSignerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    @Bindable var model: TrustedSignerSheetModel

    var body: some View {
        Group {
            switch model.stage {
            case .handoff: handoff
            case .waiting: waiting
            }
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(theme.bgBase.ignoresSafeArea())
    }

    // MARK: - Hand-off

    private var handoff: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            HandoffCardView(
                model: HandoffCardModel.build(page: model.page, keyLabel: model.keyLabel, line: model.line, loc: loc)
            )
            Spacer(minLength: Tokens.Space.s8)
            VelaButton(
                title: loc.t("componentsUi.signing.openSigner"),
                kind: .primary,
                enabled: model.line.opens && model.open != nil,
                loading: model.line.state == .checking
            ) { model.open?() }
                .accessibilityIdentifier("handoff.open")
            if TrustedSignerSheetModel.offersRecheck(model.line), let recheck = model.recheck {
                VelaButton(title: loc.t("common.tryAgain"), kind: .secondary, action: recheck)
                    .accessibilityIdentifier("handoff.recheck")
            }
            VelaButton(title: loc.t("common.cancel"), kind: .secondary, action: model.cancel)
                .accessibilityIdentifier("handoff.cancel")
        }
    }

    // MARK: - Waiting

    private var copy: TrustedSignerSheetModel.Copy {
        TrustedSignerSheetModel.copy(unreachable: model.unreachable, loc: loc)
    }

    private var waiting: some View {
        VStack(spacing: Tokens.Space.s16) {
            HandoffCardView(
                model: HandoffCardModel
                    .build(page: model.page, keyLabel: model.keyLabel, line: model.line, loc: loc)
                    .waiting(title: copy.title, hint: copy.hint, down: model.unreachable)
            )
            .accessibilityIdentifier(model.unreachable ? "trustedSigner.signerDown" : "trustedSigner.waiting")
            if copy.busy {
                ProgressView()
                    .padding(.top, Tokens.Space.s8)
                    .accessibilityHidden(true)
            }
            Spacer(minLength: Tokens.Space.s8)
            if let reopen = model.reopen {
                VelaButton(title: copy.reopen, kind: copy.reopenPrimary ? .primary : .secondary,
                           action: reopen)
                    .accessibilityIdentifier(model.unreachable ? "trustedSigner.retry" : "trustedSigner.reopen")
            }
            VelaButton(title: copy.cancel, kind: .secondary, action: model.cancel)
        }
    }
}
