//
//  WalletFixturesTests.swift
//  VelaWalletTests
//
//  Spec 015 fixture canon (data-model.md, FR-012): the builders must
//  reproduce the mock content verbatim so visual diffing against
//  docs/design/wallet/ stays meaningful.
//

import CoreGraphics
import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct WalletFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    @Test func allTenMobileStatesExist() {
        #expect(MobileStateId.allCases.count == 10)
        for state in MobileStateId.allCases {
            let model = WalletFixtures.buildMobileState(state, loc: loc)
            #expect(model.state == state)
        }
    }

    @Test func identityIsVerbatim() {
        #expect(WalletFixtures.identity.name == "大表哥")
        #expect(WalletFixtures.identity.longName == "这是一个非常长")
        #expect(WalletFixtures.identity.addressDisplay == "0x14fB1f…D1eA5c")
        #expect(WalletFixtures.identity.addressFull == "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c")
    }

    @Test func h1DefaultState() {
        let model = WalletFixtures.buildMobileState(.h1, loc: loc)
        #expect(model.balance.integer == "$1,383")
        #expect(model.balance.decimals == "28")
        #expect(model.balance.state == .normal)
        // H1 first screen: 今天 rows 1–2 only (data-model.md).
        #expect(model.activityGroups.count == 1)
        #expect(model.activityGroups[0].rows.count == 2)
        #expect(model.activityGroups[0].rows[0].amount == "−2")
        #expect(model.activityGroups[0].rows[0].unit == "POL")
        #expect(model.activityGroups[0].rows[1].amount == "+120")
        #expect(model.activityGroups[0].rows[1].positive)
        #expect(model.assetRows.count == 6)
        #expect(model.assetRows[0].ticker == "BNB")
        #expect(model.assetRows[0].balance == "0.8533")
    }

    @Test func h1sShowsAllDayGroups() {
        let model = WalletFixtures.buildMobileState(.h1s, loc: loc)
        #expect(model.activityGroups.count == 2)
        #expect(model.activityGroups[0].label == "今天")
        #expect(model.activityGroups[0].rows.count == 3)
        #expect(model.activityGroups[1].label == "昨天")
        #expect(model.activityGroups[1].rows.count == 1)
        #expect(model.activityGroups[1].rows[0].amount == "+50")
        #expect(model.activityGroups[1].rows[0].unit == "USDC")
    }

    @Test func h2EmptyZeroLive() {
        let model = WalletFixtures.buildMobileState(.h2, loc: loc)
        #expect(model.balance.state == .zeroLive)
        #expect(model.balance.integer == "$0")
        #expect(model.balance.decimals == "00")
        #expect(model.balance.liveText == "实时 · 监听收款中")
        #expect(model.activitySection.mode == .empty)
        #expect(model.assetsSection.mode == .empty)
        #expect(model.activityGroups.isEmpty)
        #expect(model.assetRows.isEmpty)
    }

    @Test func h3Loading() {
        let model = WalletFixtures.buildMobileState(.h3, loc: loc)
        #expect(model.balance.state == .loading)
        #expect(model.balance.integer == nil)
        #expect(model.activitySection.mode == .loading)
        #expect(model.assetsSection.mode == .loading)
    }

    @Test func h4PartialPrice() {
        let model = WalletFixtures.buildMobileState(.h4, loc: loc)
        #expect(model.balance.decimals == "46")
        #expect(model.balance.status?.kind == .warning)
        #expect(model.balance.status?.text == "部分代币无法获取价格。")
        #expect(model.assetRows.count == 3)
        #expect(model.assetRows[2].ticker == "CAKE")
        if case .noPrice(let text) = model.assetRows[2].fiat {
            #expect(text == "无价格")
        } else {
            Issue.record("CAKE row must be no-price")
        }
    }

    @Test func h5MaskedKeepsUnitsAndColors() {
        let model = WalletFixtures.buildMobileState(.h5, loc: loc)
        #expect(model.balance.state == .hidden)
        #expect(model.balance.integer == "••••••")
        #expect(model.balance.decimals == nil)
        for group in model.activityGroups {
            for row in group.rows {
                #expect(row.masked)
                #expect(row.amount == "••••")
                #expect(!row.unit.isEmpty)
            }
        }
        // Received rows keep the success color while masked.
        #expect(model.activityGroups[0].rows[1].positive)
        for row in model.assetRows {
            #expect(row.masked)
            #expect(row.balance == "••••")
            if case .masked = row.fiat {} else { Issue.record("masked asset must mask fiat") }
        }
    }

    @Test func h6Refreshing() {
        let model = WalletFixtures.buildMobileState(.h6, loc: loc)
        #expect(model.balance.status?.kind == .refreshing)
        #expect(model.balance.status?.text == "部分余额仍在更新。")
    }

    @Test func h7Extremes() {
        let model = WalletFixtures.buildMobileState(.h7, loc: loc)
        #expect(model.header.name == "这是一个非常长")
        #expect(model.balance.integer == "$1,234,567")
        #expect(model.balance.decimals == "89")
        if case .single(_, let label) = model.pill {
            #expect(label == "BNB Chain")
        } else {
            Issue.record("H7 uses the single-chain pill")
        }
        #expect(model.activityGroups[0].rows[0].amount == "−1234.5678")
        #expect(model.activityGroups[0].rows[1].amount == "−0.0000001")
        #expect(model.assetRows[1].balance == "1,234,567.8901")
        #expect(model.textScale == 1)
    }

    @Test func h7xScales() {
        let model = WalletFixtures.buildMobileState(.h7x, loc: loc)
        #expect(model.textScale == 1.35)
        #expect(model.balance.integer == "$1,234,567")
    }

    @Test func h8Sheet() throws {
        let model = WalletFixtures.buildMobileState(.h8, loc: loc)
        let sheet = try #require(model.sheet)
        #expect(sheet.title == "选择链")
        #expect(sheet.rows.count == 7)
        #expect(sheet.rows[0].name == "所有网络")
        #expect(sheet.rows[0].count == 8)
        #expect(sheet.rows[0].selected)
        #expect(sheet.rows[1].name == "BNB Chain")
        #expect(sheet.rows[2].name == "Ethereum")
        #expect(sheet.rows[2].count == 3)
        #expect(!sheet.rows.dropFirst().contains { $0.selected })
    }

    @Test func identiconBoardSeeds() {
        #expect(WalletFixtures.identiconBoardSeeds.count == 6)
        #expect(WalletFixtures.identiconBoardSeeds.first == WalletFixtures.identity.addressFull)
        #expect(WalletFixtures.identiconBoardSeeds.last == "")
    }

    @Test func englishLocaleResolvesEveryString() {
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        let model = WalletFixtures.buildMobileState(.h1, loc: en)
        // No key leaks: resolved strings never look like corpus keys (FR-010).
        for s in [model.balance.label, model.actions.receive, model.actions.send,
                  model.actions.scan, model.activitySection.title, model.assetsSection.title,
                  model.tabs.wallet, model.tabs.contacts, model.tabs.explore, model.tabs.settings] {
            #expect(!s.contains("."))
            #expect(!s.isEmpty)
        }
    }
}
