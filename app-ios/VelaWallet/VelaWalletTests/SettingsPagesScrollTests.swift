//
//  SettingsPagesScrollTests.swift
//  VelaWalletTests
//
//  The rules behind Settings' pushed pages (device pass 2026-10-09, the same
//  finding as Android's `SettingsPagesTest`): the home and every pushed page
//  shared ONE scroll view, so 设置 → 高级 → 网络 opened at the offset the home
//  had been scrolled to. A pushed page now opens at its top, the ‹ goes back
//  to the Settings home, and only the home keeps its place.
//
//  The screen itself — real taps, a scrolled home, the frames on screen — is
//  `SettingsPagesScrollUITests`: a row here is a tap gesture, which a unit
//  test cannot press (`accessibilityActivate` leaves it to VoiceOver's own
//  synthesised tap).
//

import Testing
@testable import VelaWallet

struct SettingsPagesScrollTests {

    private let pushed: [SettingsPage] = [.networks, .networkDetail, .addNetwork, .rpcProviders, .endpoints, .storage, .about]

    @Test func backFromEveryPushedPageIsTheSettingsHome() {
        for page in pushed {
            #expect(page.back == .home, "Back from \(page)")
        }
        #expect(SettingsPage.home.back == nil, "the home has no way back inside Settings")
    }

    @Test func onlyTheHomeKeepsItsPlaceEveryPushedPageOpensAtItsTop() {
        #expect(SettingsPage.home.keepsItsPlace)
        for page in pushed {
            #expect(!page.keepsItsPlace, "\(page) opens at its top")
        }
    }
}
