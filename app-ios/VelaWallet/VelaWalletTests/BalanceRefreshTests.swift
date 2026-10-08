//
//  BalanceRefreshTests.swift
//  VelaWalletTests
//
//  Issue 462: the hero's "↻ Updated 2m" control. A tap reads every chain
//  again (`RefreshRequested{force, pull}`), the glyph turns and says
//  "Updating…" until that round settles — and for at least 650 ms, so a fast
//  read still shows that it happened — and the grey "still updating" line no
//  longer appears above it to push it out from under the finger.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BalanceRefreshTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    private func view(
        refreshing: Bool = false, at: Double? = nil, notice: BalanceNoticeWire? = nil
    ) -> BalanceViewWire {
        BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: 10, balanceUnknown: false, balancePartial: false,
            notice: notice, hidden: false, refreshing: refreshing, lastRefreshedAtMs: at,
            tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    // MARK: - The spin

    /// The tap starts the spin at once, a second tap is refused while it
    /// turns, and it stops only when the core is done AND 650 ms have passed.
    @Test func aTapSpinsUntilTheRoundEndsAndAtLeast650Ms() throws {
        let idle = RefreshSpin()
        #expect(!idle.spinning)
        let tapped = try #require(idle.tapped(now: 1_000))
        #expect(tapped.spinning)
        #expect(tapped.tapped(now: 1_010) == nil, "a second tap while turning dispatches nothing")

        // The core says its round is out, then answers in 100 ms: the spin
        // holds to the minimum anyway.
        let busy = tapped.core(busy: true, now: 1_005)
        #expect(busy.releaseAt == nil, "never released while the core is busy")
        #expect(busy.settle(now: 9_999).spinning)
        let answered = busy.core(busy: false, now: 1_100)
        #expect(answered.releaseAt == 1_000 + RefreshSpin.minimumMs)
        #expect(answered.settle(now: 1_600).spinning, "a fast read still turns for 650 ms")
        let done = answered.settle(now: 1_650)
        #expect(!done.spinning)
        #expect(done.tapped(now: 1_700) != nil, "tappable again once it stopped")
    }

    /// A slow round keeps it turning past the minimum, for as long as the
    /// core says the refresh is out.
    @Test func aSlowRoundTurnsUntilItSettles() throws {
        let busy = try #require(RefreshSpin().tapped(now: 0)).core(busy: true, now: 10)
        #expect(busy.settle(now: 5_000).spinning)
        let settled = busy.core(busy: false, now: 5_000)
        #expect(!settled.settle(now: 5_000).spinning, "past the minimum, it stops with the round")
    }

    /// A refresh the shell did not start itself — the pull, a rescue sheet's
    /// retry — turns the control too, from when the core said so.
    @Test func aPullsRefreshTurnsTheControlToo() {
        let pulled = RefreshSpin().core(busy: true, now: 2_000)
        #expect(pulled.spinning)
        #expect(pulled.tapped(now: 2_100) == nil)
        let answered = pulled.core(busy: false, now: 2_050)
        #expect(answered.settle(now: 2_649).spinning)
        #expect(!answered.settle(now: 2_650).spinning)
    }

    /// A tap the core never answered (a machine not booted yet) still stops.
    @Test func aTapTheCoreNeverHeardStillStops() throws {
        let tapped = try #require(RefreshSpin().tapped(now: 0))
        #expect(tapped.releaseAt == RefreshSpin.minimumMs)
        #expect(!tapped.settle(now: RefreshSpin.minimumMs).spinning)
    }

    // MARK: - The event

    /// The REAL core, handed the very event the control sends: a person's
    /// refresh, past the background gate, which the core marks as theirs —
    /// `refreshing` is what turns the control, so it must come back true.
    @Test func theTapsEventIsAPersonsRefreshTheCoreHolds() throws {
        let core = BalanceDashboardCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "privacy_hydrated", "hidden": false]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "account_changed", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ]))
        let event = try #require(
            try JSONSerialization.jsonObject(with: Data(WalletStore.refreshEvent(pull: true).utf8))
                as? [String: Any]
        )
        #expect(event["type"] as? String == "refresh_requested")
        #expect(event["force"] as? Bool == true)
        #expect(event["pull"] as? Bool == true)
        let answer = try CoreJSON.object(core.dispatch(eventJson: WalletStore.refreshEvent(pull: true)))
        let viewObject = try #require(answer["view"] as? [String: Any])
        let view = try CoreJSON.decode(BalanceViewWire.self, from: viewObject)
        #expect(view.refreshing, "the core holds a person's refresh until its round ends")
    }

    // MARK: - The model

    /// "上次更新 · 2分钟前": the core's last read, in the core's relative words,
    /// and "Updating…" ready beside it so the control can reserve its width.
    @Test func theControlSaysWhenTheFigureWasRead() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let nowMs = now.timeIntervalSince1970 * 1000
        let model = WalletLive.refresh(view(at: nowMs - 120_000), loc: loc, now: now)
        #expect(model.updated == loc.t("home.lastUpdated", vars: ["ago": "2分钟前"]))
        #expect(model.updated?.contains("2分钟前") == true)
        #expect(model.updating == loc.t("home.updating"))
        #expect(!model.updating.isEmpty && model.updating != "home.updating", "the corpus has the key")
        #expect(!model.refreshing)

        let english = WalletLive.refresh(view(at: nowMs - 120_000), loc: en, now: now)
        #expect(english.updated == "Updated 2m")
        #expect(english.updating == "Updating…")

        let never = WalletLive.refresh(view(), loc: loc, now: now)
        #expect(never.updated == nil, "never read: the glyph alone, no made-up time")
    }

    /// The label ages: the same read, later, says so.
    @Test func theLabelAges() {
        let at = 1_800_000_000_000.0
        func said(after seconds: Double) -> String {
            RelativeTime.ago(atMs: at, nowMs: at + seconds * 1000, loc: en)
        }
        #expect(said(after: 0) == "now")
        #expect(said(after: 44) == "now")
        #expect(said(after: 45) == "1m")
        #expect(said(after: 150) == "3m", "minutes round half away from zero, as the core's do")
        #expect(said(after: 3_599) == "60m")
        #expect(said(after: 3_600) == "1h")
        #expect(said(after: 5 * 3_600 + 1_800) == "6h")
        #expect(said(after: -30) == "now", "a clock behind the read is never negative")
        #expect(said(after: 2 * 86_400) == Formats.date(Date(timeIntervalSince1970: at / 1000)))
    }

    /// The control turns while the core's refresh is out — or while the
    /// store's held spin says so, which outlasts a fast read.
    @Test func theControlTurnsWithTheHeldSpin() {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        let core = WalletLive.apply(view(refreshing: true, at: 1), on: drawn, loc: loc)
        #expect(core.balance.refresh?.refreshing == true)
        let held = WalletLive.apply(view(refreshing: false, at: 1), on: drawn, loc: loc, spinning: true)
        #expect(held.balance.refresh?.refreshing == true, "the 650 ms hold outlasts the core's flag")
        let idle = WalletLive.apply(view(refreshing: true, at: 1), on: drawn, loc: loc, spinning: false)
        #expect(idle.balance.refresh?.refreshing == false)
    }

    /// The line above the control no longer comes and goes with a refresh the
    /// person asked for — that pushed the control out from under the finger.
    /// The core's own "still updating" notice still says it.
    @Test func aPersonsRefreshPutsNoLineAboveTheControl() {
        let base = WalletFixtures.buildMobileState(.h1, loc: loc).balance
        #expect(WalletLive.balance(view(refreshing: true), fallback: base, loc: loc).status == nil)
        let still = WalletLive.balance(view(notice: .stillUpdating), fallback: base, loc: loc)
        #expect(still.status?.kind == .refreshing)
        #expect(still.status?.text == loc.t("home.balanceStale"))
    }

    /// The gallery draws the control idle ("2m ago"), and the first read's
    /// skeleton (H3) as the glyph alone.
    @Test func theFixturesDrawTheControl() {
        let h1 = WalletFixtures.buildMobileState(.h1, loc: loc).balance.refresh
        #expect(h1?.updated == loc.t("home.lastUpdated", vars: ["ago": loc.t("time.minutesShort", vars: ["n": "2"])]))
        #expect(h1?.refreshing == false)
        let h3 = WalletFixtures.buildMobileState(.h3, loc: loc).balance.refresh
        #expect(h3 != nil && h3?.updated == nil)
    }
}
