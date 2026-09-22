//
//  ClearSignerSheets.swift
//  VelaWallet
//
//  Everything the Clear Signer puts on screen, in ONE sheet whose content
//  swaps (spec 075).
//
//  One sheet, not four. Presenting a second sheet while a first is dismissing
//  fails silently on iOS — the nesting bug the founder hit in onboarding — and
//  this surface walks through up to four screens in a row: where is your
//  signer, the pairing code to scan, the six digits to compare, and the wait
//  while the page holds the request. A sheet that dismissed between them would
//  drop the ceremony it is hosting.
//
//  The model is driven from `ClearSigner`; nothing here decides anything. In
//  particular the confirm button does not "approve" the request — it says the
//  two screens show the same six digits, which is the one check that catches a
//  stand-in page on the relay, and until it is tapped the wallet has sent
//  nothing.
//

import SwiftUI
import VelaCore

/// What the Clear Signer's sheet is showing.
@Observable
final class ClearSignerSheetModel {
    enum Stage: Equatable {
        /// This device, or another one (`clearSignerWhere`).
        case choosing
        /// The relay could not be reached: the line, and the choice again.
        case relayDown
        /// The QR and the link, while the other device is opened.
        case pairing
        /// The six digits both screens must show, and the confirm button.
        case code(String)
        /// The page has the request.
        case waiting
    }

    var stage: Stage = .choosing
    /// The pairing link — the QR's payload and what "copy link" copies.
    var link: String = ""
    /// `true` for this device.
    var pick: (Bool) -> Void = { _ in }
    var confirmCode: () -> Void = {}
    var copyLink: () -> Void = {}
    /// Only while a page is open on this device.
    var reopen: (() -> Void)?
    var cancel: () -> Void = {}
    /// Set once the link has been copied, so the button can say so.
    var copied = false
}

struct ClearSignerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    @Bindable var model: ClearSignerSheetModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            switch model.stage {
            case .choosing, .relayDown:
                where_
            case .pairing:
                pairing
            case .code(let code):
                codeCheck(code)
            case .waiting:
                waiting
            }
            Spacer(minLength: Tokens.Space.s8)
            if let reopen = model.reopen, model.stage == .waiting {
                VelaButton(title: loc.t("componentsUi.signing.clearSignerReopen"),
                           kind: .secondary, action: reopen)
            }
            VelaButton(title: loc.t("common.cancel"), kind: .secondary, action: model.cancel)
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(theme.bgBase.ignoresSafeArea())
    }

    // MARK: - Where is your Clear Signer?

    private var where_: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(I18nKeys.ClearSigner.whereIsIt))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
            if model.stage == .relayDown {
                Text(loc.t(I18nKeys.ClearSigner.relayDown))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.errorBase)
                    .fixedSize(horizontal: false, vertical: true)
            }
            choice(loc.t(I18nKeys.ClearSigner.thisDevice)) { model.pick(true) }
            choice(loc.t(I18nKeys.ClearSigner.otherDevice)) { model.pick(false) }
        }
    }

    private func choice(_ title: String, action: @escaping () -> Void) -> some View {
        Button {
            VelaHaptic.select.play()
            action()
        } label: {
            HStack {
                Text(title)
                    .typeRole(Typography.rowTitle)
                    .foregroundStyle(theme.fgBase)
                Spacer()
                Image(systemName: "chevron.right").foregroundStyle(theme.fgSubtle)
            }
            .frame(minHeight: Tokens.Layout.hitTarget)
        }
    }

    // MARK: - The pairing code

    private var pairing: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(I18nKeys.ClearSigner.pair))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
            Text(loc.t(I18nKeys.ClearSigner.pairHint))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .fixedSize(horizontal: false, vertical: true)
            // The core's own encoder — `QrCode`'s, the one the receive screen
            // and the caBLE code draw with — and `nil` draws NO code rather
            // than a demo pattern somebody would try to scan.
            if let matrix = qrMatrix(text: model.link) {
                ClearSignerQrView(matrix: matrix)
                    .aspectRatio(1, contentMode: .fit)
                    .frame(maxWidth: 220)
                    .frame(maxWidth: .infinity, alignment: .center)
            }
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(loc.t(I18nKeys.ClearSigner.pairWaiting))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgMuted)
            }
            VelaButton(
                title: loc.t(model.copied ? I18nKeys.Flow.copied : I18nKeys.ClearSigner.copyLink),
                kind: .secondary
            ) {
                model.copyLink()
            }
        }
    }

    private func codeCheck(_ code: String) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(I18nKeys.ClearSigner.code, vars: ["code": code]))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("clear-signer-code")
            VelaButton(title: loc.t(I18nKeys.ClearSigner.codeConfirm), kind: .primary) {
                model.confirmCode()
            }
        }
    }

    private var waiting: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            HStack(spacing: Tokens.Space.s12) {
                ProgressView()
                Text(loc.t("componentsUi.signing.clearSignerWaiting"))
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
            }
            Text(loc.t("componentsUi.signing.clearSignerWaitingHint"))
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// The module matrix as pixels — black on white in both appearances, because
/// a camera reads contrast and an inverted code does not scan. The encoder is
/// the core's, the same one the receive screen and the caBLE QR draw with.
struct ClearSignerQrView: View {
    let matrix: QrMatrix

    var body: some View {
        Canvas { context, size in
            let width = Int(matrix.width)
            let quiet = 2
            let units = CGFloat(width + quiet * 2)
            let cell = min(size.width, size.height) / units
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
            for row in 0..<width {
                for col in 0..<width where matrix.modules[row * width + col] {
                    let rect = CGRect(
                        x: (CGFloat(col) + CGFloat(quiet)) * cell,
                        y: (CGFloat(row) + CGFloat(quiet)) * cell,
                        width: cell.rounded(.up),
                        height: cell.rounded(.up)
                    )
                    context.fill(Path(rect), with: .color(.black))
                }
            }
        }
    }
}
