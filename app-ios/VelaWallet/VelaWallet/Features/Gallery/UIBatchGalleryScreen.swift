//
//  UIBatchGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for PR 3 (the UI batch), reached with `VELA_PAGE=pr3` and
//  `VELA_STATE=<board>` — the surfaces no other gallery can put on screen
//  without a ceremony, a second device or a chain that refuses the wallet.
//  Every board is the PRODUCTION view over fixture data:
//
//  - `cable-create` / `cable-signin` (issue #480) — the "Phone or tablet ·
//    scan a code" sheet over a page, as tall as its content, Cancel at the
//    bottom. The same one sheet create, add-key, sign-in and the account
//    switcher's sign-in raise.
//
//  Fixture data only: nothing here is scanned, signed or sent.
//

#if DEBUG

import SwiftUI
import VelaCore

struct UIBatchGalleryScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var scheme
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "cable-signin":
            cableBoard(chooser: .signIn)
        default:
            cableBoard(chooser: .create)
        }
    }

    // MARK: - Issue #480: the scan-a-code sheet

    /// A caBLE payload's shape: `FIDO:/` and the digits the phone scans. Not
    /// a live one — no device answers it.
    private static let cablePayload = "FIDO:/" + String(repeating: "0914372806155923481170265394", count: 6)

    private func cableBoard(chooser: KeyChooser) -> some View {
        // The page the sheet rises over: what shows above a content-sized
        // sheet is the screen the person came from.
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(chooser == .create ? I18nKeys.Create.keysTitle : I18nKeys.Login.header))
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgBase)
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Layout.screenPaddingX)
        .background(theme.bgBase.ignoresSafeArea())
        .sheet(isPresented: .constant(true)) {
            CableQrSheet(loc: loc, payload: Self.cablePayload, chooser: chooser)
                .themed(scheme)
        }
    }
}

#endif
