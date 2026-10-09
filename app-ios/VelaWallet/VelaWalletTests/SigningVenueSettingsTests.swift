//
//  SigningVenueSettingsTests.swift
//  VelaWalletTests
//
//  Spec 102 P2-08…P2-10 and D4 on iOS, through the real builders and the real
//  session machine:
//
//  - the account's "Where you review and sign": the core's choices, a page
//    that cannot reach the account's keys drawn disabled WITH the core's
//    reason, the active venue marked, the account's domain said;
//  - choosing a venue writes it to the account record through the session
//    machine — by address — and the plan then follows it; one the core
//    refuses (R1) writes nothing;
//  - Settings → Signing pages: official first, each page's domain and line;
//  - the keys block says the domain of an account on its own domain;
//  - the signing sheet for an account whose venue is a page is the hand-off
//    card, its Open shut until the page's check admits it, and it shows no
//    second preview.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SigningVenueSettingsTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let official = "https://sign.getvela.app/"
    private let own = "https://sign.example.com/"

    private func line(_ url: String) -> SignerIntegrityLine {
        SigningPageFixtures.line(url)
    }

    private var pages: SigningPagesViewWire {
        SigningPagesViewWire(
            pages: SigningPageFixtures.pages, saved: [SigningPageWire(url: own)], loaded: true
        )
    }

    // MARK: - Where you review and sign

    /// A `getvela.app` account in Vela: Vela's own sheet active, the official
    /// page offered with its line, and somebody's own page DISABLED with the
    /// core's sentence naming both domains.
    @Test func theVenueSheetDrawsTheCoresChoicesAndReasons() throws {
        let plan = SigningPlanWire(domain: "getvela.app", venue: .inVela)
        let model = SettingsLive.withSigning(
            plan: plan, pages: pages, line: line, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        let venue = try #require(model.venue)
        #expect(venue.title == "Where you review and sign")
        #expect(venue.domainLine == "Keys on getvela.app")
        #expect(venue.row.subtitle == "Review and sign in Vela")
        let inVela = try #require(venue.inVela)
        #expect(inVela.active && inVela.enabled)
        #expect(inVela.title == "Review and sign in Vela")
        #expect(venue.pagesHeader == "Review and sign on a trusted signing page")
        // D6: the official page by its name; a self-hosted one as such.
        #expect(venue.pages.map(\.title) == ["Vela's official signing page", "Self-hosted · sign.example.com"])
        let officialRow = try #require(venue.pages.first)
        #expect(officialRow.enabled && !officialRow.active)
        #expect(officialRow.line?.state == .matches)
        let ownRow = try #require(venue.pages.last)
        #expect(!ownRow.enabled)
        #expect(ownRow.reason == "This page is on sign.example.com; this account's keys are on getvela.app.")
        #expect(SettingsScreen.overlay(forRow: VenueSettingModel.rowId) == .signingVenue)
    }

    /// An account on its own domain: Vela's sheet cannot reach its keys and
    /// says so; its page is the active venue; the keys block says the domain.
    @Test func anAccountOnItsOwnDomainIsLockedToItsPage() throws {
        let plan = SigningPlanWire(domain: "sign.example.com", venue: .page(url: own))
        var base = SettingsFixtures.build(.st1, loc: loc)
        let key = WalletKeys.Row(
            key: CreateKeyRow(
                name: "Mine", authenticatorAttachment: "platform", transports: "internal", confirmed: true,
                synced: true, syncedKnown: false, aaguid: "", providerName: "", method: .platform, kind: .platform
            ),
            synced: nil, publicKeyHex: "04" + String(repeating: "11", count: 64)
        )
        base = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .device, rows: [key]), backup: nil, on: base, loc: loc
        )
        #expect(base.keys != nil)
        let model = SettingsLive.withSigning(plan: plan, pages: pages, line: line, on: base, loc: loc)
        let venue = try #require(model.venue)
        #expect(venue.inVela?.enabled == false)
        #expect(venue.inVela?.reason == "Vela can't reach keys on sign.example.com.")
        #expect(venue.pages.first { $0.active }?.subtitle == "sign.example.com")
        #expect(venue.pages.first { $0.title == "Vela's official signing page" }?.enabled == false)
        #expect(venue.row.subtitle == "Self-hosted · sign.example.com")
        #expect(model.keys?.domainLine == "Keys on sign.example.com")
    }

    /// Polish 9: the account's own page asks to be trusted — the venue row
    /// carries "Trust this version"'s version; a refused row never does.
    @Test func aVenueRowThatAsksToBeTrustedCarriesItsAnswer() throws {
        let plan = SigningPlanWire(domain: "sign.example.com", venue: .page(url: own))
        let model = SettingsLive.withSigning(
            plan: plan, pages: pages, line: SigningPageFixtures.askingLine,
            asksTrust: SigningPageFixtures.asksTrust,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        let venue = try #require(model.venue)
        let ownRow = try #require(venue.pages.first { $0.venue == .page(url: own) })
        #expect(ownRow.active && ownRow.enabled)
        #expect(ownRow.line?.state == .askToTrust)
        #expect(ownRow.trustVersion == SigningPageFixtures.askingVersion)
        #expect(venue.pages.filter { $0.venue != .page(url: own) }.allSatisfy { $0.trustVersion == nil })
    }

    /// No plan read yet, or no account: no row — never a guessed one.
    @Test func noPlanNoRow() {
        let model = SettingsLive.withSigning(
            plan: nil, pages: pages, line: line, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        #expect(model.venue == nil)
    }

    // MARK: - Settings → Signing pages

    @Test func signingPagesListOfficialFirstWithDomainsAndLines() throws {
        let model = SettingsLive.withSigning(
            plan: nil, pages: pages, line: line, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
        let panel = try #require(model.signingPages)
        #expect(panel.title == "Signing pages")
        #expect(panel.subtitle == "Pages you trust to show and sign requests.")
        #expect(panel.rows.map(\.official) == [true, false])
        // Polish 3: "Keys on …" only where it is news — the official page's
        // keys are not on its own host; a self-hosted page's are.
        #expect(panel.rows.map(\.domainLine) == ["Keys on getvela.app", nil])
        #expect(panel.rows.first?.line.state == .matches)
        #expect(panel.rows.map(\.title) == ["Vela's official signing page", "Self-hosted · sign.example.com"])
        let row = try #require(SettingsFixtures.build(.st1, loc: loc).sections.flatMap(\.rows)
            .first { $0.id == SigningPagesPageModel.rowId })
        #expect(row.title == "Signing pages")
    }

    /// Core round 10: the page's own words — never another screen's — and
    /// "Trust this version" offered on a self-hosted page whose check asks,
    /// never on the official page; a page the person named keeps its name.
    @Test func signingPagesSpeakTheirOwnWordsAndAskToTrust() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let version = String(repeating: "3f", count: 32)
        let named = SigningPagesViewWire(
            pages: [
                SigningPageFixtures.pages[0],
                SigningPageRowWire(url: own, name: "", domain: "sign.example.com", official: false),
                SigningPageRowWire(url: "https://sign.work.example/", name: "Work", domain: "sign.work.example",
                                   official: false, trusted: [version]),
            ],
            saved: [SigningPageWire(url: own), SigningPageWire(url: "https://sign.work.example/", name: "Work", trusted: [version])],
            loaded: true
        )
        let panel = SettingsLive.signingPagesPage(
            named, line: line, asksTrust: { _ in version }, loc: zh
        )
        #expect(panel.rename == "重命名" && panel.remove == "移除" && panel.trust == "信任这个版本")
        #expect(panel.nameLabel == "名称")
        #expect(panel.addLabel == "添加自己部署的签名页")
        #expect(panel.rows.map(\.title) == ["Vela 官方签名页", "自己部署的签名页 · sign.example.com", "Work"])
        #expect(panel.rows.first?.trustVersion == nil, "the official page was offered a trust")
        #expect(panel.rows.dropFirst().allSatisfy { $0.trustVersion == version })
        for row in panel.rows {
            #expect(!row.title.contains("我自己的签名页"))
        }
    }

    // MARK: - The session writes the choice

    /// Through the REAL session machine over a real store: the venue is
    /// written to the account record by address and the plan follows it; a
    /// venue the core refuses (R1) changes nothing.
    @Test func choosingAVenueWritesItToTheAccountAndARefusedOneWritesNothing() async throws {
        let defaults = UserDefaults(suiteName: "vela.tests.venue.\(UUID().uuidString)")!
        let store = AccountStore(defaults: defaults)
        let publicKey = "04" + String(repeating: "11", count: 64)
        await store.saveAccount([
            "id": "cred-1", "name": "Mine", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "public_key_hex": publicKey,
            "created_at_iso": "2026-09-01T00:00:00.000Z",
            "keys": [["credential_id": "cred-1", "public_key_hex": publicKey, "name": "Mine", "transports": "internal"]],
            "sign_in_key": ["credential_id": "cred-1", "method": "platform"],
        ])
        let session = SessionController(store: store, registryTransport: IndexScript.unreachable)
        session.boot()
        await Wait.until({ !session.view.loading }, orIdle: { session.isIdle })
        #expect(session.view.hasWallet)
        // The session derives the account's address from its key set; the
        // choice goes by THAT address (invariant ⑨).
        let address = session.view.address
        let port = SendAccountPort(accounts: store)
        #expect(await storedPlan(store)?.venue == .inVela)

        session.chooseSigningVenue(address: address, venue: .page(url: official))
        await Wait.until({ await plan(port, address)?.venue == .page(url: official) }, orIdle: { session.isIdle })
        #expect(await plan(port, address)?.venue == .page(url: official), "the choice was not stored")
        #expect(await plan(port, address)?.key?.credentialId == "cred-1", "the key changed with the venue")

        // R1: a page on another domain cannot reach these keys — refused.
        session.chooseSigningVenue(address: address, venue: .page(url: own))
        await Wait.until({ session.isIdle })
        #expect(await plan(port, address)?.venue == .page(url: official))
    }

    /// The plan of the one stored record, whatever address it was written
    /// under (the fixture's, before the session wrote the derived one).
    private func storedPlan(_ store: AccountStore) async -> SigningPlanWire? {
        guard let record = await store.loadAccounts().first,
              let data = try? JSONSerialization.data(withJSONObject: record)
        else { return nil }
        return SigningPlanWire.of(accountJson: String(decoding: data, as: UTF8.self))
    }

    private func plan(_ port: SendAccountPort, _ address: String) async -> SigningPlanWire? {
        await port.accountJson(of: address).flatMap(SigningPlanWire.of(accountJson:))
    }

    // MARK: - D4: the signing sheet hands off

    private func signingModel(
        line: SignerIntegrityLine?, page: String?, gateOpen: Bool = true, fee: FeeViewWire? = nil
    ) -> SigningModel {
        let request = SigningController.Incoming(
            id: "r1", method: "personal_sign",
            paramsJson: #"["0x48","0x88cCA0EeDbF2C4426110bbFc998F048689266894"]"#,
            origin: "https://app.example", transportId: "tab-1", chainId: 100
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
        context.trustedSignerRoute = page != nil
        context.handoffPage = page
        context.handoffKeyLabel = KeyLabelWire(placeKey: "onboarding.create.methodPlatformTitle")
        context.handoffLine = line
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: .empty, guard: .empty, fee: fee, context: context,
            gate: SignConfirmStateWire(enabled: gateOpen, block: nil, key: nil)
        )
    }

    @Test func aPageVenueGetsTheHandoffCardNotASecondPreview() throws {
        let model = signingModel(line: line(official), page: official)
        let card = try #require(model.handoff)
        #expect(card.title == "Review and sign on a trusted signing page")
        #expect(card.page == "sign.getvela.app")
        #expect(card.pageName == "Vela's official signing page")
        #expect(card.key == HandoffKeyRow(label: "Confirm with", value: "This device"))
        #expect(card.opens)
        #expect(model.confirm?.enabled == true)
        #expect(model.confirmAsButton)
        #expect(model.confirmButtonLabel == "Continue to signing page")
        #expect(model.handoffBlocks.allSatisfy {
            if case .warning = $0 { return true }
            return false
        }, "the card repeats the preview")
    }

    /// Open stays shut until the page's check admits it — checking, refused
    /// or failed alike — and while the core's own gate is shut.
    @Test func openIsShutUntilThePageIsAdmitted() throws {
        for state: (SignerIntegrityState, String) in [
            (.checking, "checking"), (.mismatch, "mismatch"), (.couldNotCheck, "couldNotCheck"),
        ] {
            let refused = SignerIntegrityLine(
                state: state.0, version: "", checkedAtMs: nil,
                key: "componentsUi.signing.integrity.\(state.1)", opens: false
            )
            let model = signingModel(line: refused, page: official)
            #expect(model.handoff?.opens == false)
            #expect(model.confirm?.enabled == false, "\(state.0) opened")
        }
        #expect(signingModel(line: line(official), page: official, gateOpen: false).confirm?.enabled == false)
    }

    /// Polish 5/6: the fee is said once. The sheet keeps its own fee row (its
    /// figure, speed and coin — chosen before the hand-off, D-18) and the
    /// card draws none.
    @Test func theSheetKeepsItsFeeRowAndTheCardDrawsNone() throws {
        let feeView = try #require(HandoffFeeFixtures.feeView)
        let model = signingModel(line: line(official), page: official, fee: feeView)
        #expect(model.handoff != nil)
        #expect(model.handoff?.fee == nil, "the card said the fee a second time")
        guard case .onchain(_, let value, _, _, _)? = model.fee else {
            Issue.record("the sheet lost its fee row in the hand-off: \(String(describing: model.fee))")
            return
        }
        #expect(value.hasPrefix("~"), "\(value)")
        #expect(value.contains("USDC"), "\(value)")
    }

    /// Core round 7: a sign-time venue refusal is said in the person's
    /// language from the core's notice — never an English sentence, never a
    /// retry.
    @Test func aVenueRefusalSaysWhyInThePersonsLanguage() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let block = VenueBlockWire.pageOnOtherDomain(pageDomain: "getvela.app", domain: "example.com")
        let notice = try CoreJSON.decoder.decode(
            SignErrorNoticeWire.self,
            from: Data(#"{"kind":"venue_blocked","detail":null,"venue_block":{"type":"page_on_other_domain","page_domain":"getvela.app","domain":"example.com"}}"#.utf8)
        )
        #expect(notice.kind == .venueBlocked && notice.venueBlock == block)
        #expect(notice.venueReason(zh) == "这个页面在 getvela.app，这个账户的钥匙在 example.com。")
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: notice, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        let blocks = SigningLive.statusBlocks(sign: sign, loc: zh)
        #expect(blocks.contains { if case .warning(_, let text) = $0 { return text == block.text(zh) }; return false })
        // The wire the executors send, read back by the core's own JSON.
        #expect(CoreJSON.string(SignExecutor.venueBlocked(block)).contains(#""type":"venue_blocked""#))
        #expect(VenueBlockWire.notOnWeb.text(zh) == "签名页只能从 Vela 应用打开，网页版不支持。")
    }

    /// In Vela: the sheet as it always was — no card.
    @Test func inVelaTheSheetIsUnchanged() {
        let model = signingModel(line: nil, page: nil)
        #expect(model.handoff == nil)
        #expect(!model.confirmAsButton)
        #expect(model.confirm?.enabled == true)
    }
}
