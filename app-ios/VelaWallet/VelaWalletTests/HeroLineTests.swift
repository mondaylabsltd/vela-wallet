//
//  HeroLineTests.swift
//  VelaWalletTests
//
//  The hero's status and its refresh control are ONE line (device walks
//  2026-10-10 / 10-11: a lone ⟳ sat on a line of its own under 「正在检查…」,
//  and once the read settled an empty line stood between the figure and
//  「⟳ 上次更新 · 刚刚」).
//
//  The real `WalletScreen`, hosted, under the views the REAL balance core
//  writes for each state the hero has — the first read out, read, a refresh
//  the person asked for, a network failed, every network out of reach,
//  hidden, the empty wallet's live line — with the tree an assistive client
//  gets read for where the label, the figure, the line, the control and the
//  first row under the hero stand.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct HeroLineTests {
    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    /// What stands where, in one state.
    struct Places: Equatable {
        var label: CGFloat
        var figure: CGFloat
        var line: CGFloat
        var lineHeight: CGFloat
        var control: CGRect
        var under: CGFloat
        /// From the label's top to the end of the hero's last line.
        var hero: CGFloat
    }

    static let states = ["checking", "updated", "updating", "failed", "offline", "hidden", "live"]

    /// The hero for `state`, from the real core's view.
    private func hero(_ state: String, loc: Loc) throws -> BalanceModel {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        let now = Date()
        func built(_ view: BalanceViewWire, spinning: Bool? = nil) -> BalanceModel {
            WalletLive.apply(view, on: drawn, loc: loc, now: now, spinning: spinning).balance
        }
        switch state {
        case "checking":
            return built(try #require(BalanceCoreScene.zeroWallet()).checking)
        case "live":
            return built(try #require(BalanceCoreScene.zeroWallet()).settled)
        case "updated":
            return built(try #require(BalanceCoreScene.view(internalFault: false, healthy: true)))
        case "updating":
            return built(try #require(BalanceCoreScene.view(internalFault: false, healthy: true)), spinning: true)
        case "failed":
            return built(try #require(BalanceCoreScene.view(internalFault: false)))
        case "offline":
            return built(try #require(BalanceCoreScene.view(internalFault: false, everyChain: true)))
        default:
            // Hidden: the read wallet with its figure withheld, as
            // `WalletLive.balance` draws a hidden view.
            let shown = built(try #require(BalanceCoreScene.view(internalFault: false, healthy: true)))
            var hidden = BalanceModel(
                label: shown.label, currency: shown.currency, state: .hidden, integer: nil, decimals: nil,
                liveText: nil, status: shown.status, a11yHide: shown.a11yHide, a11yShow: shown.a11yShow
            )
            hidden.refresh = shown.refresh
            return hidden
        }
    }

    private func home(_ balance: BalanceModel, loc: Loc) -> some View {
        var model = WalletFixtures.buildMobileState(.h1, loc: loc)
        model.balance = balance
        return WalletScreen(model: model, loc: loc, onToggleBalance: {}, onStatusTap: {}, onRefreshNow: {})
            .themed(.light)
    }

    /// Where things stand under `balance`.
    private func places(
        _ seen: HostedScreen.Seen, _ balance: BalanceModel, loc: Loc
    ) throws -> Places {
        let receiveLabel = WalletFixtures.buildMobileState(.h1, loc: loc).actions.receive
        let label = try #require(seen.tree.first { $0.label.hasPrefix(balance.label) }, "the hero's label")
        let figure: HostedScreen.Element
        switch balance.state {
        case .hidden:
            figure = try #require(seen.labelled(balance.a11yShow), "the hidden figure")
        case .loading:
            // The skeleton says nothing; its line is the label's next.
            figure = label
        default:
            figure = try #require(
                seen.tree.first { $0.label.hasPrefix(balance.integer ?? "~") && $0.frame.height > 30 },
                "the figure \(balance.integer ?? "")"
            )
        }
        let control = try #require(seen.element(BalanceRefreshControl.testId), "the refresh control")
        // What the line says, if it says anything.
        let said = balance.checkingText ?? balance.status?.text ?? balance.liveText
        let line = said.flatMap { text in seen.tree.first { $0.label == text } }
        let receive = try #require(seen.labelled(receiveLabel), "Receive, the first row under the hero")
        return Places(
            label: label.frame.minY,
            figure: figure.frame.minY,
            line: line?.frame.midY ?? control.frame.midY,
            lineHeight: line?.frame.height ?? 0,
            control: control.frame,
            under: receive.frame.minY,
            hero: receive.frame.minY - Tokens.Space.s24 - label.frame.minY
        )
    }

    private func measure(_ size: CGSize, loc: Loc) async throws -> [String: Places] {
        var out: [String: Places] = [:]
        let first = try hero(Self.states[0], loc: loc)
        let hosted = HostedScreen(home(first, loc: loc), size: size)
        defer { hosted.close() }
        for state in Self.states {
            let balance = try hero(state, loc: loc)
            let seen = try await hosted.show(home(balance, loc: loc)) {
                $0.element(BalanceRefreshControl.testId) != nil
            }
            try await hosted.save("hero-\(loc.resolvedLanguage)-\(Int(size.width))-\(state)")
            let at = try places(seen, balance, loc: loc)
            out[state] = at
            print("MEASURE hero \(loc.resolvedLanguage) \(Int(size.width))x\(Int(size.height)) \(state): hero \(at.hero) figure \(at.figure - at.label) line \(at.line - at.label) control \(at.control.minX),\(at.control.minY - at.label) \(at.control.width)x\(at.control.height) under \(at.under - at.label)")
        }
        return out
    }

    /// One height, one place for everything, in every state — on the
    /// 390-wide phone and the SE, in English and Chinese — and ONE line: what
    /// the line says and the control stand on the same row, the control at
    /// its end.
    @Test(arguments: [HostedScreen.phone, HostedScreen.se])
    func theHeroIsOneHeightAndNothingMovesBetweenItsStates(size: CGSize) async throws {
        for loc in [en, zh] {
            let all = try await measure(size, loc: loc)
            let base = try #require(all["updated"])
            for state in Self.states {
                let at = try #require(all[state])
                #expect(at.hero == base.hero, "\(state): the hero is \(at.hero) tall, not \(base.hero)")
                #expect(at.label == base.label)
                #expect(at.under == base.under, "\(state): the first row under the hero moved \(at.under - base.under)")
                #expect(at.control == base.control, "\(state): the refresh control is at \(at.control), not \(base.control)")
                // The figure, where there is one to read (not the skeleton,
                // not the mask — which keep its line, `BalanceHeroA11yTests`).
                if ["checking", "updating", "failed", "live"].contains(state) {
                    #expect(at.figure == base.figure, "\(state): the figure moved \(at.figure - base.figure)")
                }
                // ONE line: what it says and the control share a row.
                #expect(abs(at.line - at.control.midY) < 1,
                        "\(state): the line is at \(at.line) and the control at \(at.control.midY) — two lines")
            }
            // The control is the line's END: on the page's trailing margin,
            // a finger wide, the line's own height.
            #expect(base.control.maxX == size.width - Tokens.Layout.screenPaddingX)
            #expect(base.control.width == Tokens.Control.md && base.control.height == Tokens.Control.sm)
            // The hero is its label, its figure and one line: the first row
            // under it starts a line's height and the gap after the figure's.
            #expect(base.hero < 160, "the hero still has a line too many: \(base.hero)")
        }
    }

    /// The line says ONE thing, most pressing first — and the control's own
    /// words stand there exactly when nothing else does.
    @Test func theLineSaysOneThingAndTheControlsWordsOnlyWhenNothingElse() throws {
        func place(_ state: String, _ loc: Loc) throws -> BalanceDisplay.LinePlace {
            BalanceDisplay.linePlace(try hero(state, loc: loc))
        }
        #expect(try place("checking", en) == .checking("Checking…"))
        #expect(try place("checking", zh) == .checking("正在检查…"))
        #expect(try place("live", en) == .live("Live · listening for payments"))
        guard case .status(let failed) = try place("failed", en) else {
            Issue.record("a failed network is not the line's status")
            return
        }
        #expect(failed.contains("Ethereum"), "\(failed)")
        guard case .status = try place("offline", en) else {
            Issue.record("every network out of reach is not the line's status")
            return
        }
        // Read, with nothing to report: when.
        guard case .stamp(let updated) = try place("updated", en) else {
            Issue.record("a read wallet with nothing to say does not say when it was read")
            return
        }
        #expect(updated.hasPrefix("Updated"), "\(updated)")
        #expect(try place("updating", en) == .stamp("Updating…"))
        #expect(try place("updating", zh) == .stamp(zh.t("home.updating")))
        guard case .stamp = try place("hidden", en) else {
            Issue.record("hidden, the line lost its words")
            return
        }

        // The control is called by those same words whether or not they are
        // drawn: "Updating…" only while it turns, and before any read has
        // settled "Refresh balance" — with nothing on the line to say when.
        let read = try #require(try hero("updated", loc: en).refresh)
        #expect(BalanceRefreshControl.spoken(read) == updated)
        let turning = try #require(try hero("updating", loc: en).refresh)
        #expect(BalanceRefreshControl.spoken(turning) == "Updating…")
        var never = try hero("updated", loc: en)
        never.refresh?.updated = nil
        #expect(BalanceDisplay.linePlace(never) == .empty)
        #expect(BalanceRefreshControl.spoken(try #require(never.refresh)) == "Refresh balance")
        // A status outranks the words, and the control still names when.
        let failing = try hero("failed", loc: en)
        #expect(BalanceRefreshControl.spoken(try #require(failing.refresh)).hasPrefix("Updated"))
        // A drawn board with no control keeps the line's room, empty.
        var bare = try hero("updated", loc: en)
        bare.refresh = nil
        #expect(BalanceDisplay.linePlace(bare) == .empty)
    }
}
