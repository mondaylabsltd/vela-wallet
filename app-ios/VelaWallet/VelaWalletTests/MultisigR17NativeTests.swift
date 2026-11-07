//
//  MultisigR17NativeTests.swift
//  VelaWalletTests
//
//  Round 17's iOS findings (docs/multisig-machines.md §30):
//
//  - r17-6: the chip says "Executing…" in the same frame the body would —
//    also once the mailbox calls it done and the receipt is not judged yet.
//  - r17-7: a cold start never shows Welcome before the account store is read.
//  - r17-17: a damaged link is said in words; no raw catalog key is drawn.
//  - r17-18: an unread review draws Approve, held still, nothing of executing.
//  - r17-19: a record the service refused is said as refused.
//  - r17-20: the switcher's count alone; the create sheet's foot on the
//    keyboard; Try again busy at once; the key index bounded and said.
//  - r17-14: the testnet space prices nothing in Send.
//  - r17-21: "Try again" for every read's retry, "Check again" only for a fee.
//  - r17-5: Activity's unread line over rows, on the home and in History.
//

#if DEBUG
import Foundation
import Observation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct MultisigR17NativeTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private static let hash = MultisigViewsTests.hash
    private static let two = MultisigViewsTests.two

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private func source(_ path: String) throws -> String {
        try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent(path), encoding: .utf8)
    }

    /// The proposals view over one item, with these fields replacing the
    /// empty ones (`"executions":{…}` and so on).
    private func model(status: String = "pending", approved: Int = 1, executing: Bool = false,
                       replacing: [String: String] = [:]) throws -> MsProposalModel {
        var item = MultisigViewsTests.item(approved: approved, status: status,
                                           canApprove: status == "pending" ? [Self.two] : [],
                                           canExecute: status == "done" ? [] : [Self.two])
        if executing { item = item.replacingOccurrences(of: #""executing":false"#, with: #""executing":true"#) }
        var json = MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":\#(approved),"threshold":2}"#,
            extra: #","approve_execute":{"100:\#(Self.hash)":["\#(Self.two)"]}"#
        )
        for (key, value) in replacing {
            json = json.replacingOccurrences(of: "\"\(key)\":{}", with: "\"\(key)\":\(value)")
        }
        let view = try #require(decode(MsProposalsView.self, json))
        return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    private static let waiting = #"{"100:\#(hash)":{"kind":"waiting"}}"#

    // MARK: - r17-6: the chip and the body say the same

    /// The tester's frame: the mailbox already calls it done (its nonce is
    /// used) while the flight's receipt is not judged — the body said
    /// "Executing…" and the chip "1/2 approved" for ~1.2 s.
    @Test func aFlightNotJudgedYetIsExecutingOnTheChipEvenOnceTheMailboxCallsItDone() throws {
        let done = try model(status: "done", replacing: ["executions": Self.waiting])
        #expect(done.statusLine == words.t("status.executing"), "\(done.statusLine)")
        #expect(done.executingHere)
        // Said once: the chip, never a second "Executing…" under it.
        #expect(!done.notes.contains { $0.text == words.t("proposal.executing") }, "\(done.notes.map(\.text))")

        // Before the mailbox moves: the same chip.
        let pending = try model(replacing: ["executions": Self.waiting])
        #expect(pending.statusLine == words.t("status.executing"))
        // The press out, and the core following its flight (r16-8).
        #expect(try model(replacing: ["actions": #"{"execute:\#(Self.hash)":{"kind":"working"}}"#]).statusLine
            == words.t("status.executing"))
        #expect(try model(executing: true).statusLine == words.t("status.executing"))
        // Judged: how it ended, never "Executing…".
        let executed = try model(status: "done", replacing: [
            "executions": #"{"100:\#(Self.hash)":{"kind":"executed","txHash":"0xabc"}}"#,
        ])
        #expect(executed.statusLine == words.t("proposal.executed"))
        #expect(!executed.executingHere)
        // Nothing out: the mailbox's count.
        #expect(try model().statusLine != words.t("status.executing"))
    }

    // MARK: - r17-7: never Welcome before the store is read

    @Test func theRootDrawsWelcomeOnlyForAStoreReadAndEmpty() throws {
        #expect(RootScreen.of(.loading) == .ground, "the store is being read: nothing of any screen")
        #expect(RootScreen.of(.onboarding) == .welcome)
        #expect(RootScreen.of(.wallet) == .wallet)
        #expect(RootScreen.of(SessionView.booting.allowedRoute) == .ground, "the first frame")
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains("let screen = RootScreen.of(session.view.allowedRoute)"))
        #expect(root.contains("if screen == .ground {"))
        #expect(!root.contains("if session.view.allowedRoute == .wallet {"), "Welcome was the `else` of `.wallet`")
    }

    /// Every route the session machine publishes on a cold start, as the root
    /// draws it — sampled on every change of the view, from the first frame.
    private func launch(store: AccountStore) async -> [RootScreen] {
        final class Seen { var screens: [RootScreen] = [] }
        let seen = Seen()
        let session = SessionController(store: store, registryTransport: IndexScript.unreachable)
        func watch() {
            withObservationTracking {
                _ = session.view.allowedRoute
            } onChange: {
                Task { @MainActor in
                    let screen = RootScreen.of(session.view.allowedRoute)
                    if seen.screens.last != screen { seen.screens.append(screen) }
                    watch()
                }
            }
        }
        seen.screens.append(RootScreen.of(session.view.allowedRoute))
        watch()
        session.boot()
        await Wait.until({ !session.view.loading }, orIdle: { session.isIdle })
        // The last change is drawn (its observation hop has run).
        await Wait.until { seen.screens.last == RootScreen.of(session.view.allowedRoute) }
        return seen.screens
    }

    @Test func aColdStartWithAnAccountNeverDrawsWelcome() async throws {
        let store = AccountStore(defaults: UserDefaults(suiteName: "vela.tests.r17launch.\(UUID().uuidString)")!)
        let publicKey = "04" + String(repeating: "11", count: 64)
        await store.saveAccount([
            "id": "cred-1", "name": "Mine", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "public_key_hex": publicKey,
            "created_at_iso": "2026-09-01T00:00:00.000Z",
            "keys": [["credential_id": "cred-1", "public_key_hex": publicKey, "name": "Mine", "transports": "internal"]],
            "sign_in_key": ["credential_id": "cred-1", "method": "platform"],
        ])
        let screens = await launch(store: store)
        #expect(screens.first == .ground, "the first frame is the ground")
        #expect(screens.last == .wallet)
        #expect(!screens.contains(.welcome), "Welcome flashed: \(screens)")
    }

    @Test func aColdStartWithNoAccountDrawsWelcomeOnceTheStoreIsRead() async throws {
        let store = AccountStore(defaults: UserDefaults(suiteName: "vela.tests.r17launch.\(UUID().uuidString)")!)
        let screens = await launch(store: store)
        #expect(screens == [.ground, .welcome], "\(screens)")
    }

    // MARK: - r17-17: a damaged link in words; no raw key drawn anywhere

    private func stand(_ json: String) -> MsLinkStand? { decode(MsLinkStand.self, json) }

    @Test func aDamagedLinkIsSaidInTheCatalogsWordsInEveryLanguage() throws {
        for tag in ["en", "zh"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let words = MsWords(loc: loc, hub: nil)
            let proposal = MultisigLinkStandSheet.brokenLine(
                stand(#"{"kind":"broken","link":"proposal","text":"5042002.0x27461D1a7e63007bD43ECB72C7813673F0420B77.0x8496296c11a8","fault":null}"#),
                words: words)
            #expect(proposal == words.t("proposal.badLink"))
            #expect(!proposal.contains("0x"), "never what the link held: \(proposal)")
            #expect(MultisigLinkStandSheet.brokenLine(
                stand(#"{"kind":"broken","link":"add","text":"0xab","fault":"checksum"}"#), words: words)
                == words.t("add.badChecksum"))
            #expect(MultisigLinkStandSheet.brokenLine(
                stand(#"{"kind":"broken","link":"add","text":"0xab","fault":"damaged"}"#), words: words)
                == words.t("add.badInvite"))
            // Every way a link's lookup can fail (`AddOutcome`), said.
            for kind in ["notFound", "unavailable", "unsupported", "notMultisig", "chooseNetwork", "unchecked",
                         "refused", "highRisk", "unverified", "busy", "somethingNew"] {
                let line = MultisigLinkStandSheet.failedLine(
                    MsJSON(any: ["kind": kind, "chain_id": 100, "findings": [Any]()]), words: words)
                #expect(!line.contains("home.multisig"), "\(tag) \(kind): \(line)")
                #expect(!line.isEmpty)
            }
            #expect(loc.missed.isEmpty, "\(tag): keys drawn raw: \(loc.missed.sorted())")
        }
    }

    /// The guard: the missing-translation fallback is caught. A key with no
    /// words is recorded by `Loc` (DEBUG) — and no multisig key is built from
    /// a kind the shell does not list, the way `add.\(kind)` drew
    /// `home.multisig.add.refused`.
    @Test func noRawCatalogKeyCanBeDrawn() throws {
        let probe = Loc(overrideTag: "en", preferredLanguages: [])
        _ = probe.t("home.multisig.add.refused")
        #expect(probe.missed == ["home.multisig.add.refused"], "the fallback is caught")
        _ = probe.t("home.multisig.add.notFound")
        #expect(probe.missed.count == 1, "a key with words is not")

        // Every literal key the multisig views and the shell's words name
        // exists in the corpus.
        let corpus = try Self.corpus()
        let dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Multisig")
        let files = try FileManager.default.subpathsOfDirectory(atPath: dir.path).filter { $0.hasSuffix(".swift") }
        #expect(files.count > 10)
        let literal = try NSRegularExpression(pattern: #"(?<![A-Za-z0-9_.])(?:words\.)?t\("([A-Za-z0-9_.]+)""#)
        let built = try NSRegularExpression(pattern: #"\bt\("[^"]*\\\("#)
        var missing: [String] = []
        var dynamic: [String] = []
        for file in files {
            let text = try String(contentsOf: dir.appendingPathComponent(file), encoding: .utf8)
            let range = NSRange(text.startIndex..., in: text)
            for match in literal.matches(in: text, range: range) {
                guard let r = Range(match.range(at: 1), in: text) else { continue }
                let key = "home.multisig." + text[r]
                if !Self.has(key, in: corpus) { missing.append("\(file): \(key)") }
            }
            for match in built.matches(in: text, range: range) {
                guard let r = Range(match.range, in: text) else { continue }
                let line = text[text.lineRange(for: r)].trimmingCharacters(in: .whitespaces)
                dynamic.append("\(file): \(line)")
            }
        }
        #expect(missing.isEmpty, "keys with no words: \(missing)")
        // A key built from a value: only the words' own wrappers, the core's
        // reading keys, and kinds behind a closed list.
        let allowed = [
            "loc.t(\"home.multisig.\\(key)\"",
            "review.reading.\\(key)",
            "case \"notFound\", \"unavailable\", \"unsupported\"",
        ]
        let open = dynamic.filter { line in !allowed.contains { line.contains($0) } }
        #expect(open.isEmpty, "a key built from an unlisted value: \(open)")
    }

    private static func corpus() throws -> Set<String> {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("assets/i18n")
        var keys = Set<String>()
        func walk(_ value: Any, _ prefix: String) {
            if let object = value as? [String: Any] {
                for (k, v) in object { walk(v, prefix.isEmpty ? k : "\(prefix).\(k)") }
            } else {
                keys.insert(prefix)
            }
        }
        for name in ["en.json", "multisig/en.json"] {
            let data = try Data(contentsOf: root.appendingPathComponent(name))
            walk(try JSONSerialization.jsonObject(with: data), "")
        }
        return keys
    }

    private static func has(_ key: String, in corpus: Set<String>) -> Bool {
        corpus.contains(key) || ["_one", "_other", "_zero", "_few", "_many"].contains { corpus.contains(key + $0) }
    }

    // MARK: - r17-18: an unread review offers Approve, held still

    @Test func whileTheReviewIsUnreadNothingAboutExecutingIsSaid() throws {
        // Unread: the review is out (no state) — the door is shut.
        let shut = try model(replacing: [
            "previews": #"{"100:\#(Self.hash):\#(Self.two.lowercased())":{"kind":"unchecked"}}"#,
        ])
        #expect(!shut.signable)
        #expect(!shut.saysExecution)
        #expect(!shut.oneTouchShown(shut.view.approveExecute(shut.item)),
                "never a shut \"Approve and execute\"")
        let sheet = try source("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift")
        #expect(sheet.contains("let completes = model.oneTouchShown(oneTouch)"))
        #expect(sheet.contains("} else if model.saysExecution, let why = model.approveOnlyReason(chosen: member) {"))
        #expect(sheet.contains("if model.saysExecution, [\"unpriced\", \"unchecked\", \"unverified\"]"))
        // Read and allowed: the one press, as before.
        let review = #"{"100:\#(Self.hash)":{"sign":{"allowed":true,"anyway":false},"concerns":[],"state":{"source":"probe","verdict":{"kind":"unchanged"}},"simulation":{"kind":"deltas","deltas":[]},"recipients":[],"fields":[],"hashes":{"domainHash":"0xd","messageHash":"0xm","safeTxHash":"\#(Self.hash)"}}}"#
        let open = try model(replacing: [
            "reviews": review, "review_states": #"{"100:\#(Self.hash)":{"kind":"ready"}}"#,
        ])
        #expect(open.signable)
        #expect(open.oneTouchShown(open.view.approveExecute(open.item)))
    }

    // MARK: - r17-19: a refused record, said as refused

    @Test func aRecordTheServiceRefusedIsNeverSaidAsUnreachable() {
        let refused = MultisigCreateSheet.sharedLine(shared: false, solo: false, refused: true, working: false, words: words)
        #expect(refused == words.t("settings.shareAgainNote"))
        #expect(refused != words.t("create.savedLater"))
        #expect(MultisigCreateSheet.sharedLine(shared: false, solo: false, refused: false, working: false, words: words)
            == words.t("create.savedLater"), "a service that did not answer")
        #expect(MultisigCreateSheet.sharedLine(shared: true, solo: false, refused: true, working: false, words: words)
            == words.t("create.shared"))
    }

    // MARK: - r17-20

    @Test func theSwitchersCountStandsAloneWithoutADanglingSeparator() {
        let count = loc.t(I18nKeys.SettingsUi.accountsCount, count: 1)
        #expect(SettingsLive.accountsSummary(count: count, total: nil) == "1 account")
        #expect(SettingsLive.accountsSummary(count: count, total: "Total $1.00") == "1 account · Total $1.00")
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let alone = SettingsLive.accountsSummary(count: zh.t(I18nKeys.SettingsUi.accountsCount, count: 2), total: nil)
        #expect(!alone.hasSuffix("·") && !alone.hasSuffix(" "), "\(alone)")
    }

    @Test func theCreateSheetsFootIsAttachedToTheKeyboard() throws {
        let components = try source("VelaWallet/Features/Multisig/Views/MultisigComponents.swift")
        #expect(components.contains(".background(theme.bgRaised.ignoresSafeArea(.all, edges: .bottom))"),
                "the foot's ground runs down behind the keyboard's bar")
        #expect(components.contains(".safeAreaInset(edge: .bottom, spacing: 0)"))
    }

    @Test func aTryAgainIsBusyTheMomentItIsPressed() throws {
        #expect(!MsRetryButton.busy(pressedOn: nil, said: "Couldn't reach"))
        #expect(MsRetryButton.busy(pressedOn: "Couldn't reach", said: "Couldn't reach"))
        #expect(!MsRetryButton.busy(pressedOn: "Couldn't reach", said: "Busy"), "the answer came")
        let sheet = try source("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift")
        #expect(sheet.contains("identifier: \"multisig.proposal.openRetry\""))
        #expect(sheet.contains("identifier: \"multisig.proposal.missingRetry\""))
        let add = try source("VelaWallet/Features/Multisig/Views/MultisigAddSheet.swift")
        #expect(add.contains("identifier: \"multisig.link.retry\""))
    }

    @Test func theKeyIndexIsBoundedAndAKeptKeyIsSaid() async throws {
        do {
            _ = try await OnboardingExecutor.bounded(seconds: 0.05) { () async throws -> String in
                try await Task.sleep(for: .seconds(30))
                return "late"
            }
            Issue.record("a question past its bound answered")
        } catch let failure as RegistryFailure {
            #expect(!failure.network, "the index's failure, not the device's")
        }
        let quick = try await OnboardingExecutor.bounded(seconds: 5) { "0xchallenge" }
        #expect(quick == "0xchallenge")
        #expect(OnboardingExecutor.indexAskSeconds <= 30)

        // Said: the key is kept, its row's Confirm finishes it — never the
        // index's raw words, never a silent drop back to the list.
        let kept = PromptKind(json: ["type": "create_failed", "detail": "The passkey index did not answer within 20 s",
                                     "key_kept": true])
        let copy = promptCopy(kept, loc: loc)
        #expect(copy.title == loc.t(I18nKeys.Create.keyKeptTitle))
        #expect(copy.message == loc.t(I18nKeys.Create.keyKeptBodyIndex))
        let offline = PromptKind(json: ["type": "create_failed", "detail": "x", "key_kept": true, "local": true])
        #expect(promptCopy(offline, loc: loc).message == loc.t(I18nKeys.Create.keyKeptBodyNetwork))
        #expect(loc.missed.isEmpty, "\(loc.missed)")
    }

    // MARK: - Drawn (hosted; shots when VELA_TEST_SHOTS is set)

    /// r17-17 as drawn, in zh: the damaged proposal link's sheet says the
    /// catalog's sentence — no key, nothing the link held.
    @Test func aDamagedLinksSheetIsDrawnInWords() async throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0])
        let localsJson = world.localsJson
        let hub = MultisigHub(
            ports: world, loc: zh,
            shelf: VelaStore(defaults: UserDefaults(suiteName: "vela.tests.r17link.\(UUID().uuidString)")!), tokens: nil,
            facts: { .init(serviceUrl: MsSimWorld.service, accounts: localsJson, networks: [100], space: [100], known: []) }
        )
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        await hub.boot()
        hub.openLink(hash: "#ms=5042002.0x27461D1a7e63007bD43ECB72C7813673F0420B77.0x8496296c11a8")
        await Wait.until({ hub.accountsView.link?.kind == "broken" }, orIdle: { hub.accounts.isIdle })
        #expect(hub.accountsView.link?.kind == "broken")
        let words = MsWords(loc: zh, hub: hub)
        let screen = HostedScreen(
            MultisigLinkStandSheet(hub: hub, words: words, onClose: {}).themed(.light)
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.element("multisig.link.broken") != nil }
        let line = try #require(seen.element("multisig.link.broken"))
        #expect(line.label.contains(words.t("proposal.badLink")), "\(line.label)")
        #expect(!seen.says("multisig.link") && !seen.says("home.multisig") && !seen.says("0x8496296c11a8"),
                "\(seen.words)")
        #expect(zh.missed.isEmpty, "\(zh.missed)")
        try await screen.save("i-r17-17-broken-link-zh-after")
    }

    /// r17-20 as drawn: the switcher's line with no total ends at the count.
    @Test func theAccountsSheetsCountIsDrawnAlone() async throws {
        let a = "0x00000000000000000000000000000000000000a1"
        let session = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": a,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Parallel Two", "address": a]]],
        ])
        let model = SettingsLive.withAccounts(session: session, balances: [], display: .live(nil, language: "en"),
                                              on: SettingsFixtures.build(.st1, loc: loc), loc: loc, noValue: true)
        #expect(model.accountsSheet.summary == "1 account")
        let screen = HostedScreen(
            SettingsSheet(model: model, overlay: .accounts, loc: loc, onDismiss: {}, onSignOut: {}).themed(.light)
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.says("Parallel Two") }
        #expect(!seen.words.contains { $0.hasSuffix("·") }, "\(seen.words)")
        try await screen.save("i-r17-20-switcher-count-after")
    }

    // MARK: - r17-14: the testnet space prices nothing

    @Test func theTestnetSpacePricesNothingInSend() throws {
        let cny = WalletLive.Display(code: "CNY", rate: 7.1, language: "zh")
        #expect(cny.fiat(1) != nil)
        let testnet = cny.inSpace(testnet: true)
        #expect(testnet.fiat(1) == nil, "no CN¥ for an amount or a fee")
        #expect(testnet.fiatLine(1, prefix: "≈ ") == WalletLive.Display.withheldLine)
        #expect(cny.inSpace(testnet: false).fiat(1) == cny.fiat(1))
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let view = try decoder.decode(CurrencyViewWire.self, from: Data(#"{"code":"CNY","rate":7.1,"committed":true}"#.utf8))
        #expect(WalletLive.Display.sendContext(view, testnet: true).rate == nil, "the send machine prices nothing")
        #expect(WalletLive.Display.sendContext(view).rate == 7.1)
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains(".inSpace(testnet: space.testnet)"))
        #expect(root.contains("SendStore.displayContext(settings.currency, testnet: space.testnet)"))
    }

    // MARK: - r17-21: one word per action

    @Test func everyReadsRetryIsTryAgainAndOnlyAFeeIsCheckedAgain() throws {
        let views = ["MultisigHomeCard.swift", "MultisigSettingsSheet.swift", "MultisigProposalSheet.swift",
                     "MultisigAddSheet.swift", "MultisigReviewBody.swift"]
        for file in views {
            let text = try source("VelaWallet/Features/Multisig/Views/\(file)")
            #expect(!text.contains("words.t(\"service.retry\")"), "\(file) says \"Retry\" for a read")
        }
        let sheet = try source("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift")
        // "Check again" is the fee's alone: every use asks the looks again.
        for line in sheet.components(separatedBy: "\n") where line.contains("proposal.feeRetry") {
            #expect(line.contains("askPreviews(force: true)"), "\(line)")
        }
    }
}
#endif
