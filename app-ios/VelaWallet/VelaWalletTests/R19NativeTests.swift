//
//  R19NativeTests.swift
//  VelaWalletTests
//
//  Round 18's iOS findings, fixed in round 19 (docs/multisig-machines.md §33):
//
//  - r18-1: "No activity yet" over a network the balance has not read — the
//    feed now hears the balance's reads (`balance_read`), through the real
//    balance and Activity machines, with one network's reads held open.
//  - r18-2: the unread line over rows, on the home and in History, drawn.
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
struct R19NativeTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private static let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let arc = 5_042_002

    // MARK: - A device whose Arc Testnet does not answer

    /// What the test's network does with Arc's reads: hold them open (the
    /// per-chain deadline then fails Arc for the round), refuse them at once,
    /// or answer them. Off the main actor, as a network is: a starved main
    /// actor (the whole suite on one runner) never delays an answer past the
    /// per-chain deadline.
    nonisolated final class Network: @unchecked Sendable {
        static let arc = 5_042_002
        enum Arc { case hung, refused, answers }
        private let lock = NSLock()
        private var _arc: Arc
        init(_ arc: Arc) { _arc = arc }
        var arc: Arc {
            get { lock.withLock { _arc } }
            set { lock.withLock { _arc = newValue } }
        }

        func read(_ address: String, _ chainId: Int) async -> TokenReads.ChainResult {
            guard chainId == Self.arc else {
                return TokenReads.ChainResult(chainId: chainId, tokens: [], failed: false, rateLimited: false)
            }
            switch arc {
            case .answers:
                return TokenReads.ChainResult(chainId: chainId, tokens: [], failed: false, rateLimited: false)
            case .refused:
                return TokenReads.ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
            case .hung:
                // Held open: only the deadline ends it (it cancels this).
                try? await Task.sleep(nanoseconds: 3_600 * 1_000_000_000)
                return TokenReads.ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
            }
        }
    }

    /// The app's two machines over a device in the testnet space: the real
    /// balance core (its executor reading `network`, bounded by a short
    /// per-chain deadline) and the real Activity core (its store holding
    /// `records`, its scan reading every network it asks), connected as
    /// `AppGraph` connects them.
    private struct Device {
        let wallet: WalletStore
        let activity: ActivityStore
        let chains: [Int]
        let suite: String
    }

    private func device(_ network: Network, records: [[String: Any]] = [], deadlineMs: UInt32 = 60_000) -> Device {
        let suite = "vela.tests.r19.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        let store = VelaStore(defaults: defaults)
        store.writeString(VelaStore.Key.networkSpace, NetworkSpaceKind.testnet.rawValue)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts, offline: true)
        let held = HeldTokens()
        let executor = BalanceExecutor(store: store, pool: pool, held: held, chainDeadlineMs: deadlineMs)
        executor.chainReader = { address, chainId in await network.read(address, chainId) }
        let wallet = WalletStore(store: store, pool: pool, held: held, executor: executor)
        let wire = records.compactMap(TxRecords.toWire)
        let activity = ActivityStore(
            store: store, accounts: accounts, held: held,
            trust: TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held),
            perform: { operation in
                switch operation["type"] as? String {
                case "read_tx_store":
                    return CoreJSON.string([
                        "type": "store_loaded", "records": wire,
                        "now_ms": Date().timeIntervalSince1970 * 1000,
                        "read_id": (operation["read_id"] as? NSNumber)?.intValue ?? 0,
                    ])
                case "scan_incoming_transfers":
                    // The scan read every network it asked (the space's
                    // defaults — Arc is not among them for an empty account).
                    return CoreJSON.string(ActivityExecutor.syncCompleted(newCount: 0, unreadChains: []))
                case "resolve_recipient_identity":
                    return CoreJSON.string(["type": "alias_resolved", "addr": operation["addr"] as? String ?? "", "name": NSNull()])
                case "read_receive_time":
                    return CoreJSON.string(["type": "receive_time_read", "id": operation["id"] as? String ?? "", "timestamp_sec": NSNull()])
                default:
                    return CoreJSON.string(["type": "toast_expired", "generation": 0])
                }
            }
        )
        AppGraph.feedFollowsBalance(wallet: wallet, activity: activity)
        let chains = NetworkSpace.chainIds(store: store)
        return Device(wallet: wallet, activity: activity, chains: chains, suite: suite)
    }

    /// The account in front, as the home's `.task` and the space's
    /// `onChange` say it.
    private func open(_ device: Device) {
        device.activity.open(address: Self.account)
        device.activity.spaceChains(device.chains)
        device.wallet.spaceChanged(device.chains, space: NetworkSpaceKind.testnet.rawValue)
        device.wallet.open(address: Self.account)
    }

    /// One frame of the home and of History, as the app draws them.
    private struct Frame {
        let read: Bool?
        let unread: [Int]
        let homeMode: SectionMode
        let homeTitle: String?
        let homeNote: String?
        let historyMode: HistoryMode
        let historyText: String
        let historyNote: String?
        var words: [String] { [homeTitle, homeNote, historyText, historyNote].compactMap { $0 } }
    }

    private func frame(_ device: Device) -> Frame? {
        guard let view = device.wallet.balance, let feed = device.activity.feed else { return nil }
        let home = WalletLive.apply(
            view, feed: feed, feedRead: device.activity.settled(for: Self.account),
            on: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc, shownAccount: Self.account
        )
        let history = FlowsLive.history(
            feed, on: HistoryModel(header: FlowHeaderModel(title: "History", backLabel: "", action: nil, pill: nil),
                                   mode: .rows, emptyText: "", groups: []),
            loc: loc, hidden: false, shownAccount: Self.account
        )
        return Frame(
            read: view.networksRead, unread: view.unreadChainIds ?? [],
            homeMode: home.activitySection.mode,
            homeTitle: home.activitySection.mode == .empty ? home.activitySection.empty?.title : nil,
            homeNote: home.activitySection.note,
            historyMode: history.mode,
            historyText: history.mode == .empty ? history.emptyText : "",
            historyNote: history.note
        )
    }

    /// Every distinct frame until `done` holds of one (or the suite's time
    /// limit cancels the wait).
    private func frames(_ device: Device, until done: (Frame) -> Bool) async -> [Frame] {
        var seen: [Frame] = []
        var last: [String]?
        while !Task.isCancelled {
            if let frame = frame(device) {
                let key = [String(describing: frame.read), "\(frame.unread)", "\(frame.homeMode)", "\(frame.historyMode)"]
                    + frame.words
                if key != last {
                    seen.append(frame)
                    last = key
                }
                if done(frame) { break }
            }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
        return seen
    }

    private var noActivity: String { loc.t("home.emptyNoActivity") }
    private var noHistory: String { loc.t("history.emptyTitle") }
    private var arcUnread: String {
        loc.t("home.activityUnreadOne", vars: ["network": WalletNetworks.builtin.meta(Self.arc)?.displayName ?? "?"])
    }

    /// The claim is the core's to make, and only over every network read:
    /// a frame that says "No activity yet" (home) or "No transactions yet"
    /// (History) is one whose balance read every network of the space.
    private func checkHonest(_ frames: [Frame], sourceLocation: SourceLocation = #_sourceLocation) {
        for frame in frames {
            let claimsEmpty = frame.homeTitle == noActivity || frame.historyText == noHistory
            if claimsEmpty {
                #expect(frame.read == true && frame.unread.isEmpty,
                        "claimed empty over a network not read: read \(String(describing: frame.read)), unread \(frame.unread)",
                        sourceLocation: sourceLocation)
            }
            #expect(!frame.words.contains { $0.contains("Chain ID") || $0.contains("\(Self.arc)") },
                    "a raw chain id: \(frame.words)", sourceLocation: sourceLocation)
        }
    }

    // MARK: - r18-1: never "No activity yet" over an unread network

    /// The tester's cold start (empty account, Arc hung): "No activity yet"
    /// stood beside "Can't reach Arc Testnet" at 52 s. Through the real
    /// machines: while Arc's read is held open the home and History are
    /// still looking; once the deadline fails it, they say Arc's activity
    /// can't be read; once Arc answers, "No activity yet".
    @Test func anEmptyAccountUnderAHungNetworkIsNeverClaimedEmptyUntilEveryNetworkIsRead() async throws {
        let network = Network(.hung)
        // Arc's read is held open: the per-chain deadline (short here) is
        // what ends it, as on the device.
        let device = device(network, deadlineMs: 3_000)
        defer { UserDefaults().removePersistentDomain(forName: device.suite) }
        #expect(device.chains.contains(Self.arc), "Arc is in the testnet space")
        open(device)

        // Held open, then failed at the deadline: the unread line.
        let hung = await frames(device) { $0.homeTitle == arcUnread }
        checkHonest(hung)
        #expect(hung.last?.homeTitle == arcUnread, "\(hung.last?.words ?? [])")
        #expect(hung.last?.historyText == arcUnread, "History says it in the empty line's place")
        // While Arc was held open (the balance's first read not over), the
        // feed's own store and scan were in and nothing was claimed.
        #expect(hung.contains { $0.read == false && $0.homeMode == .loading && $0.historyMode == .loading },
                "\(hung.map(\.words))")
        #expect(device.activity.settled(for: Self.account), "the shell's store read and scan were back")
        // Drawn: the line in the empty state's place.
        let unreadHome = WalletLive.apply(
            try #require(device.wallet.balance), feed: device.activity.feed, feedRead: true,
            on: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc, shownAccount: Self.account
        )
        let screen = HostedScreen(
            WalletScreen(model: unreadHome, loc: loc, onToggleBalance: {}, onStatusTap: {}, onRefreshNow: {}).themed(.light)
        )
        let drawn = try await screen.read { $0.says(arcUnread) }
        #expect(drawn.says(arcUnread) && !drawn.says(noActivity), "\(drawn.words)")
        try await screen.save("r18-1-home-unread-empty-hung-arc")
        screen.close()

        // Arc answers: every network read — now, and only now, empty.
        network.arc = .answers
        device.wallet.refresh(pull: true)
        let read = await frames(device) { $0.homeTitle == noActivity }
        checkHonest(read)
        #expect(read.last?.homeTitle == noActivity)
        #expect(read.last?.historyText == noHistory)
    }

    /// A feed core booted after the balance said its reads hears them again
    /// (as the space is said again): the home's `.task` can come after the
    /// first view.
    @Test func aFeedBootedLaterHearsTheBalancesReadsAgain() async throws {
        let network = Network(.refused)
        let device = device(network)
        defer { UserDefaults().removePersistentDomain(forName: device.suite) }
        device.wallet.spaceChanged(device.chains, space: NetworkSpaceKind.testnet.rawValue)
        device.wallet.open(address: Self.account)
        await Wait.until { device.wallet.balance?.networksRead == true }
        #expect(device.wallet.balance?.unreadChainIds == [Self.arc])
        device.activity.open(address: Self.account)
        device.activity.spaceChains(device.chains)
        let seen = await frames(device) { $0.homeTitle == arcUnread }
        checkHonest(seen)
        #expect(seen.last?.homeTitle == arcUnread)
    }

    /// The event is the web's, field for field.
    @Test func theBalanceReadEventIsTheCoresShape() throws {
        var view = BalanceViewWire(
            address: Self.account, displayTotalUsd: 0, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: 1,
            networksRead: true, unreadChainIds: [Self.arc],
            tokens: [], unpricedTokens: [], failedChainIds: [Self.arc], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        let event = try CoreJSON.object(try #require(ActivityStore.balanceReadEvent(view)))
        #expect(event["type"] as? String == "balance_read")
        #expect(event["account"] as? String == Self.account)
        #expect(event["read"] as? Bool == true)
        #expect(event["unread_chain_ids"] as? [Int] == [Self.arc])
        // An older core's view (neither field): not read, nothing named.
        view.networksRead = nil
        view.unreadChainIds = nil
        let older = try CoreJSON.object(try #require(ActivityStore.balanceReadEvent(view)))
        #expect(older["read"] as? Bool == false)
        #expect(older["unread_chain_ids"] as? [Int] == [])
        // No account yet: nothing said.
        view = BalanceViewWire(
            address: nil, displayTotalUsd: nil, balanceUnknown: true, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [], unpricedTokens: [], failedChainIds: [], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        #expect(ActivityStore.balanceReadEvent(view) == nil)
        // The app connects the two (AppGraph).
        let graph = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/App/AppGraph.swift"), encoding: .utf8)
        #expect(graph.contains("Self.feedFollowsBalance(wallet: wallet, activity: activityStore)"))
    }

    // MARK: - r17-4 / r18-1: the network by its name, never its id

    private func feed(unread: [Int], key: String?, network: String?) throws -> FeedViewWire {
        let object: [String: Any] = [
            "rows": [], "home_rows": [], "transactions": [], "new_item_id": NSNull(), "toast": NSNull(),
            "account": Self.account.lowercased(), "settled": false, "unread_chains": unread,
            "unread_key": key.map { $0 as Any } ?? NSNull(), "unread_network": network.map { $0 as Any } ?? NSNull(),
        ]
        return try CoreJSON.decode(FeedViewWire.self, from: object)
    }

    @Test func theUnreadNetworkIsNamedByTheShellThenTheCoreElseCounted() throws {
        let decoded = try feed(unread: [Self.arc], key: "home.activityUnreadOne", network: "Arc Testnet")
        #expect(decoded.unreadNetwork == "Arc Testnet")
        // The shell's own list names it first.
        let own = WalletLive.activityUnreadLine(decoded, loc: loc, networks: .builtin)
        #expect(own == arcUnread)
        // A network the shell's list does not have: the core's name.
        let unknown = 999_999_001
        let named = try feed(unread: [unknown], key: "home.activityUnreadOne", network: "Somewhere Testnet")
        #expect(WalletLive.activityUnreadLine(named, loc: loc, networks: .builtin)
            == loc.t("home.activityUnreadOne", vars: ["network": "Somewhere Testnet"]))
        // Nobody can name it: the many-networks words, never "Chain ID 999999001".
        let nameless = try feed(unread: [unknown], key: "home.activityUnreadOne", network: nil)
        let line = try #require(WalletLive.activityUnreadLine(nameless, loc: loc, networks: .builtin))
        #expect(line == loc.t("home.activityUnreadMany", vars: ["n": "1"]), "\(line)")
        #expect(!line.contains("\(unknown)") && !line.contains("Chain ID"))
    }

    // MARK: - r18-2: the unread line over rows, drawn on both surfaces

    private func received(chainId: Int) -> [String: Any] {
        SignExecutor.recordRow([
            "record_id": "r19-row", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            "result": "", "from": Self.account.lowercased(), "chain_id": chainId,
            "now_ms": Date().timeIntervalSince1970 * 1000, "status": "confirmed",
            "user_op_hash": "0x" + String(repeating: "cd", count: 32),
            "dapp_origin": "http://127.0.0.1:9460", "dapp_url": "http://127.0.0.1:9460",
        ], nativeSymbol: "ETH")
    }

    /// The tester's run (Arc refused 110 s, an account with rows): the
    /// balance said "Can't reach Arc Testnet" and Activity said nothing.
    /// Through the real machines: the refused Arc joins the feed's unread
    /// set, and the line stands over the rows on the home and in History —
    /// and is drawn there.
    @Test func aRefusedNetworkIsSaidOverTheRowsOnTheHomeAndInHistory() async throws {
        let network = Network(.refused)
        let device = device(network, records: [received(chainId: 11_155_111)])
        defer { UserDefaults().removePersistentDomain(forName: device.suite) }
        open(device)
        let seen = await frames(device) { $0.homeNote == arcUnread && $0.historyNote == arcUnread }
        checkHonest(seen)
        let last = try #require(seen.last)
        #expect(last.homeMode == .rows)
        #expect(last.homeNote == arcUnread, "\(last.words)")
        #expect(last.historyMode == .rows)
        #expect(last.historyNote == arcUnread)

        // Drawn: the home's line over its rows.
        let view = try #require(device.wallet.balance)
        let feed = try #require(device.activity.feed)
        let model = WalletLive.apply(view, feed: feed, feedRead: true,
                                     on: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc,
                                     shownAccount: Self.account)
        let home = HostedScreen(
            WalletScreen(model: model, loc: loc, onToggleBalance: {}, onStatusTap: {}, onRefreshNow: {}).themed(.light)
        )
        defer { home.close() }
        let drawn = try await home.read { $0.element("home.activity.unread") != nil }
        #expect(drawn.element("home.activity.unread")?.label == arcUnread, "\(drawn.words)")
        try await home.save("r18-2-home-unread-over-rows")
    }

    // MARK: - r18-18: three causes, three sentences

    /// The index's answer to the creation-time member challenge, by what the
    /// transport did — as the executor hands it to the core.
    private func challengeFailure(_ transport: @escaping RegistryClient.Transport) async -> [String: Any] {
        let registry = RegistryClient(baseURL: "https://index.test", transport: transport)
        do {
            _ = try await OnboardingExecutor.bounded(seconds: 30) {
                try await registry.memberChallenge(groupPublicKey: "0x01", publicKey: "0x02", attestation: "", rpId: "getvela.app")
            }
            Issue.record("the challenge answered")
            return [:]
        } catch {
            return OnboardingExecutor.failure(for: ["type": "sign_member_proof"], error: error)
        }
    }

    /// The core's prompt for the key kept after `failure` (`create_wallet`:
    /// `refused` → `key_kept_refused`, its words the test build's; else
    /// `key_kept_unconfirmed(message, network)`), as the sheet says it.
    private func keptSheet(_ failure: [String: Any]) -> PromptCopy {
        var kind: [String: Any] = ["type": "create_failed", "detail": failure["message"] as? String ?? "", "key_kept": true]
        if failure["refused"] as? Bool == true {
            kind["words_key"] = "common.testBuild.productionWriteRefused"
        } else {
            kind["local"] = failure["network"] as? Bool ?? false
        }
        return promptCopy(PromptKind(json: kind), loc: loc)
    }

    /// The tester's r17-20(d) run: the index hung and the sheet said "This
    /// device is offline" (the request's own idle timeout, 15 s, was read as
    /// the device's). And the §28 guard's refusal was said "index didn't
    /// answer". Now: hung or not answering → the index's words; offline →
    /// the device's; refused by this test build → the test build's sentence.
    @Test func aHungIndexAnOfflineDeviceAndARefusedWriteAreThreeSentences() async throws {
        let index = loc.t(I18nKeys.Create.keyKeptBodyIndex)
        let offline = loc.t(I18nKeys.Create.keyKeptBodyNetwork)
        let refused = loc.t("common.testBuild.productionWriteRefused")
        #expect(Set([index, offline, refused]).count == 3, "three different sentences")

        // The index held the request open past its idle timeout.
        let hung = await challengeFailure { _ in throw URLError(.timedOut) }
        #expect(hung["type"] as? String == "index_failed")
        #expect(hung["network"] as? Bool == false, "a request that left and got no answer is the index's: \(hung)")
        #expect(keptSheet(hung).message == index)
        // Nobody listening at the index's address.
        let down = await challengeFailure { _ in throw URLError(.cannotConnectToHost) }
        #expect(keptSheet(down).message == index)
        // Bounded by the clock (r17-20): past it, the index's failure too.
        do {
            _ = try await OnboardingExecutor.bounded(seconds: 0.05) { () async throws -> String in
                try await Task.sleep(nanoseconds: 3_600 * 1_000_000_000)
                return ""
            }
        } catch {
            let past = OnboardingExecutor.failure(for: ["type": "sign_member_proof"], error: error)
            #expect(keptSheet(past).message == index)
        }

        // This device is offline: the request never left it.
        let away = await challengeFailure { _ in throw URLError(.notConnectedToInternet) }
        #expect(away["network"] as? Bool == true)
        #expect(keptSheet(away).message == offline)

        // This test build's guard refused the write to the live index.
        let guarded = await challengeFailure { _ in throw ProductionWriteRefused.error(host: "p256-index-v2.getvela.app") }
        #expect(guarded["refused"] as? Bool == true, "\(guarded)")
        #expect(guarded["network"] as? Bool == false)
        let sheet = keptSheet(guarded)
        #expect(sheet.message == refused, "\(sheet.message)")
        #expect(sheet.confirmLabel == nil, "no retry that cannot help")
        #expect(!sheet.message.contains("didn't answer") && !sheet.message.contains("offline"))
    }

    /// The other places the guard's refusal is handed to the core (§32): the
    /// publish's sync-failed screen (its sentence, no Retry Upload), the
    /// session's landing, the multisig service's write, and the multisig
    /// words for a refused record.
    @Test func everyRefusedWriteIsHandedOnAndSaidAsTheTestBuilds() async throws {
        let refused = loc.t("common.testBuild.productionWriteRefused")
        let error = ProductionWriteRefused.error(host: "safe-genesis.getvela.app")
        // The session's background publish.
        let landing = SessionExecutor.landingUnconfirmed(
            RegistryFailure(message: "Register: refused", network: false, refused: true))
        #expect(landing["type"] as? String == "registry_landing_unconfirmed")
        #expect(landing["refused"] as? Bool == true)
        #expect(SessionExecutor.landingUnconfirmed(URLError(.timedOut))["refused"] as? Bool == false)
        // The create's publish: the screen's sentence is the core's key.
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let probe = try decoder.decode(SyncKeyProbe.self, from: Data(#"{"sync_error_key":"common.testBuild.productionWriteRefused"}"#.utf8))
        #expect(probe.syncErrorKey == "common.testBuild.productionWriteRefused")
        let screen = HostedScreen(RetryScreen(loc: loc, detail: "Register: refused", sentenceKey: probe.syncErrorKey,
                                              busy: false, onRetry: {}, onStartOver: {}, onEditEndpoint: {}).themed(.light))
        defer { screen.close() }
        let seen = try await screen.read { $0.says(refused) }
        #expect(seen.says(refused), "\(seen.words)")
        #expect(!seen.says(loc.t(I18nKeys.Create.retryUploadBtn)), "no Retry Upload that cannot help: \(seen.words)")
        #expect(seen.says(loc.t(I18nKeys.Flow.editIndexEndpoint)), "the way on: a local index")
        try await screen.save("r18-18-sync-failed-refused")
        let views = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Onboarding/Core/CoreViews.swift"), encoding: .utf8)
        #expect(views.contains("syncErrorKey = try values.decodeIfPresent(String.self, forKey: .syncErrorKey)"))

        // The multisig service's write.
        #expect(TestBuildRefusal.matches(error))
        #expect(!TestBuildRefusal.matches(URLError(.cannotConnectToHost)))
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Multisig/Shell/MultisigExecutor.swift"), encoding: .utf8)
        #expect(source.contains(#"if TestBuildRefusal.matches(error) { failed["refused"] = true }"#))
        // A record this build refused to write.
        let words = MsWords(loc: loc, hub: nil)
        #expect(MultisigCreateSheet.sharedLine(shared: false, solo: false, refused: "production_write_refused",
                                               working: false, words: words) == refused)
        #expect(MultisigCreateSheet.sharedLine(shared: false, solo: false, refused: "refused",
                                               working: false, words: words) == words.createRefused)
    }

    private struct SyncKeyProbe: Decodable { let syncErrorKey: String? }

    // MARK: - r18-4: the chip and the body are one frame while it executes

    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one
    private static let two = MultisigViewsTests.two
    private static let three = MultisigViewsTests.three

    /// One frame of the proposal sheet's model: the proposals view over the
    /// item with `status`, and these fields in place of the empty ones.
    private func proposal(status: String, canApprove: [String] = [], replacing: [String: String] = [:],
                          oneTouch: [String] = []) throws -> MsProposalModel {
        let item = MultisigViewsTests.item(approved: 1, status: status, canApprove: canApprove, canExecute: [])
        var json = MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":1,"threshold":2}"#,
            extra: #","approve_execute":{"100:\#(Self.hash)":[\#(oneTouch.map { "\"\($0)\"" }.joined(separator: ","))]}"#
        )
        for (key, value) in replacing {
            json = json.replacingOccurrences(of: "\"\(key)\":{}", with: "\"\(key)\":\(value)")
        }
        let object = try JSONSerialization.jsonObject(with: Data(json.utf8))
        let view = try #require(MultisigMachine.decode(MsProposalsView.self, from: object))
        return MsProposalModel(item: try #require(view.items.first), view: view,
                               words: MsWords(loc: loc, hub: nil), account: nil)
    }

    private var working: String { #"{"execute:\#(Self.hash)":{"kind":"working"}}"# }

    /// The tester's r17-6 run, frame by frame (B's one press, "Approve and
    /// execute"): the press out; the mailbox's refresh calling it done (its
    /// nonce used) while this device's flight has no verdict yet — for
    /// ~1.2 s the chip read "1/2 approved" over a body listing only the one
    /// approval the mailbox counted, no buttons; the flight followed
    /// (`waiting`); the receipt judged. Every frame's chip and body agree:
    /// "Executing…" over the members as they stood, no count, no "Waiting"
    /// — until the receipt resolves, and then both move together.
    @Test func whileAnExecutionIsUnresolvedTheChipAndTheBodyNeverSayTheMailboxsCount() throws {
        let words = MsWords(loc: loc, hub: nil)
        let executing = words.t("status.executing")
        let base = try proposal(status: "pending")
        let counted = words.standing(base.standing, item: base.item)
        #expect(counted.contains("1"), "the mailbox's count: \(counted)")
        let frames: [(String, MsProposalModel)] = [
            ("press out", try proposal(status: "pending", canApprove: [Self.two],
                                       replacing: ["actions": working], oneTouch: [Self.two])),
            ("mailbox done, no verdict yet", try proposal(status: "done", replacing: ["actions": working])),
            ("flight followed", try proposal(status: "done", replacing: [
                "actions": working, "executions": #"{"100:\#(Self.hash)":{"kind":"waiting"}}"#,
            ])),
        ]
        let members = [Self.one, Self.two, Self.three]
        for (name, frame) in frames {
            #expect(frame.statusLine == executing, "\(name): the chip read \(frame.statusLine)")
            #expect(frame.statusLine != counted)
            #expect(frame.approvalsCount.isEmpty, "\(name): the heading's count \(frame.approvalsCount)")
            #expect(frame.listed.map { $0.lowercased() } == members.map { $0.lowercased() },
                    "\(name): every member, never the mailbox's stripped list: \(frame.listed)")
            for member in frame.listed {
                let word = frame.memberWord(member)
                #expect(word == .approved || word == .blank, "\(name): \(member) is \(word)")
            }
            #expect(frame.executedFacts == nil, "\(name)")
            #expect(frame.executedTx == nil)
            #expect(!frame.notes.contains { $0.id == "executed" || $0.id == "execFailed" }, "\(name)")
        }
        // The receipt judged: both say it ran, together.
        let ran = try proposal(status: "done", replacing: [
            "executions": #"{"100:\#(Self.hash)":{"kind":"executed","txHash":"0xabc"}}"#,
        ])
        #expect(ran.statusLine == words.t("proposal.executed"))
        #expect(ran.approvalsCount == words.t("proposal.executed"))
        #expect(!ran.executingHere)
        // Another member's execution ran first: resolved — the chip no longer
        // says "Executing…", whatever the action still says.
        let lost = try proposal(status: "done", replacing: [
            "actions": working, "executions": #"{"100:\#(Self.hash)":{"kind":"executedByOther"}}"#,
        ])
        #expect(!lost.executingHere)
        #expect(lost.statusLine != executing)
    }

    // MARK: - r18-5 (§32): the fee on screen at the passkey is the fee signed

    /// The tester's run: the one press carried 0.248201, the line re-quoted
    /// to 0.203525 before Face ID, and 0.248201775 was charged. The core
    /// (§32) no longer re-quotes the pressed executor's look while the press
    /// is out and serves it `signing: true`: the line is drawn from it, and
    /// marked as the figure being signed.
    @Test func thePressedFeeIsDrawnAndMarkedAsTheOneBeingSigned() throws {
        let words = MsWords(loc: loc, hub: nil)
        let look = #"{"kind":"priced","fee":{"amount":"248201775000000000"},"canPay":true,"balance":"500000000000000000","signing":true}"#
        let pressed = try proposal(status: "pending", canApprove: [Self.two], replacing: [
            "actions": working, "previews": #"{"100:\#(Self.hash):\#(Self.two.lowercased())":\#(look)}"#,
        ], oneTouch: [Self.two])
        let line = try #require(pressed.feeLine(for: Self.two))
        #expect(line.text.contains("0.248201"), "\(line.text)")
        let mark = try #require(pressed.feeSigning(for: Self.two))
        #expect(mark == words.t("proposal.feeSigning"))
        #expect(mark != "home.multisig.proposal.feeSigning", "the corpus has it")
        // Not pressed: no mark.
        let quiet = look.replacingOccurrences(of: #","signing":true"#, with: "")
        let idle = try proposal(status: "pending", canApprove: [Self.two], replacing: [
            "previews": #"{"100:\#(Self.hash):\#(Self.two.lowercased())":\#(quiet)}"#,
        ], oneTouch: [Self.two])
        #expect(idle.feeSigning(for: Self.two) == nil)
        #expect(idle.shownFee(for: Self.two) == "248201775000000000", "the press carries what is drawn")
    }

    // MARK: - r18-9: no fiat, and no currency, in the testnet space

    /// Every currency sign and code a money surface could draw.
    private static let currencySigns = ["$", "¥", "€", "£", "₩", "₹", "₽", "₺", "₫", "₱", "฿", "₪", "₦", "Rp", "zł"]
    private static let currencyCodes = ["USD", "CNY", "EUR", "JPY", "GBP", "KRW", "HKD", "TWD", "BRL", "MXN",
                                        "RUB", "TRY", "VND", "IDR", "INR", "AUD", "CAD", "CHF", "SGD"]

    private func checkNoFiat(_ strings: [String], _ surface: String, sourceLocation: SourceLocation = #_sourceLocation) {
        for text in strings where !text.isEmpty {
            for sign in Self.currencySigns {
                #expect(!text.contains(sign), "\(surface): \(sign) in \"\(text)\"", sourceLocation: sourceLocation)
            }
            // A code as a word ("CNY", "· USD"), never inside a ticker ("USDC").
            for code in Self.currencyCodes {
                #expect(text.range(of: "(?<![A-Za-z])\(code)(?![A-Za-z])", options: .regularExpression) == nil,
                        "\(surface): \(code) in \"\(text)\"", sourceLocation: sourceLocation)
            }
        }
    }

    private var cny: CurrencyViewWire {
        CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
    }

    /// A testnet account holding priced coins, its total known.
    private func pricedBalance() -> BalanceViewWire {
        BalanceViewWire(
            address: Self.account, displayTotalUsd: 12.5, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: 1, updatedAtMs: 1,
            networksRead: true, unreadChainIds: [],
            tokens: [
                BalanceTokenWire(chainId: Self.arc, symbol: "USDC", name: "USDC", balance: "12.5", decimals: 18,
                                 tokenAddress: nil, priceUsd: 1, spam: false),
            ],
            unpricedTokens: [], failedChainIds: [], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: 12.5,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    /// The tester's iOS frame: the testnet home's hero read "Total balance ·
    /// CNY" over the space's no-value words. The core's rule
    /// (`network_space_shows_fiat`): the testnet space shows no currency.
    @Test func theTestnetHeroNamesNoCurrency() async throws {
        #expect(!networkSpaceShowsFiat(space: "testnet"))
        #expect(networkSpaceShowsFiat(space: "mainnet"))
        let noValue = networkSpaceShowsFiat(space: "testnet") ? nil : loc.t("networkSpace.noValue")
        let model = WalletLive.apply(pricedBalance(), currency: cny, on: WalletFixtures.buildMobileState(.h1, loc: loc),
                                     loc: loc, noValue: noValue, shownAccount: Self.account)
        #expect(model.balance.state == .noValue)
        #expect(model.balance.currency.isEmpty, "the hero's label names \(model.balance.currency)")
        checkNoFiat([model.balance.label, model.balance.currency, model.balance.integer ?? "", model.balance.decimals ?? ""]
                    + model.assetRows.map { row in
                        if case .value(let figure) = row.fiat { return figure }
                        return ""
                    }, "home")
        let screen = HostedScreen(
            WalletScreen(model: model, loc: loc, onToggleBalance: {}, onStatusTap: {},
                         space: (label: loc.t("networkSpace.testnet"), testnet: true), onRefreshNow: {}).themed(.light)
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.says(loc.t("home.totalBalance")) }
        // A token's lettermark ("USD" for USDC) is its ticker, not a currency.
        checkNoFiat(seen.words.filter { word in !["USDC", "USDT"].contains { $0 != word && $0.hasPrefix(word) } },
                    "the drawn home")
        try await screen.save("r18-9-testnet-hero")
    }

    private func fiatText(_ fiat: AssetFiatModel) -> String {
        if case .value(let figure) = fiat { return figure }
        return ""
    }

    /// Every money surface, in the testnet space, as the app hands them their
    /// display (RootView's `spaceDisplay` / `spaceNoValue`, the core's
    /// `network_space_shows_fiat`): no currency sign and no currency code in
    /// anything drawn — and the same surfaces in the mainnet space DO price,
    /// so the check is not vacuous.
    @Test func noMoneySurfaceDrawsFiatInTheTestnetSpace() throws {
        let person = WalletLive.Display.from(cny, language: "en")
        let testnet = person.inSpace(.testnet)
        let mainnet = person.inSpace(.mainnet)
        #expect(NetworkSpaceKind.testnet.showsFiat == networkSpaceShowsFiat(space: "testnet"))
        #expect(!NetworkSpaceKind.testnet.showsFiat && NetworkSpaceKind.mainnet.showsFiat)
        #expect(mainnet.fiat(1) != nil, "the mainnet space prices")
        // The one formatter every surface goes through.
        #expect(testnet.fiat(1) == nil && testnet.fiatParts(1) == nil && !testnet.settled)
        #expect(testnet.fiatLine(1, prefix: "≈ ") == WalletLive.Display.withheldLine)
        let balance = pricedBalance()

        // The home hero and its rows.
        let noValue = NetworkSpaceKind.testnet.showsFiat ? nil : loc.t("networkSpace.noValue")
        let home = WalletLive.apply(balance, currency: cny, on: WalletFixtures.buildMobileState(.h1, loc: loc),
                                    loc: loc, noValue: noValue, shownAccount: Self.account)
        checkNoFiat([home.balance.label, home.balance.currency, home.balance.integer ?? "", home.balance.decimals ?? "",
                     home.balance.noValueText ?? ""] + home.assetRows.map { fiatText($0.fiat) }, "home")
        let mainHome = WalletLive.apply(balance, currency: cny, on: WalletFixtures.buildMobileState(.h1, loc: loc),
                                        loc: loc, shownAccount: Self.account)
        #expect(mainHome.balance.currency == "CNY", "mainnet: the hero names its currency")

        // Assets (all of them).
        guard case .assets(let drawnAssets) = WalletFlowFixtures.build(.t1, loc: loc).base else {
            Issue.record("no assets fixture"); return
        }
        let assets = FlowsLive.assets(balance, currency: cny, on: drawnAssets, loc: loc,
                                      noValue: !NetworkSpaceKind.testnet.showsFiat)
        checkNoFiat(assets.rows.map { fiatText($0.fiat) }, "assets")
        let mainAssets = FlowsLive.assets(balance, currency: cny, on: drawnAssets, loc: loc)
        #expect(mainAssets.rows.contains { !fiatText($0.fiat).isEmpty }, "mainnet: worth on a row")

        // The account switcher: its rows and total.
        let session = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.account,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Testnet A", "address": Self.account]]],
        ])
        let switcher = SettingsLive.withAccounts(
            session: session, balances: [BalanceCacheEntryWire(address: Self.account, usd: 12.5)],
            display: testnet, on: SettingsFixtures.build(.st1, loc: loc), loc: loc,
            noValue: !NetworkSpaceKind.testnet.showsFiat
        )
        checkNoFiat(switcher.accountsSheet.rows.map(\.amount) + [switcher.accountsSheet.summary], "switcher")

        // The token's own page: its worth and its price.
        guard case .tokenDetail(let drawnToken)? = WalletFlowFixtures.build(.t2, loc: loc).sheet else {
            Issue.record("no token fixture"); return
        }
        let token = FlowsLive.tokenDetail(balance.tokens[0], feed: nil, display: testnet, on: drawnToken, loc: loc)
        checkNoFiat([token.fiat] + token.facts.map(\.value), "token page")
        let mainToken = FlowsLive.tokenDetail(balance.tokens[0], feed: nil, display: mainnet, on: drawnToken, loc: loc)
        #expect(mainToken.fiat.contains("¥") || mainToken.fiat.contains("CN"), "mainnet: \(mainToken.fiat)")

        // An activity row (amounts in coins) and its detail's stored worth.
        let rows = WalletLive.activityGroups(try receivedFeed().rows, loc: loc, hidden: false).flatMap(\.rows)
        #expect(!rows.isEmpty)
        checkNoFiat(rows.map(\.amount) + rows.map(\.unit) + rows.map(\.subtitle), "activity rows")
        #expect(FlowsLive.storedFiat("$1.00", hidden: false, display: testnet) == WalletLive.Display.withheldLine)
        #expect(FlowsLive.storedFiat("$1.00", hidden: false, display: mainnet) == "≈ $1.00")

        // The browser's network picker's holdings.
        #expect(WalletLive.networkHoldings(balance, display: testnet).isEmpty)

        // Send (form, confirm, fee) and the signing sheet's fee: the send
        // machine is told no rate, and their lines go through the display.
        #expect(SendStore.displayContext(cny, space: .testnet).rate == nil)
        #expect(SendStore.displayContext(cny, space: .mainnet).rate == 7.1)

        // The multisig sheet's fee line: the coin, never fiat.
        let look = #"{"kind":"priced","fee":{"amount":"120000000000000000"},"canPay":true,"balance":"500000000000000000"}"#
        let sheet = try proposal(status: "pending", canApprove: [Self.two], replacing: [
            "previews": #"{"100:\#(Self.hash):\#(Self.two.lowercased())":\#(look)}"#,
        ], oneTouch: [Self.two])
        checkNoFiat([sheet.feeLine(for: Self.two)?.text ?? ""], "multisig fee line")

        // The app hands every one of them the space's display: no surface
        // reads the person's currency around the core's rule.
        let root = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/App/RootView.swift"), encoding: .utf8)
        #expect(root.components(separatedBy: "Display.live(settings.currency").count == 2,
                "one display, the space's (`spaceDisplay`)")
        #expect(root.contains("WalletLive.Display.live(settings.currency, language: loc.resolvedLanguage).inSpace(space.current)"))
        #expect(!root.contains("space.testnet ? loc.t(\"networkSpace.noValue\")"))
        #expect(!root.contains("noValue: space.current == .testnet"))
        #expect(root.contains("SendStore.displayContext(settings.currency, space: space.current)"))
        #expect(root.contains("noValue: !space.showsFiat"))
    }

    /// A feed holding one priced receipt (0.3 USDC, worth $0.30).
    private func receivedFeed() throws -> FeedViewWire {
        let row = #"""
        {"type":"header","id":"day-1","day_start_ms":1789948800000,"timestamp":1790000000},
        {"type":"item","item":{"id":"r1","direction":"in","counterparty":"0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D","alias":null,
          "value":"0.3","symbol":"USDC","decimals":6,"usd_value":0.3,"priced":true,"chain_id":5042002,"timestamp":1790000000,
          "day_start_ms":1789948800000,"tx_hash":"0x01","batch":null,"kind":"receive","status":"confirmed","site":null,
          "counterparty_role":"recipient","dapp":null,"subtitle":[],"figure_maskable":true}}
        """#
        let json = """
        {"rows":[\(row)],"home_rows":[\(row)],"transactions":[],"new_item_id":null,"toast":null,"hidden":false,
         "account":"\(Self.account.lowercased())","settled":true,"unread_chains":[],"unread_key":null}
        """
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(FeedViewWire.self, from: Data(json.utf8))
    }
}
#endif
