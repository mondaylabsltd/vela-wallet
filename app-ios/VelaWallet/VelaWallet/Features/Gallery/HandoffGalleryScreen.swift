//
//  HandoffGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for spec 102's hand-off (D4), reached with
//  `VELA_PAGE=handoff` and `VELA_STATE=<board>`:
//
//  - `sheet` / `sheet-checking` / `sheet-refused` / `sheet-ask` — a site's
//    transaction on the signing sheet, for an account whose venue is a page,
//    drawn by the production builder (`SigningLive.model`): the hand-off card
//    in place of a second preview — "Confirm with …" from the plan's key
//    label, the fee and speed the page will sign (`handoffFeeRow`), and the
//    integrity line with its "checked {{time}}" (`signerIntegrityTime`). Open
//    is shut until the page's check admits it; `sheet-ask` is a self-hosted
//    page's own build, with "Trust this version" beside the question;
//  - `sheet-blocked` — the same request refused at sign time because the
//    account cannot sign here (`venue_blocked`), the reason in the person's
//    language;
//  - `card`, `checking`, `refused`, `couldNotCheck`, `ask` — the trusted
//    page's own sheet before anything opens; `ceremony` is a self-hosted
//    page's create, with its own title;
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
        case "sheet", "sheet-checking", "sheet-refused", "sheet-ask", "sheet-blocked":
            SigningSheet(model: signingModel)
        default:
            TrustedSignerSheet(loc: loc, model: sheetModel)
        }
    }

    private static let official = "https://sign.getvela.app/"
    private static let selfHosted = SigningPageFixtures.selfHosted

    /// The board's page: a self-hosted one where the board is about it.
    private var page: String {
        switch state {
        case "ask", "sheet-ask", "ceremony": Self.selfHosted
        default: Self.official
        }
    }

    /// Checked two minutes ago — "checked 14:30" in the person's own format.
    private static var checkedAt: UInt64 { UInt64(Date().timeIntervalSince1970 * 1000) - 120_000 }

    private var line: SignerIntegrityLine {
        switch state {
        case "checking", "sheet-checking": SignerPageChecks.checking
        case "refused", "sheet-refused":
            SignerIntegrityLine(
                state: .mismatch, version: "5f0c2e19", checkedAtMs: Self.checkedAt,
                key: "componentsUi.signing.integrity.mismatch", opens: false
            )
        case "couldNotCheck":
            SignerIntegrityLine(
                state: .couldNotCheck, version: "", checkedAtMs: nil,
                key: "componentsUi.signing.integrity.couldNotCheck", opens: false
            )
        case "ask", "sheet-ask":
            SignerIntegrityLine(
                state: .askToTrust, version: "3f9a1c22", checkedAtMs: Self.checkedAt,
                key: "componentsUi.signing.integrity.askTrust", opens: false
            )
        case "ceremony":
            SignerIntegrityLine(
                state: .trustedHere, version: "3f9a1c22", checkedAtMs: Self.checkedAt,
                key: "componentsUi.signing.integrity.trusted", opens: true
            )
        default: SigningPageFixtures.line(Self.official)
        }
    }

    private var sheetModel: TrustedSignerSheetModel {
        let model = TrustedSignerSheetModel()
        model.page = page
        model.keyLabel = KeyLabelWire(placeKey: "onboarding.create.methodPlatformTitle").text(loc)
        model.line = line
        model.open = {}
        model.recheck = {}
        model.trust = {}
        switch state {
        case "waiting":
            model.stage = .waiting
            model.reopen = {}
        case "down":
            model.stage = .waiting
            model.reopen = {}
            model.unreachable = true
        case "ceremony":
            // A self-hosted page's create: its own title, through the core.
            model.stage = .handoff
            model.title = trustedSignerCeremonyTitleKey(
                operationJson: #"{"type":"register_passkey","method":"platform","page":"\#(Self.selfHosted)","name":"Everyday wallet"}"#
            ).map { loc.t($0) }
        default:
            model.stage = .handoff
        }
        return model
    }

    private var signingModel: SigningModel {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let request = SigningController.Incoming(
            id: "handoff", method: "eth_sendTransaction",
            paramsJson: #"[{"from":"\#(address)","to":"0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913","value":"0x0","data":"0x"}]"#,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 8453
        )
        let blocked = state == "sheet-blocked"
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil,
            error: blocked
                ? SignErrorNoticeWire(
                    kind: .venueBlocked, detail: nil,
                    venueBlock: .pageOnOtherDomain(pageDomain: "getvela.app", domain: "example.com")
                )
                : nil,
            funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: SettingsLive.chainColor(8453), nativeSymbol: "ETH",
            walletName: "Everyday wallet", walletAddress: address
        )
        context.trustedSignerRoute = true
        context.handoffPage = page
        context.handoffKeyLabel = KeyLabelWire(name: "YubiKey 5C", placeKey: "onboarding.create.methodSecurityKeyTitle")
        context.handoffLine = line
        // The fee the sheet settled, through the core's own row.
        context.handoffFee = state == "sheet-checking" ? nil : HandoffFeeModel.of(
            feeJson: HandoffFeeFixtures.feeJson, speedJson: HandoffFeeFixtures.speedJson,
            fee: nil, display: .usd, networks: .builtin, loc: loc
        )
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: .empty, guard: .empty, fee: nil, context: context,
            gate: SignConfirmStateWire(enabled: true, block: nil, key: nil)
        )
    }
}

/// A settled fee — 0.02 USDC on Base at the standard speed — as the fee and
/// speed machines write their views. Data for the boards and the tests.
enum HandoffFeeFixtures {
    /// The coin that pays: USDC on Base, in band.
    private static let usdc =
        #"{"type":"erc20","token":"0x833589fcd6edb6e08f4c7c32d4f71b54bda02913","decimals":6,"amount":"20000","symbol":"USDC"}"#

    static func feeJson(tier: String = "standard", ready: Bool = true, busy: Bool = false) -> String {
        """
        {"busy":\(busy),"failed":null,"fee":{"chain_id":8453,"total_wei":"21000000000000",\
        "max_fee_per_gas":"1000000","network_fee_per_gas":"1000000","relayer_fee_per_gas":"0",\
        "bundler_gas_price":"1000000","in_band_gas_basis":"0","effective_gas_price":null,\
        "max_gas_price":null,"total_gas":"21000","deployed":true,"tier":"\(tier)","quoted":true,\
        "fee_asset":\(usdc),"fee_recipient":null},"stale":false,\
        "fee_token":"0x833589fcd6edb6e08f4c7c32d4f71b54bda02913","options":[],"confirm_fee_ready":\(ready)}
        """
    }

    static func speedJson(tier: String = "standard", single: Bool = false) -> String {
        """
        {"tier":"\(tier)","preferred":"standard","previews":[],"open":false,"picked":false,\
        "free":false,"free_note":false,"single":\(single),"gas_price_line":false,"options":[]}
        """
    }

    static var feeJson: String { feeJson() }
    static var speedJson: String { speedJson() }
}

#endif
