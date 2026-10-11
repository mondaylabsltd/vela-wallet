//
//  SheetFitTests.swift
//  VelaWalletTests
//
//  A sheet opens with its last action in reach (device walk 2026-10-11, the
//  other phone: the identicon sheet opened at half height, 「复制地址」 under
//  the navigation bar and 「关闭」 off-screen until it was dragged up).
//
//  Each of this app's own sheets, PRESENTED — a real `.sheet` over a hosted
//  screen, with the detents its call site gives it — in a window the size of
//  the phone and one the size of the SE (375 × 667), in Chinese and English.
//  Read from the presented controller: where the sheet stands, and where its
//  last action is. The action is whole, on the sheet, above the home
//  indicator's room — without a drag.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SheetFitTests {
    static let sizes = [CGSize(width: 402, height: 874), CGSize(width: 375, height: 667)]
    static let languages = ["zh", "en"]

    /// A screen that presents `sheet` the moment it is up.
    private struct Presenter<Sheet: View>: View {
        @State private var shown = true
        @ViewBuilder let sheet: () -> Sheet
        var body: some View {
            Color.clear.sheet(isPresented: $shown) { sheet() }
        }
    }

    /// The last action `label` of the sheet `content` makes, as presented.
    private func fit<Content: View>(
        _ name: String, last label: String, size: CGSize, @ViewBuilder _ content: @escaping () -> Content
    ) async throws {
        try await fit(name, last: label, size: size, screen: Presenter(sheet: content).themed(.light))
    }

    /// The same, for a `screen` that presents its own sheet when it appears.
    private func fit(_ name: String, last label: String, size: CGSize, screen: some View) async throws {
        let hosted = HostedScreen(screen, size: size)
        defer { hosted.close() }
        let sheet = try #require(
            try await hosted.sheet { $0.labelled(label) != nil },
            "\(name) \(Int(size.width))×\(Int(size.height)): the sheet never came up with 「\(label)」"
        )
        try await hosted.save("sheet-\(name)-\(Int(size.height))")
        let action = try #require(sheet.seen.tree.last { $0.label == label }).frame
        print("MEASURE sheet \(name) \(Int(size.width))x\(Int(size.height)): sheet top \(sheet.frame.minY) of \(size.height), last action 「\(label)」 \(action.minY)…\(action.maxY), floor \(sheet.floor)")
        #expect(action.height >= 40, "\(name): 「\(label)」 is \(action.height) tall")
        #expect(action.minY >= sheet.frame.minY, "\(name): 「\(label)」 is above the sheet")
        #expect(action.maxY <= sheet.floor + 0.5,
                "\(name) \(Int(size.width))×\(Int(size.height)): 「\(label)」 ends at \(action.maxY), under the floor \(sheet.floor) — cut, or behind the home indicator")
    }

    // MARK: - The identicon viewer (the home's avatar, a contact row's)

    @Test(arguments: sizes)
    func theIdenticonViewerShowsItsCloseButton(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            try await fit("identicon-\(language)", last: loc.t("componentsUi.identiconViewer.close"), size: size) {
                IdenticonViewerSheet(
                    loc: loc, address: "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c", onClose: {}
                )
                .presentationDetents([.large])
                .themed(.light)
            }
        }
    }

    // MARK: - A site asks to connect; a connected site's panel (Explore)

    private func connection(asking: Bool, _ loc: Loc) -> ConnectionModel {
        let me = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        let origin = "https://app.uniswap.org"
        let dbr = DbrViewWire(
            ready: true,
            consent: asking
                ? DbrConsentViewWire(tab: "t1", origin: origin, methods: ["eth_requestAccounts"],
                                     address: me, chainId: 100)
                : nil,
            tabs: [DbrTabViewWire(tab: "t1", origin: origin, connectedAddress: asking ? nil : me,
                                  chainId: 100, secure: true, crashed: false)],
            sites: [], signing: nil, queuedSigning: 0
        )
        return ExploreLive.connectionModel(
            dbr: dbr, tab: dbr.tabs.first, engine: nil, identity: (name: "Parallel One", address: me),
            chainIds: [1, 100, 8453], loc: loc
        )
    }

    /// The real Explore screen, opening its connection sheet on `model` —
    /// the drawn board that opens that sheet, with the panel swapped in.
    private func explore(_ model: ConnectionModel, _ loc: Loc) -> some View {
        var home = ExploreFixtures.buildMobileState(.e7, loc: loc)
        home.menus.connection = model
        return ExploreScreen(model: home, loc: loc, signing: SigningFixtures.build(.cs12, loc: loc))
            .themed(.light)
    }

    @Test(arguments: sizes)
    func theConsentSheetShowsItsAnswers(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let model = connection(asking: true, loc)
            try await fit("consent-\(language)", last: try #require(model.consent?.approve), size: size,
                          screen: explore(model, loc))
        }
    }

    @Test(arguments: sizes)
    func theConnectionPanelShowsDisconnect(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let model = connection(asking: false, loc)
            try await fit("connection-\(language)", last: model.disconnect, size: size, screen: explore(model, loc))
        }
    }

    // MARK: - A page from outside; the passkey index; a phone's approval

    @Test(arguments: sizes)
    func theExternalPageSheetShowsCancel(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let page = BrowserController.ExternalPage(
                url: "https://app.uniswap.org/swap?inputCurrency=ETH&outputCurrency=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48&chain=mainnet",
                host: "app.uniswap.org"
            )
            try await fit("external-\(language)", last: loc.t(I18nKeys.ExternalPage.cancel), size: size) {
                ExternalPageSheet(loc: loc, page: page, onAnswer: { _ in }).themed(.light)
            }
        }
    }

    @Test(arguments: sizes)
    func theEndpointSheetShowsReset(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            try await fit("endpoint-\(language)", last: loc.t(I18nKeys.Settings.resetToDefault), size: size) {
                EndpointSheet(loc: loc, defaultURL: "https://index.getvela.app", draft: "https://index.getvela.app",
                              onSave: { _ in }).themed(.light)
            }
        }
    }

    @Test(arguments: sizes)
    func thePhonesApprovalSheetShowsCancel(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let touch = OnboardingModel.UsbTouch(kind: "touch", product: HybridCeremony.hybridProduct)
            try await fit("cable-touch-\(language)", last: loc.t(I18nKeys.Flow.cancel), size: size) {
                UsbTouchSheet(loc: loc, touch: touch, onCancel: {}).themed(.light)
            }
        }
    }

    /// A security key holding more wallets than half a screen has room for:
    /// the list is as tall as it is, and the last wallet is on it.
    @Test(arguments: sizes)
    func theWalletPickerShowsItsLastWallet(size: CGSize) async throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let names = (1...6).map { "Wallet \($0)" }
        let pending = OnboardingModel.PendingWalletPick(
            choices: names.map { CtapCredentialChoice(name: $0, credentialId: $0, product: "YubiKey 5C") },
            answer: { _ in }
        )
        try await fit("wallet-picker", last: names[5], size: size) {
            UsbWalletPickerSheet(loc: loc, pending: pending, onPick: { _ in }).themed(.light)
        }
    }

    @Test(arguments: sizes)
    func theSignOutSheetShowsCancel(size: CGSize) async throws {
        for language in Self.languages {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            try await fit("sign-out-\(language)", last: loc.t("settings.signOut.cancel"), size: size) {
                SignOutSheet(loc: loc, pendingUploadWarning: true, accountCount: 6, onConfirm: {}, onDismiss: {})
                    .themed(.light)
            }
        }
    }
}
