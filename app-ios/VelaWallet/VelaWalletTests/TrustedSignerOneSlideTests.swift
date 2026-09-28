//
//  TrustedSignerOneSlideTests.swift
//  VelaWalletTests
//
//  Spec 079 US7 (the native half): when the account signs on the Trusted
//  Signer's page the sheet offers a button that goes there — the page's slide
//  is the one consent — and a request from this app's browser tells the page
//  the browser saw its origin; the wallet's own requests do not. Hermetic.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct TrustedSignerOneSlideTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// An account that signs on the page gets a button, not a second slide.
    @Test func anAccountThatSignsOnThePageGetsAButtonNotASecondSlide() {
        let request = SigningController.Incoming(
            id: "r1", method: "personal_sign",
            paramsJson: #"["0x48","0x88cCA0EeDbF2C4426110bbFc998F048689266894"]"#,
            origin: "http://127.0.0.1:8137", transportId: "tab-1", chainId: 100
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
        func model() -> SigningModel {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: .empty, guard: .empty, fee: nil, context: context
            )
        }
        #expect(!model().confirmAsButton, "a passkey account slides on the sheet")
        context.trustedSignerRoute = true
        let button = model()
        #expect(button.confirmAsButton)
        #expect(button.confirmButtonLabel == loc.t("componentsUi.signing.openSigner"))
        #expect(button.confirm != nil, "the same approve, drawn as a button")
    }

    /// The spine's own route decides: a key behind a page is the page's.
    @Test func theRouteIsTheSpinesOwn() async {
        let accounts = ScriptedAccounts()
        let relay = RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        #expect(!(await spine.signsOnTrustedSigner(account: "0xabc")), "no route named: the platform sheet")

        accounts.routesJson = #"[{"credential_id":"cred-0","transports":"internal","signer_origin":"https://sign.example"}]"#
        #expect(await spine.signsOnTrustedSigner(account: "0xabc"), "a key behind a page signs there")
    }

    /// A page's request tells the page the browser saw the site; the page
    /// then names it instead of "未知站点". The wallet's own does not.
    @Test func aPagesRequestSaysTheBrowserSawItsOrigin() throws {
        let fixture = TrustedSignerFixture()
        func request(seen: Bool) throws -> String {
            try trustedSignerRequest(
                input: UserOpSpine.trustedSignerInput(
                    asked: UserOpSpine.Asked(
                        method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(fixture.account)"]"#,
                        origin: "https://app.example", seenByBrowser: seen
                    ),
                    chainId: 100, account: fixture.account, accountName: "Mine", keys: fixture.keys, calls: []
                ),
                draft: nil
            )
        }
        let seen = try request(seen: true)
        #expect(seen.contains("vela_browser"), "the core fills context.dapp for a page the browser saw")
        #expect(seen.contains("app.example"))
        #expect(!(try request(seen: false)).contains("vela_browser"), "nothing is claimed by default")
    }
}
