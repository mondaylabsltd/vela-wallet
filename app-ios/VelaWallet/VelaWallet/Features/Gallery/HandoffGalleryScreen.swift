//
//  HandoffGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for spec 102's hand-off (D4), reached with
//  `VELA_PAGE=handoff` and `VELA_STATE=<board>`:
//
//  - `sheet` / `sheet-checking` / `sheet-refused` — a site's request on the
//    signing sheet, for an account whose venue is the official page, drawn by
//    the production builder (`SigningLive.model`): the hand-off card in place
//    of a second preview, Open shut until the page's check admits it;
//  - `card`, `checking`, `refused`, `couldNotCheck` — the trusted page's own
//    sheet before anything opens (a custom-domain ceremony shows this);
//  - `waiting`, `down` — the page has the request, or could not open.
//
//  Fixture data only: nothing here is checked, signed or opened.
//

#if DEBUG

import SwiftUI
import VelaCore

struct HandoffGalleryScreen: View {
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "sheet", "sheet-checking", "sheet-refused":
            SigningSheet(model: signingModel)
        default:
            TrustedSignerSheet(loc: loc, model: sheetModel)
        }
    }

    private static let official = "https://sign.getvela.app/"

    /// A message as `personal_sign` carries it.
    private static func hexOf(_ text: String) -> String {
        "0x" + Data(text.utf8).map { String(format: "%02x", $0) }.joined()
    }

    private var line: SignerIntegrityLine {
        switch state {
        case "checking", "sheet-checking": SignerPageChecks.checking
        case "refused", "sheet-refused":
            SignerIntegrityLine(
                state: .mismatch, version: "5f0c2e19", checkedAtMs: nil,
                key: "componentsUi.signing.integrity.mismatch", opens: false
            )
        case "couldNotCheck":
            SignerIntegrityLine(
                state: .couldNotCheck, version: "", checkedAtMs: nil,
                key: "componentsUi.signing.integrity.couldNotCheck", opens: false
            )
        default: SigningPageFixtures.line(Self.official)
        }
    }

    private var sheetModel: TrustedSignerSheetModel {
        let model = TrustedSignerSheetModel()
        model.page = Self.official
        model.keyLabel = TrustedSigner.keyLabel(.platform, loc: loc)
        model.line = line
        model.open = {}
        model.recheck = {}
        switch state {
        case "waiting":
            model.stage = .waiting
            model.reopen = {}
        case "down":
            model.stage = .waiting
            model.reopen = {}
            model.unreachable = true
        default:
            model.stage = .handoff
        }
        return model
    }

    private var signingModel: SigningModel {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let request = SigningController.Incoming(
            id: "handoff", method: "personal_sign",
            paramsJson: String(
                decoding: (try? JSONSerialization.data(withJSONObject: [Self.hexOf("Hello, Vela"), address])) ?? Data(),
                as: UTF8.self
            ),
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 100
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: SettingsLive.chainColor(100), nativeSymbol: "XDAI",
            walletName: "Everyday wallet", walletAddress: address
        )
        context.trustedSignerRoute = true
        context.handoffPage = Self.official
        context.handoffPlace = .platform
        context.handoffLine = line
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: .empty, guard: .empty, fee: nil, context: context,
            gate: SignConfirmStateWire(enabled: true, block: nil, key: nil)
        )
    }
}

#endif
