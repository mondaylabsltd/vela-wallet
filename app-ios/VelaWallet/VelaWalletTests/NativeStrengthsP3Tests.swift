//
//  NativeStrengthsP3Tests.swift
//  VelaWalletTests
//
//  Native parity Phase 3 on iOS, branch by branch: the doors from outside
//  (quick actions, widgets' buttons, Shortcuts), the links the app claims
//  and the core reads, the widget snapshot the app writes and the extension
//  only reads, the live scanner's choice, and the haptic vocabulary.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

// MARK: - The doors

@MainActor
struct VelaEntryTests {

    @Test func eachDoorIsItsOwnLinkAndReadsBack() {
        for entry in VelaEntry.allCases {
            #expect(entry.url.absoluteString == "velawallet://\(entry.rawValue)")
            #expect(VelaEntry(url: entry.url) == entry)
            #expect(VelaEntry(shortcutType: entry.shortcutType) == entry)
        }
    }

    /// A door carries nothing: a link that tries to carry something is not
    /// a door (and `velawallet://pay?…` stays the payment link's).
    @Test func aDoorThatCarriesSomethingIsNotADoor() {
        for raw in [
            "velawallet://send?to=0x0000000000000000000000000000000000000001",
            "velawallet://send/0x01",
            "velawallet://scan#x",
            "velawallet://pay?to=0x01",
            "velawallet://settings",
            "https://wallet.getvela.app/send",
            "other://send",
        ] {
            #expect(VelaEntry(url: URL(string: raw)!) == nil, "\(raw) read as a door")
        }
        #expect(VelaEntry(shortcutType: "app.getvela.VelaWallet.entry.pay") == nil)
    }

    /// The doors are named in the dock's own words, in every language.
    @Test @MainActor func theDoorsAreNamedFromTheCorpus() {
        for tag in ["en", "zh", "ja", "de"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            for entry in VelaEntry.allCases {
                let title = loc.t(entry.titleKey)
                #expect(title != entry.titleKey && !title.isEmpty, "\(entry) has no \(tag) name")
            }
        }
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(zh.t(VelaEntry.send.titleKey) == "转账")
    }
}

@MainActor
struct AppEntryInboxTests {

    @Test func aDoorWaitsOnceAndTheLastOneAskedWins() {
        let inbox = AppEntryInbox()
        #expect(inbox.take() == nil)
        inbox.deliver(.send)
        inbox.deliver(.scan)
        #expect(inbox.pending == .scan)
        #expect(inbox.take() == .scan)
        #expect(inbox.take() == nil)
    }

    /// A door opens only into a signed-in wallet, after the launch, and
    /// never over a signature being asked for or a drawn board.
    @Test func aDoorOpensOnlyWhenTheWalletCanOpenIt() {
        #expect(AppEntryInbox.canOpen(signedIn: true, launching: false, signing: false, pageOverride: false))
        #expect(!AppEntryInbox.canOpen(signedIn: false, launching: false, signing: false, pageOverride: false))
        #expect(!AppEntryInbox.canOpen(signedIn: true, launching: true, signing: false, pageOverride: false))
        #expect(!AppEntryInbox.canOpen(signedIn: true, launching: false, signing: true, pageOverride: false))
        #expect(!AppEntryInbox.canOpen(signedIn: true, launching: false, signing: false, pageOverride: true))
    }

    /// No wallet, no quick actions: "Send" on the icon of a phone with no
    /// account would be a door to nowhere.
    @Test func quickActionsAreTheThreeDoorsWhileSignedIn() {
        #expect(QuickActions.items(signedIn: false, title: { $0.rawValue }).isEmpty)
        let items = QuickActions.items(signedIn: true, title: { "T-\($0.rawValue)" })
        #expect(items.map(\.type) == VelaEntry.allCases.map(\.shortcutType))
        #expect(items.map(\.localizedTitle) == ["T-send", "T-receive", "T-scan"])
    }

    @Test func aQuickActionLandsInTheInbox() {
        let inbox = AppEntryInbox()
        let item = UIApplicationShortcutItem(type: VelaEntry.receive.shortcutType, localizedTitle: "x")
        #expect(QuickActions.handle(item, inbox: inbox))
        #expect(inbox.take() == .receive)
        let foreign = UIApplicationShortcutItem(type: "com.example.other", localizedTitle: "x")
        #expect(!QuickActions.handle(foreign, inbox: inbox))
        #expect(inbox.pending == nil)
    }

    /// The intents open the app at their door, through the same inbox.
    @Test func theIntentsDeliverTheirDoor() async throws {
        _ = AppEntryInbox.shared.take()
        _ = try await OpenSendIntent().perform()
        #expect(AppEntryInbox.shared.take() == .send)
        _ = try await OpenReceiveIntent().perform()
        #expect(AppEntryInbox.shared.take() == .receive)
        _ = try await OpenScanIntent().perform()
        #expect(AppEntryInbox.shared.take() == .scan)
        #expect(OpenSendIntent.openAppWhenRun && OpenReceiveIntent.openAppWhenRun && OpenScanIntent.openAppWhenRun)
    }
}

// MARK: - The links

@MainActor
struct AppLinkTests {

    private let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
    private let member = "88cca0eedbf2c4426110bbfc998f048689266894"

    private func read(_ raw: String) -> AppLink? { AppLink.read(URL(string: raw)!) }

    @Test func aDoorAndAPaymentReadAsBefore() {
        #expect(read("velawallet://scan") == .entry(.scan))
        guard case .pay(.pay(let to, let chain, _, _, _, _, _))? =
            read("https://wallet.getvela.app/pay?to=\(safe)&chain=100")
        else { Issue.record("a payment link did not read"); return }
        #expect(to == safe && chain == "100")
        #expect(read("velawallet://open?url=https://app.example") == .pay(.open(url: "https://app.example")))
    }

    /// The proposal link, read by the core: the network, the multisig and
    /// the transaction it names — nothing else.
    @Test func aProposalLinkReadsThroughTheCore() {
        let raw = "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)"
        guard case .proposal(let tx, let url)? = read(raw) else { Issue.record("no proposal"); return }
        #expect(tx == MultisigProposalRef(chainId: 100, multisig: safe, safeTxHash: hash))
        #expect(url.absoluteString == raw)
        // A chat app that encoded the `=` (r4b-17) does not change what it says.
        guard case .proposal(let encoded, _)? = read("https://wallet.getvela.app/zh-TW/wallet#ms%3D100.\(safe).\(hash)")
        else { Issue.record("an encoded proposal did not read"); return }
        #expect(encoded == tx)
    }

    @Test func anInviteNamesItsNetworksAndMembers() {
        let raw = "https://wallet.getvela.app/en/wallet#msa=100,8453:\(safe)&a=\(member)pAlice"
        guard case .invite(let invite, _)? = read(raw) else { Issue.record("no invite"); return }
        #expect(invite.multisig == safe)
        #expect(invite.chainIds == [100, 8453])
        #expect(invite.members?.first?.name == "Alice")
        #expect(invite.members?.first?.kind == "passkey")
        #expect(invite.members?.first?.address?.lowercased() == "0x\(member)")
        // Opened on the site's front page before onboarding (r4b-17).
        guard case .invite? = read("https://wallet.getvela.app/en#msa=\(safe)") else {
            Issue.record("a front-page invite did not read"); return
        }
    }

    /// A link meant as one and not reading as one is SAID (the web's ms-22).
    @Test func aDamagedLinkIsSaidAndSaysWhy() {
        if case .broken(.proposal, _)? = read("https://wallet.getvela.app/en/wallet#ms=100.\(safe)") {} else {
            Issue.record("a cut proposal link was not said")
        }
        if case .broken(.invite, _)? = read("https://wallet.getvela.app/en/wallet#msa=100:0x12") {} else {
            Issue.record("a damaged invite was not said")
        }
        // The address's letters changed on the way: the checksum refuses it.
        let swapped = "0x0Cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
        if case .broken(.inviteChecksum, _)? = read("https://wallet.getvela.app/en/wallet#msa=\(swapped)") {} else {
            Issue.record("a changed checksum was not said")
        }
    }

    @Test func theConnectPageReadsItsTransaction() {
        let raw = "https://wallet.getvela.app/en/connect#for=0x\(member)&ms=100.\(safe.lowercased()).\(hash)"
        guard case .connect(let connect, _)? = read(raw) else { Issue.record("no connect"); return }
        #expect(connect.members == ["0x\(member)"])
        #expect(connect.tx?.chainId == 100)
        #expect(!connect.damaged)
        if case .broken(.connect, _)? = read("https://wallet.getvela.app/en/connect#wss%3A%2F%2Frelay.example%23short") {} else {
            Issue.record("a damaged room was not said")
        }
    }

    /// Everything else is ignored in silence — another host, another page,
    /// the web wallet itself with no multisig link.
    @Test func everythingElseIsNotOurs() {
        for raw in [
            "https://evil.example/en/wallet#ms=100.\(safe).\(hash)",
            "http://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)",
            "https://wallet.getvela.app/en/wallet",
            "https://wallet.getvela.app/en/wallet#tab=history",
            "https://wallet.getvela.app/en/settings#ms=100.\(safe).\(hash)",
            "https://wallet.getvela.app/english/wallet#ms=100.\(safe).\(hash)",
            "https://getvela.app/download",
        ] {
            #expect(read(raw) == nil, "\(raw) was claimed")
        }
    }

    /// Every multisig reading goes through the core — the shell splits no
    /// fragment of its own.
    @Test func everyMultisigReadingIsTheCores() {
        var methods: [String] = []
        let counting: (String, String) throws -> String = { method, args in
            methods.append(method)
            return try multisigLinkCall(method: method, argsJson: args)
        }
        _ = AppLink.read(URL(string: "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)")!, call: counting)
        #expect(methods == ["isMultisigLink", "parseProposalLink"])
        methods = []
        _ = AppLink.read(URL(string: "https://wallet.getvela.app/en/connect#for=0x\(member)")!, call: counting)
        #expect(methods == ["readConnectHash"])
        // A core that cannot answer claims nothing.
        let dead: (String, String) throws -> String = { _, _ in throw NSError(domain: "dead", code: 1) }
        #expect(AppLink.read(URL(string: "https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)")!, call: dead) == nil)
    }

    @Test func aLocaleSegmentIsTheWebsShape() {
        #expect(AppLink.Place(path: "/en/wallet") == .wallet)
        #expect(AppLink.Place(path: "/zh-TW/wallet/") == .wallet)
        #expect(AppLink.Place(path: "/es-MX") == .localeRoot)
        #expect(AppLink.Place(path: "/pt-BR/connect") == .connect)
        #expect(AppLink.Place(path: "/wallet") == .elsewhere)
        #expect(AppLink.Place(path: "/en/wallet/x") == .elsewhere)
        #expect(AppLink.Place(path: "/e1/wallet") == .elsewhere)
    }
}

/// The AASA the owner hosts claims exactly the links the app reads: a path
/// claimed but not read would open the app on nothing, and one read but not
/// claimed would never reach it.
@MainActor
struct AppSiteAssociationTests {

    private static let dir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("universal-links")

    private struct Component {
        let path: String
        let fragment: String?
        let caseSensitive: Bool
    }

    private static func components(_ host: String) throws -> [Component] {
        let data = try Data(contentsOf: dir.appendingPathComponent("\(host).apple-app-site-association.json"))
        let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let details = try #require((json["applinks"] as? [String: Any])?["details"] as? [[String: Any]])
        #expect(details.first?["appIDs"] as? [String] == ["F9W689P9NE.app.getvela.VelaWallet"])
        return (details.first?["components"] as? [[String: Any]] ?? []).map {
            Component(path: $0["/"] as? String ?? "", fragment: $0["#"] as? String,
                      caseSensitive: $0["caseSensitive"] as? Bool ?? true)
        }
    }

    /// Apple's pattern rules: `*` any run, `?` one character.
    private static func glob(_ pattern: String, _ text: String, caseSensitive: Bool) -> Bool {
        let p = Array(caseSensitive ? pattern : pattern.lowercased())
        let t = Array(caseSensitive ? text : text.lowercased())
        func match(_ i: Int, _ j: Int) -> Bool {
            if i == p.count { return j == t.count }
            if p[i] == "*" { return (j...t.count).contains { match(i + 1, $0) } }
            guard j < t.count, p[i] == "?" || p[i] == t[j] else { return false }
            return match(i + 1, j + 1)
        }
        return match(0, 0)
    }

    private static func claimed(_ url: URL, by components: [Component]) -> Bool {
        let path = url.path.isEmpty ? "/" : url.path
        let fragment = AppLink.rawFragment(url)
        return components.contains { c in
            guard glob(c.path, path, caseSensitive: true) else { return false }
            guard let pattern = c.fragment else { return true }
            guard let raw = fragment else { return false }
            // AASA compares the decoded fragment with the decoded pattern.
            let decodedPattern = pattern.removingPercentEncoding ?? pattern
            let decoded = raw.removingPercentEncoding ?? raw
            return glob(decodedPattern, decoded, caseSensitive: c.caseSensitive)
        }
    }

    private let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"

    @Test func whatIsClaimedIsWhatIsRead() throws {
        let wallet = try Self.components("wallet.getvela.app")
        let site = try Self.components("getvela.app")
        let samples: [(String, Bool)] = [
            ("https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)", true),
            ("https://wallet.getvela.app/zh-TW/wallet#msa=100:\(safe)", true),
            ("https://wallet.getvela.app/en/wallet#ms%3D100.\(safe).\(hash)", true),
            ("https://wallet.getvela.app/en#msa=\(safe)", true),
            ("https://wallet.getvela.app/es-MX#ms=100.\(safe).\(hash)", true),
            ("https://wallet.getvela.app/en/connect#for=0x88cca0eedbf2c4426110bbfc998f048689266894", true),
            ("https://wallet.getvela.app/pay?to=\(safe)&chain=100", true),
            ("https://getvela.app/pay?to=\(safe)&chain=100", true),
            // The web wallet itself is never taken from the browser.
            ("https://wallet.getvela.app/en/wallet", false),
            ("https://wallet.getvela.app/en/wallet#tab=history", false),
            ("https://wallet.getvela.app/en/settings", false),
            ("https://wallet.getvela.app/en", false),
            ("https://getvela.app/download", false),
        ]
        for (raw, expected) in samples {
            let url = try #require(URL(string: raw))
            let components = url.host == "getvela.app" ? site : wallet
            let isClaimed = Self.claimed(url, by: components)
            #expect(isClaimed == expected, "\(raw): claimed \(isClaimed)")
            #expect((AppLink.read(url) != nil) == expected, "\(raw): read \(AppLink.read(url) != nil)")
        }
        // The passkeys' association is kept on the site.
        let siteJSON = try JSONSerialization.jsonObject(
            with: Data(contentsOf: Self.dir.appendingPathComponent("getvela.app.apple-app-site-association.json"))
        ) as? [String: Any]
        let apps = (siteJSON?["webcredentials"] as? [String: Any])?["apps"] as? [String]
        #expect(apps?.contains("F9W689P9NE.app.getvela.VelaWallet") == true)
        #expect(site.contains { $0.path == "/sign" }, "the extension's /sign hand-off is gone")
    }
}

@MainActor
struct MultisigLinkModelTests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let url = URL(string: "https://wallet.getvela.app/en/wallet#ms=1")!
    private let tx = MultisigProposalRef(
        chainId: 100, multisig: "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83",
        safeTxHash: "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
    )

    private func model(_ link: AppLink) -> MultisigLinkModel? {
        MultisigLinkModel.of(link, loc: en, chainName: { $0 == 100 ? "Gnosis" : nil })
    }

    @Test func aProposalShowsWhatTheCoreRead() throws {
        let m = try #require(model(.proposal(tx, url: url)))
        #expect(m.title == "Multisig transaction")
        #expect(m.rows.map(\.value) == ["Gnosis", tx.multisig, tx.safeTxHash])
        #expect(m.rows.map(\.mono) == [false, true, true])
        #expect(m.openLabel == "Open in system browser")
        #expect(!m.broken)
    }

    @Test func anInviteNamesItsMembersAndNetworks() throws {
        let invite = MultisigInviteRef(
            multisig: tx.multisig, chainIds: [100, 777],
            members: [InvitedMemberRef(address: nil, name: "Alice", kind: "passkey"),
                      InvitedMemberRef(address: nil, name: nil, kind: nil)]
        )
        let m = try #require(model(.invite(invite, url: url)))
        #expect(m.title == "Add a multisig")
        #expect(m.body?.contains("Gnosis") == true)
        // A network this device does not have is said as such.
        #expect(m.body?.contains("777") == true)
        #expect(m.members == [MultisigLinkModel.Member(label: "Alice · Vela account (passkey)")])
        #expect(m.membersLabel != nil)
    }

    @Test func theConnectPageIsForAnotherWallet() throws {
        let connect = ConnectRef(invite: nil, members: [], tx: tx, damaged: false)
        let m = try #require(model(.connect(connect, url: url)))
        #expect(m.title == "For a member with another wallet")
        #expect(m.body?.contains("MetaMask") == true)
        #expect(m.rows.count == 3)
    }

    @Test func aDamagedLinkSaysWhatIsWrongInTheWebsWords() throws {
        let cases: [(AppLink.BrokenLink, String)] = [
            (.proposal, "home.multisig.proposal.badLink"),
            (.invite, "home.multisig.add.badInvite"),
            (.inviteChecksum, "home.multisig.add.badChecksum"),
            (.connect, "home.multisig.external.linkDamaged"),
        ]
        for (kind, key) in cases {
            let m = try #require(model(.broken(kind, url: url)))
            #expect(m.broken)
            #expect(m.body == en.t(key))
            #expect(m.body != key, "\(key) is not in the catalog")
            #expect(m.rows.isEmpty)
        }
    }

    @Test func aDoorOrAPaymentHasNoSheet() {
        #expect(model(.entry(.send)) == nil)
        #expect(model(.pay(.open(url: "https://x.example"))) == nil)
    }

    /// Every word the sheet says is in the catalog, in Chinese too.
    @Test func everyWordIsTheCorpus() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let k = I18nKeys.AppLinks.self
        for key in [k.proposalTitle, k.inviteTitle, k.connectTitle, k.connectBody, k.network, k.multisig,
                    k.safeTxHash, k.missingNetwork, k.usedOn, k.invitedMembers, k.kindVela, k.kindWallet,
                    k.kindContract, k.badProposal, k.badInvite, k.badInviteChecksum, k.badConnect,
                    k.openInBrowser, k.copyLink, k.close, I18nKeys.Widgets.updated] {
            #expect(en.t(key) != key, "\(key) missing in en")
            #expect(zh.t(key) != key, "\(key) missing in zh")
        }
    }
}

// MARK: - The widget snapshot

@MainActor
struct WidgetSnapshotTests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    private func hero(_ state: BalanceStateKind, integer: String? = "$12,480", decimals: String? = "36",
                      status: BalanceStatusModel? = nil, checking: String? = nil,
                      noValue: String? = nil) -> BalanceModel {
        BalanceModel(label: en.t("home.totalBalance"), currency: "USD", state: state,
                     integer: integer, decimals: decimals, status: status,
                     a11yHide: en.t("home.a11yHideBalance"), a11yShow: en.t("home.a11yShowBalance"),
                     checkingText: checking, noValueText: noValue)
    }

    private func tempStore() throws -> WidgetSnapshotStore {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("widget-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return WidgetSnapshotStore(directory: dir)
    }

    @Test func aFigureIsTheHerosFigure() {
        let s = WidgetPublisher.snapshot(balance: hero(.normal), asOfMs: 1_700_000_000_000,
                                         accountName: "Main", loc: en)
        #expect(s.figure == .amount)
        #expect(s.integer == "$12,480" && s.decimals == "36")
        #expect(s.title == "Total balance")
        #expect(s.spoken == "$12,480.36 USD")
        #expect(s.asOf == Date(timeIntervalSince1970: 1_700_000_000))
        #expect(s.accountName == "Main")
        #expect(!s.partial && s.note == nil)
        #expect([s.sendLabel, s.receiveLabel, s.scanLabel] == ["Send", "Receive", "Scan"])
    }

    /// Hidden stays hidden: the digits are not even written down.
    @Test func aHiddenBalanceIsNeverWritten() throws {
        let s = WidgetPublisher.snapshot(balance: hero(.hidden), asOfMs: 1, accountName: nil, loc: en)
        #expect(s.figure == .hidden && s.integer == nil && s.decimals == nil && s.asOf == nil)
        #expect(s.spoken == "Show balance")
        let store = try tempStore()
        #expect(store.write(s))
        let file = try Data(contentsOf: try #require(store.directory).appendingPathComponent(WidgetSnapshotStore.fileName))
        #expect(String(data: file, encoding: .utf8)?.contains("12,480") == false)
    }

    @Test func aPartialTotalSaysSoAndAFirstReadSaysChecking() {
        let partial = WidgetPublisher.snapshot(
            balance: hero(.normal, status: BalanceStatusModel(kind: .warning, text: "Some balances are still updating.")),
            asOfMs: 5, accountName: nil, loc: en)
        #expect(partial.partial && partial.note == "Some balances are still updating.")
        // "Updating…" under a refresh the person asked for is the app's moment.
        let refreshing = WidgetPublisher.snapshot(
            balance: hero(.normal, status: BalanceStatusModel(kind: .refreshing, text: "Updating…")),
            asOfMs: 5, accountName: nil, loc: en)
        #expect(!refreshing.partial && refreshing.note == nil)
        let checking = WidgetPublisher.snapshot(balance: hero(.loading, integer: nil, decimals: nil, checking: "Checking…"),
                                                asOfMs: nil, accountName: nil, loc: en)
        #expect(checking.figure == .none && checking.note == "Checking…" && checking.asOf == nil)
        let testnet = WidgetPublisher.snapshot(balance: hero(.noValue, integer: nil, decimals: nil, noValue: "No value"),
                                               asOfMs: 9, accountName: nil, loc: en)
        #expect(testnet.figure == .none && testnet.note == "No value" && testnet.asOf == nil)
    }

    @Test func theStoreReadsWhatItWroteAndNothingElse() throws {
        let store = try tempStore()
        #expect(store.read() == nil)
        let s = WidgetPublisher.snapshot(balance: hero(.normal), asOfMs: 1_700_000_000_000, accountName: nil, loc: en)
        #expect(store.write(s))
        #expect(store.read() == s)
        var future = s
        future.version = WidgetSnapshot.currentVersion + 1
        #expect(store.write(future))
        #expect(store.read() == nil, "a shape this build does not know was drawn")
        var empty = s
        empty.integer = ""
        #expect(store.write(empty))
        #expect(store.read() == nil, "a figure without digits was drawn")
        store.clear()
        #expect(store.read() == nil)
        // No App Group (an unsigned build): nothing is written, nothing read.
        let none = WidgetSnapshotStore(directory: nil)
        #expect(!none.write(s) && none.read() == nil)
    }

    /// Written only when it changes — and withdrawn when nobody is signed in.
    @Test func publishingRedrawsOnlyOnAChange() throws {
        let store = try tempStore()
        var reloads = 0
        let s = WidgetPublisher.snapshot(balance: hero(.normal), asOfMs: 1, accountName: nil, loc: en)
        WidgetPublisher.publish(s, store: store, reload: { reloads += 1 })
        WidgetPublisher.publish(s, store: store, reload: { reloads += 1 })
        #expect(reloads == 1)
        WidgetPublisher.publish(nil, store: store, reload: { reloads += 1 })
        #expect(reloads == 2 && store.read() == nil)
        WidgetPublisher.publish(nil, store: store, reload: { reloads += 1 })
        #expect(reloads == 2)
    }

    /// When, never how long ago: a widget is not redrawn every minute.
    @Test func theStampSaysWhenTheFigureWasRead() throws {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        var s = WidgetPublisher.snapshot(balance: hero(.normal), asOfMs: 1_700_000_000_000 - 3_600_000,
                                         accountName: nil, loc: en)
        let today = try #require(WidgetFace.stamp(s, now: now))
        #expect(today.hasPrefix("Updated "))
        #expect(!today.contains("2023"), "today's stamp carries a date: \(today)")
        s.asOf = now.addingTimeInterval(-3 * 86_400)
        let older = try #require(WidgetFace.stamp(s, now: now))
        #expect(older != today && older.count > today.count, "an older figure does not say its day: \(older)")
        s.asOf = nil
        #expect(WidgetFace.stamp(s, now: now) == nil)
    }

    /// Every face draws, in both appearances and both languages, for every
    /// state — and, with `TEST_RUNNER_VELA_WIDGET_SHOTS=<dir>`, is saved for
    /// the round's screenshots.
    @Test func everyFaceRenders() throws {
        let out = ProcessInfo.processInfo.environment["VELA_WIDGET_SHOTS"].map { URL(fileURLWithPath: $0) }
        if let out { try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true) }
        for tag in ["en", "zh"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let base = BalanceModel(label: loc.t("home.totalBalance"), currency: "USD", state: .normal,
                                    integer: "$12,480", decimals: "36", status: nil,
                                    a11yHide: loc.t("home.a11yHideBalance"), a11yShow: loc.t("home.a11yShowBalance"))
            let now = Date()
            let figure = WidgetPublisher.snapshot(balance: base, asOfMs: now.timeIntervalSince1970 * 1000 - 600_000,
                                                  accountName: "Parallel One", loc: loc)
            let hiddenModel = BalanceModel(label: base.label, currency: "USD", state: .hidden,
                                       a11yHide: base.a11yHide, a11yShow: base.a11yShow)
            let hidden = WidgetPublisher.snapshot(balance: hiddenModel, asOfMs: nil, accountName: nil, loc: loc)
            for scheme in [ColorScheme.light, .dark] {
                let name = "\(tag)-\(scheme == .dark ? "dark" : "light")"
                try save(WidgetShot.small(figure), "widget-small-\(name)", out: out, scheme: scheme)
                try save(WidgetShot.medium(figure), "widget-medium-\(name)", out: out, scheme: scheme)
                try save(WidgetShot.lock(figure), "widget-lock-\(name)", out: out, scheme: scheme)
                try save(WidgetShot.small(hidden), "widget-small-hidden-\(name)", out: out, scheme: scheme)
            }
        }
    }

    private func save(_ view: some View, _ name: String, out: URL?, scheme: ColorScheme) throws {
        let renderer = ImageRenderer(content: view.environment(\.colorScheme, scheme))
        renderer.scale = 3
        let image = try #require(renderer.uiImage, "\(name) did not render")
        #expect(image.size.width > 0)
        if let out, let png = image.pngData() {
            try png.write(to: out.appendingPathComponent("\(name).png"))
        }
    }
}

/// The faces at the sizes a Home Screen and a Lock Screen give them, on the
/// widget's own ground — for the snapshots.
@MainActor
enum WidgetShot {
    static func small(_ s: WidgetSnapshot) -> some View {
        face(BalanceSmallFace(snapshot: s), width: 170, height: 170)
    }

    static func medium(_ s: WidgetSnapshot) -> some View {
        face(BalanceMediumFace(snapshot: s), width: 364, height: 170)
    }

    /// The Lock Screen's rectangle and its inline line, over a wallpaper.
    static func lock(_ s: WidgetSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            BalanceInlineFace(snapshot: s)
                .font(.system(size: 15, weight: .medium))
            BalanceRectangularFace(snapshot: s)
                .frame(width: 172, height: 76)
            ScanCircularFace(label: s.scanLabel)
                .frame(width: 76, height: 76)
        }
        .foregroundStyle(.white)
        .padding(20)
        .frame(width: 260)
        .background(LinearGradient(colors: [Color(white: 0.25), Color(white: 0.1)],
                                   startPoint: .top, endPoint: .bottom))
        .environment(\.colorScheme, .dark)
    }

    private static func face(_ content: some View, width: CGFloat, height: CGFloat) -> some View {
        Wrapped(content: content, width: width, height: height)
    }

    private struct Wrapped<Content: View>: View {
        @Environment(\.colorScheme) private var scheme
        let content: Content
        let width: CGFloat
        let height: CGFloat

        var body: some View {
            content
                .padding(16)
                .frame(width: width, height: height)
                .background(WidgetPalette(scheme).page)
                .clipShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
                .padding(12)
                .background(scheme == .dark ? Color(white: 0.05) : Color(white: 0.85))
        }
    }
}

// MARK: - The scanner

@MainActor
struct DataScannerChoiceTests {

    @Test func visionKitReadsOnlyWhereThePhoneHasItAndNeverInAFixture() {
        #expect(DataScannerSupport.usesDataScanner(fixtureOnly: false, deviceAllows: true))
        #expect(!DataScannerSupport.usesDataScanner(fixtureOnly: false, deviceAllows: false))
        #expect(!DataScannerSupport.usesDataScanner(fixtureOnly: true, deviceAllows: true))
    }

    /// A fixture session never reaches the camera, VisionKit's included.
    @Test func aFixtureScannerNeverStartsVisionKit() async {
        let scanner = CameraScanner(fixtureOnly: true, dataScannerAllowed: true)
        await scanner.start()
        #expect(!scanner.usesDataScanner && !scanner.running && scanner.hardwareReaches == 0)
    }

    /// Whatever reads the code, it is delivered once.
    @Test func aCodeIsDeliveredOnce() {
        let scanner = CameraScanner(fixtureOnly: true)
        var heard: [String] = []
        scanner.onScan = { heard.append($0) }
        scanner.deliver("ethereum:0x01")
        scanner.deliver("ethereum:0x02")
        #expect(heard == ["ethereum:0x01"])
    }

    /// The simulator has neither VisionKit's scanner nor a camera: the
    /// AVFoundation path stays the fallback.
    @Test func theSimulatorFallsBack() {
        #if targetEnvironment(simulator)
        #expect(!DataScannerSupport.deviceAllows)
        #endif
    }
}

// MARK: - Haptics

@MainActor
struct HapticChoreographyTests {

    /// The tap that moves value or signs feels like one.
    @Test func theValueTapIsAConfirm() {
        var confirmed = 0
        let played = VelaHaptic.recording {
            VelaButton.pressed({ confirmed += 1 }, haptic: .confirm)
        }
        #expect(played == [.confirm] && confirmed == 1)
        #expect(VelaHaptic.recording { VelaButton.pressed {} } == [.press])
    }

    /// The declarative and the imperative say the same thing.
    @Test func everyHapticHasItsSystemFeedback() {
        #expect(VelaHaptic.allCases.count == 7)
        #expect(VelaHaptic.success.sensory == .success)
        #expect(VelaHaptic.reject.sensory == .error)
        #expect(VelaHaptic.approvalCounted.sensory == .increase)
        #expect(VelaHaptic.confirm.sensory != VelaHaptic.press.sensory)
    }
}

// MARK: - The system's words

/// Siri's phrases, the Shortcuts' titles and the widgets' names are the
/// corpus's, in all 15 languages, and every key the code names is there.
struct SystemStringCatalogTests {

    private static let app = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()

    private static func catalog(_ path: String) throws -> [String: [String: String]] {
        let data = try Data(contentsOf: app.appendingPathComponent(path))
        let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let strings = try #require(json["strings"] as? [String: [String: Any]])
        return strings.mapValues { entry in
            ((entry["localizations"] as? [String: [String: Any]]) ?? [:]).compactMapValues {
                ($0["stringUnit"] as? [String: Any])?["value"] as? String
            }
        }
    }

    private static let apple = ["en", "zh-Hans", "zh-Hant", "zh-HK", "ja", "ko", "vi", "id", "tr",
                                "es-MX", "pt-BR", "fr", "de", "ru", "it"]

    @Test func everyPhraseNamesTheAppInEveryLanguage() throws {
        let phrases = try Self.catalog("VelaWallet/AppShortcuts.xcstrings")
        #expect(phrases.count == 6)
        for (key, values) in phrases {
            #expect(Set(values.keys) == Set(Self.apple), "\(key) lacks a language")
            for (lang, value) in values {
                #expect(value.contains("${applicationName}"), "\(key) [\(lang)] does not name the app")
            }
        }
        #expect(phrases["Send with ${applicationName}"]?["zh-Hans"]?.contains("转账") == true)
    }

    @Test func everyKeyTheCodeNamesIsThere() throws {
        let app = try Self.catalog("VelaWallet/Localizable.xcstrings")
        let widgets = try Self.catalog("VelaWidgets/Localizable.xcstrings")
        for key in ["intent.send.title", "intent.receive.title", "intent.scan.title"] {
            #expect(Set(app[key].map { Array($0.keys) } ?? []) == Set(Self.apple), "\(key) missing in the app")
            #expect(widgets[key] != nil, "\(key) missing in the extension")
        }
        for key in ["widget.balance.name", "widget.balance.description", "widget.scan.name",
                    "widget.scan.description", "control.scan.title"] {
            #expect(Set(widgets[key].map { Array($0.keys) } ?? []) == Set(Self.apple), "\(key) missing")
        }
        #expect(widgets["widget.balance.name"]?["ja"] != nil)
    }
}
