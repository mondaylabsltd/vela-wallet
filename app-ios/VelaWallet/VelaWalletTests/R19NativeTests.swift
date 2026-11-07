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
    /// or answer them.
    final class Network: @unchecked Sendable {
        enum Arc { case hung, refused, answers }
        private let lock = NSLock()
        private var _arc: Arc
        init(_ arc: Arc) { _arc = arc }
        var arc: Arc {
            get { lock.withLock { _arc } }
            set { lock.withLock { _arc = newValue } }
        }

        func read(_ address: String, _ chainId: Int) async -> TokenReads.ChainResult {
            guard chainId == R19NativeTests.arc else {
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

    private func device(_ network: Network, records: [[String: Any]] = [], deadlineMs: UInt32 = 3_000) -> Device {
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
        let device = device(network)
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
}
#endif
