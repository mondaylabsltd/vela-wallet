//
//  ClearSignerSheets.swift
//  VelaWallet
//
//  What the Clear Signer puts on screen while the page holds the request
//  (specs 071 and 075).
//
//  ONE sheet. It used to walk through four screens — where is your signer,
//  the pairing code, the six digits, then the wait — because the signer could
//  be on another device. The owner retired those channels on 2026-09-23, so
//  all that is left is the wait, with "open the page again" and the way out.
//  It stays a sheet with a model rather than becoming a view with arguments:
//  presenting a second sheet while a first is dismissing fails silently on
//  iOS, and a session that reopens its page must not risk that.
//
//  The model is driven from `ClearSigner`; nothing here decides anything.
//

import SwiftUI
import VelaCore

/// What the Clear Signer's sheet is showing.
@Observable
final class ClearSignerSheetModel {
    enum Stage: Equatable {
        /// The page has the request.
        case waiting
    }

    var stage: Stage = .waiting
    /// Only while a page is open on this device.
    var reopen: (() -> Void)?
    var cancel: () -> Void = {}
}

struct ClearSignerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    @Bindable var model: ClearSignerSheetModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            waiting
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
