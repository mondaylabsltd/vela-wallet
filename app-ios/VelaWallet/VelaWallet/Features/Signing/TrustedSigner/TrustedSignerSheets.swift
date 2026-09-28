//
//  TrustedSignerSheets.swift
//  VelaWallet
//
//  What the Trusted Signer puts on screen while the page holds the request
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
//  The model is driven from `TrustedSigner`; nothing here decides anything.
//

import SwiftUI
import VelaCore

/// What the Trusted Signer's sheet is showing.
@Observable
final class TrustedSignerSheetModel {
    enum Stage: Equatable {
        /// The page has the request.
        case waiting
    }

    var stage: Stage = .waiting
    /// Only while a page is open on this device.
    var reopen: (() -> Void)?
    var cancel: () -> Void = {}
    /// Spec 079: the page could not open (the channel's verdict, after a HEAD
    /// to its address). The card says so, and its button is a retry.
    var unreachable = false

    /// What the card says, in the corpus's words.
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
}

struct TrustedSignerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    @Bindable var model: TrustedSignerSheetModel

    private var copy: TrustedSignerSheetModel.Copy {
        TrustedSignerSheetModel.copy(unreachable: model.unreachable, loc: loc)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            waiting
            Spacer(minLength: Tokens.Space.s8)
            if let reopen = model.reopen, model.stage == .waiting {
                VelaButton(title: copy.reopen, kind: copy.reopenPrimary ? .primary : .secondary,
                           action: reopen)
                    .accessibilityIdentifier(model.unreachable ? "trustedSigner.retry" : "trustedSigner.reopen")
            }
            VelaButton(title: copy.cancel, kind: .secondary, action: model.cancel)
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(theme.bgBase.ignoresSafeArea())
    }

    private var waiting: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            HStack(spacing: Tokens.Space.s12) {
                if copy.busy {
                    ProgressView()
                } else {
                    LucideIcon(.triangleAlert, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.warningBase)
                        .accessibilityHidden(true)
                }
                Text(copy.title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier(model.unreachable ? "trustedSigner.signerDown" : "trustedSigner.waiting")
            }
            if let hint = copy.hint {
                Text(hint)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The module matrix as pixels — black on white in both appearances, because
/// a camera reads contrast and an inverted code does not scan. The encoder is
/// the core's, the same one the receive screen and the caBLE QR draw with.
struct TrustedSignerQrView: View {
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
