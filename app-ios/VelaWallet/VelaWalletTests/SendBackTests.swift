//
//  SendBackTests.swift
//  VelaWalletTests
//
//  087 F27, found on an iPhone 11: leaving Send with the back arrow did not
//  end the core's journey. The arrow only popped the shell's stack, `open` is
//  idempotent within a journey, and nothing but the CORE's close re-armed it —
//  so the next 转账 from the home resumed the abandoned journey: a pay link's
//  recipient (0xD400…130b) and its "Base only" scope on a brand-new send.
//
//  Driven through the real `SendStore`, its real executor and the real `send`
//  core, with the arrow asking the machine exactly as `RootView.flowBack`
//  does (`SendLive.back`).
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(3)))
struct SendBackTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    /// The founder's code: pay 0xD400…130b, on Base.
    private let payee = "0xD400866e00B055B20752a826CD5C89b811de130b"
    /// Scoped to Base, where this wallet holds nothing.
    private var payLink: String { "ethereum:\(payee)@8453" }
    /// The same ask on Gnosis, where it holds xDAI — so the code opens the form.
    private var gnosisLink: String { "ethereum:\(payee)@100" }

    // MARK: - Fixtures

    private func balance() throws -> BalanceViewWire {
        let json = """
        {
          "address": "\(me)",
          "display_total_usd": 0.5, "balance_unknown": false, "balance_partial": false,
          "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": 1,
          "tokens": [{
            "chain_id": 100, "symbol": "xDAI", "name": "xDAI",
            "balance": "0.5", "decimals": 18, "token_address": null,
            "price_usd": 1.0, "spam": false
          }],
          "unpriced_tokens": [],
          "failed_chain_ids": [], "rate_limited_chain_ids": [], "unreachable_networks": [],
          "holdings_loading": false, "cached_total_usd": null,
          "switcher": { "open": false, "loading": false, "balances": [] }
        }
        """
        return try CoreJSON.decode(BalanceViewWire.self, from: CoreJSON.object(json))
    }

    /// Gnosis, where the money is, and Base, where the code asks to be paid.
    private func networks() throws -> NetViewWire {
        func row(_ chainId: Int, _ name: String, _ symbol: String) -> String {
            """
            {
              "id": "chain-\(chainId)", "chain_id": \(chainId), "display_name": "\(name)",
              "native_symbol": "\(symbol)", "is_custom": false,
              "rpc_url": "", "explorer_url": "", "bundler_url": "",
              "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
              "rpc_save_deferred": false
            }
            """
        }
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [\(row(100, "Gnosis", "xDAI")), \(row(8453, "Base", "ETH"))],
          "wizard": {
            "phase": "idle", "query": "", "custom_rpc": "", "suggestions": [],
            "chain_info": null, "compat": null, "error": null, "can_add": false
          }
        }
        """
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(json))
    }

    private func store() throws -> SendStore {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts, offline: true)
        let accountPort = ScriptedAccounts()
        let held = try balance()
        let nets = try networks()
        return SendStore(executor: SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort,
            fees: FeeStore(relay: relay, accounts: accountPort, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: { held }, networks: { nets },
            holdingsRound: { _ in 0 }, openHoldings: { _ in },
            ports: SendExecutor.Ports()
        ))
    }

    /// 转账, as `RootView.openSend` sends it.
    private func open(_ send: SendStore) async {
        send.open(
            accountId: "cred-0", address: me, name: nil,
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
    }

    /// The header's arrow, asking the machine exactly as `RootView.flowBack`
    /// does.
    private func tapBack(_ send: SendStore) {
        guard let view = send.view else { return }
        switch SendLive.back(view) {
        case .step: send.back()
        case .leave: send.done()
        }
    }

    /// A code scanned, or a pay link opened: both reach the core as one
    /// `scan_resolved`, and a full request re-opens the journey locked —
    /// on the form when this wallet holds the asset asked for, on the
    /// picker scoped to the request's network when it does not (the screen
    /// the device showed: 选择代币 · 收款人 0xD400…130b · 仅支持 Base 网络付款).
    private func scan(_ send: SendStore, _ code: String) async {
        send.scanned(code)
        await Wait.until({ send.view?.resolvingLock == false }, orIdle: { send.isIdle })
    }

    private func same(_ a: String?, _ b: String) -> Bool {
        a?.caseInsensitiveCompare(b) == .orderedSame
    }

    /// What a brand-new send looks like: the picker, nobody, nothing locked.
    private func expectFresh(_ send: SendStore, _ why: Comment) {
        #expect(send.view?.stage == .selectToken, why)
        #expect(send.view?.recipient == "", why)
        #expect(send.view?.locked == false, why)
        #expect(send.view?.lockError == nil, why)
        #expect(send.view?.selectedToken == nil, why)
    }

    // MARK: - (1) Back from the picker ends the journey

    @Test func backFromThePickerEndsTheJourneyAndTheNextOpenIsFresh() async throws {
        let send = try store()
        await open(send)
        // Something to leave behind: a pay link, scoped to Base.
        await scan(send, payLink)
        #expect(same(send.view?.recipient, payee))
        #expect(send.view?.locked == true)

        let picker = try #require(send.view)
        #expect(SendLive.back(picker) == .step, "the picker's arrow is the machine's back")
        tapBack(send)
        await Wait.until { send.closes == 1 }

        // The flow goes with the journey — only while a send screen is up.
        let flows = FlowNav()
        flows.enter(.send)
        flows.close(ifShowing: SendLive.flowStates)
        #expect(!flows.isOpen, "the core closed the journey and the send screens stayed up")
        flows.enter(.activity)
        flows.close(ifShowing: SendLive.flowStates)
        #expect(flows.top == .a1, "a close must not shut a flow the person went to since")

        // The next 转账 is a new journey, not the old one resumed.
        await open(send)
        expectFresh(send, "the next 转账 resumed the journey Back had ended")
        #expect(send.view?.tokens.isEmpty == false, "the new journey never loaded its holdings")
    }

    // MARK: - (2) Back from the form goes to the picker

    @Test func backFromTheFormGoesToThePickerKeepingTheScannedRecipient() async throws {
        let send = try store()
        await open(send)
        await scan(send, gnosisLink)
        let form = try #require(send.view)
        #expect(form.stage == .enterDetails, "a code for a held asset opens the form")
        #expect(same(form.recipient, payee))
        #expect(form.selectedToken?.chainId == 100)
        #expect(SendLive.back(form) == .step)

        tapBack(send)

        // The CORE's back: the picker, not the home — and the recipient that
        // was handed in survives the trip (#332).
        #expect(send.view?.stage == .selectToken, "Back from the form did not land on the picker")
        #expect(send.view.map { SendLive.flowState($0, feeSheetOpen: false) } == .sd1)
        #expect(same(send.view?.recipient, payee), "the scanned recipient was thrown away")
        #expect(send.view?.selectedToken == nil)
        try await Task.sleep(nanoseconds: 100_000_000)
        #expect(send.closes == 0, "a step back from the form closed the whole journey")
    }

    /// A recipient the person TYPED does not survive the way back — the
    /// core's rule, and the reason `RootView.flowBack` re-reads the field.
    @Test func backFromTheFormClearsATypedRecipient() async throws {
        let send = try store()
        await open(send)
        let token = try #require(send.view?.tokens.first?.id)
        send.selectToken(id: token)
        await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
        send.setRecipient(payee)
        #expect(same(send.view?.recipient, payee))

        tapBack(send)
        #expect(send.view?.stage == .selectToken)
        #expect(send.view?.recipient == "", "the field would show an address the machine dropped")
    }

    // MARK: - (3) A pay link left by Back does not reach the next send

    /// The device's own steps: a Base pay link lands on the picker scoped to
    /// Base with 0xD400…130b on it; Back; the home's 转账.
    @Test func aPayLinkJourneyLeftByBackDoesNotLeakIntoTheNextSend() async throws {
        let send = try store()
        await open(send)
        await scan(send, payLink)
        let picker = try #require(send.view)
        #expect(picker.stage == .selectToken)
        #expect(picker.locked, "the code's network scope")
        #expect(same(picker.recipient, payee))
        #expect(SendLive.back(picker) == .step)

        tapBack(send) // the picker: the core closes the journey
        await Wait.until { send.closes == 1 }

        await open(send) // the home's 转账
        expectFresh(send, "a pay link's recipient and network scope came back on a new send")
    }

    /// The same, from the form: Back, Back, 转账.
    @Test func aPayLinkFormLeftByBackTwiceDoesNotLeakIntoTheNextSend() async throws {
        let send = try store()
        await open(send)
        await scan(send, gnosisLink)
        #expect(send.view?.stage == .enterDetails)

        tapBack(send) // form → picker, still locked, still to 0xD400…
        #expect(send.view?.stage == .selectToken)
        #expect(send.view?.locked == true)
        try await Task.sleep(nanoseconds: 100_000_000)
        #expect(send.closes == 0)
        tapBack(send) // picker → the core closes the journey
        await Wait.until { send.closes == 1 }

        await open(send)
        expectFresh(send, "a pay link's recipient came back on a new send")
    }

    /// The same journey, left by a door the core never hears — another 转账,
    /// a link, a contact. The guard that keeps a rebuild from resetting the
    /// form is exactly what resurrected it, so every entry leaves first.
    @Test func aJourneyLeftByAnotherDoorDoesNotComeBack() async throws {
        let send = try store()
        await open(send)
        await scan(send, gnosisLink)
        #expect(send.view?.stage == .enterDetails)

        // Why `leave` exists: a second `open` mid-journey is dropped.
        await open(send)
        #expect(send.view?.locked == true, "the guard is what keeps a rebuild from resetting the form")

        send.leave()
        // At once — the screen entering next must not draw the journey just
        // left while the real `open` waits on the account store.
        expectFresh(send, "the journey just left was still the machine's view")

        await open(send)
        expectFresh(send, "the next 转账 resumed the abandoned pay link")
        #expect(send.view?.tokens.isEmpty == false)
        #expect(send.closes == 0, "leaving is the shell's, not a close the flow host acts on")
    }

    /// Leaving before the machine ever booted is nothing to forget.
    @Test func leavingBeforeTheFirstOpenIsHarmless() async throws {
        let send = try store()
        send.leave()
        #expect(send.view == nil)
        await open(send)
        expectFresh(send, "the first open after a leave did not boot the machine")
        #expect(send.view?.tokens.isEmpty == false)
    }

    // MARK: - The stages that leave rather than step

    /// A request this wallet cannot honour is drawn AS the picker, and one tap
    /// leaves it — the machine's `back` would move between steps nobody sees.
    @Test func aRequestThisWalletCannotHonourLeavesInOneTap() async throws {
        let send = try store()
        await open(send)
        send.scanned("ethereum:\(payee)@424242")
        await Wait.until({ send.view?.stage == .lockError }, orIdle: { send.isIdle })
        let view = try #require(send.view)
        #expect(view.stage == .lockError)
        #expect(SendLive.back(view) == .leave)

        tapBack(send)
        await Wait.until { send.closes == 1 }
        await open(send)
        expectFresh(send, "a refused request survived its own Back")
    }

    @Test func eachStageAsksTheMachineTheRightThing() throws {
        func view(_ stage: String) throws -> SendViewWire {
            let object = try CoreJSON.object(SendCore().view()).merging(["stage": stage]) { $1 }
            return try CoreJSON.decode(SendViewWire.self, from: object)
        }
        #expect(SendLive.back(try view("select_token")) == .step)
        #expect(SendLive.back(try view("enter_details")) == .step)
        #expect(SendLive.back(try view("confirm")) == .step)
        #expect(SendLive.back(try view("receipt")) == .leave)
        #expect(SendLive.back(try view("lock_resolving")) == .leave)
        #expect(SendLive.back(try view("lock_error")) == .leave)
    }

    // MARK: - Send's "add this network" answers at once (final notes F6/F27)

    /// A code for a chain this wallet does not have. Nothing on this shell
    /// raises `add_network_tapped` (a known gap: the notice draws no "Add
    /// this network"), but if the core is ever asked, the answer comes AT
    /// ONCE — never a ten-second wait — and the form says the core's own
    /// sentence for an add that did not happen, over the lock it could not
    /// lift.
    @Test func anAddNetworkAskIsAnsweredAtOnceInTheCoresSentence() async throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let send = try store()
        await open(send)
        await scan(send, "ethereum:\(payee)@480")
        let locked = try #require(send.view)
        #expect(locked.lockError == .network(chainId: 480), "\(String(describing: locked.lockError))")
        #expect(SendLive.lockNotice(locked, loc: loc)?.text.hasPrefix("Network not supported") == true)

        let asked = ContinuousClock.now
        send.dispatch(["type": "add_network_tapped", "chain_id": 480])
        await Wait.until({ send.view?.addNetworkMsg != nil }, orIdle: { send.isIdle })
        let took = ContinuousClock.now - asked
        let answered = try #require(send.view)
        #expect(answered.addNetworkMsg == .netAddError)
        #expect(!answered.addingNetwork, "the form is still \"adding\"")
        #expect(took < .seconds(1), "the add-network ask took \(took)")
        #expect(SendLive.lockNotice(answered, loc: loc)?.text == "Couldn't add the network. Please try again.")
        // The lock stands — the person is not sent on to an empty form.
        #expect(answered.lockError == .network(chainId: 480))
    }
}
