//
//  SendDisplayFollowsTests.swift
//  VelaWalletTests
//
//  PR 3 final note F25 — an open Send journey hears the display currency.
//
//  `SendStore.displayChanged` had no caller. The send machine was told the
//  display once, at `open`, and kept it for the whole journey: a Send opened
//  in a wallet's first seconds — before the currency committed — stayed on
//  the USD/1 placeholder it was opened with, and one left open across a
//  change in Settings kept the old currency's rate.
//
//  Driven through the real `SendStore`, its real executor and the real `send`
//  core. The machine's own view is the witness: whether ⇄ may enter fiat, and
//  which currency a typed figure is counted in.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(3)))
struct SendDisplayFollowsTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

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

    private func networks() throws -> NetViewWire {
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [{
            "id": "chain-100", "chain_id": 100, "display_name": "Gnosis",
            "native_symbol": "xDAI", "is_custom": false,
            "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
            "rpc_save_deferred": false
          }],
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

    private let waiting = CurrencyViewWire(code: "USD", rate: 1, committed: false, pending: "CNY")
    private let cny = CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
    private let eur = CurrencyViewWire(code: "EUR", rate: 0.9, committed: true)

    /// 转账, as `RootView.openSend` sends it under `currency`, on to the form
    /// with xDAI picked.
    private func open(_ send: SendStore, under currency: CurrencyViewWire?) async throws {
        let display = SendStore.displayContext(currency)
        send.open(accountId: "cred-0", address: me, name: nil,
                  displayCode: display.code, displayRate: display.rate, fiatDecimals: 2)
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
        let token = try #require(send.view?.tokens.first)
        send.selectToken(id: token.id)
        await Wait.until({ send.view?.selectedToken != nil }, orIdle: { send.isIdle })
    }

    /// As `RootView.sendDisplayStands` says it when the currency view moves.
    private func stands(_ send: SendStore, _ currency: CurrencyViewWire?) async {
        let display = SendStore.displayContext(currency)
        send.displayStands(code: display.code, rate: display.rate, fiatDecimals: 2)
        await Wait.until({ false }, orIdle: { send.isIdle })
    }

    // MARK: - The rule

    /// One rule for `open` and every change after it: the committed pair as
    /// the hero prints it; before that, the code on its way and NO rate —
    /// never the USD/1 placeholder.
    @Test func theMachineIsToldTheCommittedPairOrNoRateAtAll() {
        #expect(SendStore.displayContext(cny).code == "CNY" && SendStore.displayContext(cny).rate == 7.1)
        #expect(SendStore.displayContext(waiting).code == "CNY" && SendStore.displayContext(waiting).rate == nil)
        let first = CurrencyViewWire(code: "USD", rate: 1, committed: false)
        #expect(SendStore.displayContext(first).code == "USD" && SendStore.displayContext(first).rate == nil,
                "the placeholder's rate reached the machine")
        #expect(SendStore.displayContext(.unread).rate == nil)
        #expect(SendStore.displayContext(nil).rate == nil)
        // A committed choice nobody could price is dollars, as on the hero.
        let unpriced = CurrencyViewWire(code: "CNY", rate: nil, committed: true)
        #expect(SendStore.displayContext(unpriced).code == "USD" && SendStore.displayContext(unpriced).rate == 1)
    }

    // MARK: - The journey

    /// A Send opened before the currency committed cannot be flipped to
    /// typing money — and when the pair lands the open journey hears it: ⇄
    /// enters fiat, in the person's currency.
    @Test func aSendOpenedBeforeTheCurrencyCommitsHearsTheCommit() async throws {
        let send = try store()
        try await open(send, under: waiting)
        #expect(send.view?.denomToggleShown == true)
        #expect(send.view?.denomToggleEnabled == false, "fiat could be entered at the placeholder's rate")
        send.toggleFiatInput()
        await Wait.until({ false }, orIdle: { send.isIdle })
        #expect(send.view?.amountFiatCode == nil, "a figure is counted in a currency nobody chose")

        await stands(send, cny)
        #expect(send.view?.denomToggleEnabled == true, "the commit never reached the open journey")
        send.toggleFiatInput()
        await Wait.until({ send.view?.amountFiatCode != nil }, orIdle: { send.isIdle })
        #expect(send.view?.amountFiatCode == "CNY")
    }

    /// The currency changed in Settings while a figure typed in the old one
    /// is on the form: the machine re-denominates by its own rule — the
    /// field is the new currency's, and empty, never ¥ digits read as €.
    @Test func aChangeUnderATypedFigureReDenominates() async throws {
        let send = try store()
        try await open(send, under: cny)
        send.toggleFiatInput()
        await Wait.until({ send.view?.amountFiatCode == "CNY" }, orIdle: { send.isIdle })
        send.setAmount("2")
        await Wait.until({ send.view?.amount == "2" }, orIdle: { send.isIdle })
        #expect(send.view?.amountFiatCode == "CNY" && send.view?.amount == "2")

        await stands(send, eur)
        #expect(send.view?.amountFiatCode == "EUR", "the journey kept the currency it was opened with")
        #expect(send.view?.amount.isEmpty == true, "2 CNY was relabelled 2 EUR: \(send.view?.amount ?? "-")")
    }

    /// The same pair said twice reaches the machine once, and a currency
    /// moving with no journey open reaches nobody.
    @Test func itIsSaidOncePerChangeAndOnlyToAnOpenJourney() async throws {
        let send = try store()
        // No journey: nothing to tell, and nothing boots.
        await stands(send, cny)
        #expect(send.view == nil, "a display change booted the send machine")

        try await open(send, under: cny)
        send.toggleFiatInput()
        await Wait.until({ send.view?.amountFiatCode == "CNY" }, orIdle: { send.isIdle })
        send.setAmount("3")
        await Wait.until({ send.view?.amount == "3" }, orIdle: { send.isIdle })
        // The pair it was opened with, again: the figure stands.
        await stands(send, cny)
        #expect(send.view?.amount == "3" && send.view?.amountFiatCode == "CNY")

        // The journey left: the next change is nobody's.
        send.leave()
        await Wait.until({ false }, orIdle: { send.isIdle })
        let before = send.view
        await stands(send, eur)
        #expect(send.view == before, "a display change reached a journey that was left")
    }

    /// While the currency is on its way the form says no "can't convert":
    /// the machine has no rate because there is none YET — a wait of a
    /// second or two, not a refusal whose line comes, goes and moves the
    /// form both times. Once committed, the reason is the core's again.
    @Test func noRefusalIsSaidWhileTheCurrencyIsOnItsWay() async throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let send = try store()
        try await open(send, under: waiting)
        let view = try #require(send.view)
        #expect(view.denomToggleReason != nil, "the core gives its reason for an inert ⇄")
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("the drawn send form is missing")
            return
        }
        let onItsWay = SendLive.form(view, fee: nil, display: .from(waiting), on: drawn, loc: loc)
        #expect(onItsWay.amount?.denomReason == nil, "a refusal is said for a currency on its way")
        #expect(onItsWay.amount?.denomEnabled == false)
        // A committed currency that has no rate says so.
        let settled = SendLive.form(view, fee: nil, display: .usd, on: drawn, loc: loc)
        #expect(settled.amount?.denomReason != nil)
    }
}
