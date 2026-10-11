//
//  SettingsKeysSteadyTests.swift
//  VelaWalletTests
//
//  Settings does not drop when the keys block loads (iPhone walk 2026-10-10:
//  the block was ONE skeleton row until the registry answered, then grew to
//  the wallet's keys and everything under it moved ~250 pt).
//
//  The block now opens on the rows this device's own record gives — the
//  core's device-only answer, no chain asked — each with the room its badges
//  will take. The real `SettingsScreen`, hosted, with the first row under the
//  block read from the tree an assistive client gets: it stands at the same
//  y while the registry is being asked as after it answered.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SettingsKeysSteadyTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    private static let names = ["Parallel One", "Parallel Two", "Parallel Three"]

    /// The wallet's keys as its account record holds them.
    private func record(_ count: Int) -> [WalletKeys.DeviceKey] {
        (0..<count).map { index in
            WalletKeys.DeviceKey(
                credentialId: "cred-\(index)",
                publicKeyHex: "04" + String(repeating: index == 0 ? "ab" : "c\(index)", count: 64),
                name: index == 0 ? Self.names[0] : "", transports: ""
            )
        }
    }

    /// What the device alone says of them — the REAL core's step, asked with
    /// no address so that it asks nobody.
    private func device(_ count: Int) -> WalletKeys.Result {
        WalletKeys(ethCall: { _, _, _ in nil }).deviceOnly(device: record(count), signInCredential: "cred-0")
    }

    /// The registry's answer for the same keys: named, badged.
    private func registry(_ count: Int) -> WalletKeys.Result {
        WalletKeys.Result(source: .registry, rows: (0..<count).map { index in
            WalletKeys.Row(
                key: CreateKeyRow(
                    name: Self.names[index], authenticatorAttachment: "platform", transports: "internal",
                    confirmed: true, synced: true, syncedKnown: true, aaguid: "", providerName: "",
                    method: .platform, kind: .platform
                ),
                synced: true,
                publicKeyHex: "04" + String(repeating: index == 0 ? "ab" : "c\(index)", count: 64),
                credentialId: "cred-\(index)", attestationHex: "", userVerified: true, signsHere: index == 0
            )
        })
    }

    private func screen(_ model: SettingsScreenModel, _ loc: Loc) -> some View {
        SettingsScreen(model: model, loc: loc).themed(.light)
    }

    /// The y of the first row under the keys block: the first section's label.
    private func under(_ seen: HostedScreen.Seen, _ base: SettingsScreenModel) throws -> CGFloat {
        let label = try #require(base.sections.first?.label)
        return try #require(seen.labelled(label), "the first section is in the tree").frame.minY
    }

    /// The device's own answer is the core's: every key, in order, the first
    /// named as the wallet, the sign-in key marked, and no badge nobody has
    /// vouched for.
    @Test func theDeviceAloneNamesEveryKeyAndVouchesForNothing() {
        let own = device(3)
        #expect(own.source == .device)
        #expect(own.rows.count == 3)
        #expect(own.rows.first?.key.name == Self.names[0])
        #expect(own.rows.map(\.signsHere) == [true, false, false])
        #expect(own.rows.allSatisfy { $0.synced == nil && $0.userVerified == nil })

        let base = SettingsFixtures.build(.st1, loc: en)
        let pending = SettingsLive.withWalletKeys(nil, device: own, backup: nil, on: base, loc: en).keys
        #expect(pending?.loading == true, "the registry has not answered")
        #expect(pending?.count == "", "never a guessed count")
        #expect(pending?.rows.map(\.name) == ["Parallel One", "Key 2", "Key 3"])
        #expect(pending?.rows.map { $0.pills.map(\.tone) } == [[.signsHere], [], []])
        #expect(pending?.note == nil, "nobody has failed to answer yet")

        // The registry's answer outranks the device's the moment it is there.
        let answered = SettingsLive.withWalletKeys(registry(3), device: own, backup: nil, on: base, loc: en).keys
        #expect(answered?.loading == false && answered?.count == "3")
        #expect(answered?.rows.map(\.name) == Self.names)

        // A record with no key at all draws no block — while asking, too.
        let none = WalletKeys.Result(source: .device, rows: [])
        #expect(SettingsLive.withWalletKeys(nil, device: none, backup: nil, on: base, loc: en).keys?.loading != true,
                "a wallet with no key to list was given a block to wait in")
        // And with nothing known at all, the one-row skeleton.
        let blank = SettingsLive.withWalletKeys(nil, device: nil, backup: nil, on: base, loc: en).keys
        #expect(blank?.loading == true && blank?.rows.isEmpty == true)
    }

    /// The page under the block stands still when the registry answers: one
    /// key and three, Chinese and English, the 390-wide phone and the SE.
    @Test(arguments: [1, 3])
    func nothingUnderTheBlockMovesWhenTheRegistryAnswers(count: Int) async throws {
        for loc in [zh, en] {
            for size in [HostedScreen.phone, HostedScreen.se] {
                let base = SettingsFixtures.build(.st1, loc: loc)
                let own = device(count)
                let pending = SettingsLive.withWalletKeys(nil, device: own, backup: nil, on: base, loc: loc)
                let answered = SettingsLive.withWalletKeys(registry(count), device: own, backup: nil, on: base, loc: loc)
                let silent = SettingsLive.withWalletKeys(own, device: own, backup: nil, on: base, loc: loc)

                let hosted = HostedScreen(screen(pending, loc), size: size)
                defer { hosted.close() }
                let first = try await hosted.read()
                let asked = try under(first, base)
                #expect(first.says(Self.names[0]), "the key is on screen while the registry is asked")
                try await hosted.save("keys-\(count)-\(loc.resolvedLanguage)-\(Int(size.width))-1-asking")

                let second = try await hosted.show(screen(answered, loc)) { $0.says(Self.names[count - 1]) }
                let landed = try under(second, base)
                try await hosted.save("keys-\(count)-\(loc.resolvedLanguage)-\(Int(size.width))-2-answered")
                print("MEASURE settings-keys \(count) key(s) \(loc.resolvedLanguage) \(Int(size.width))x\(Int(size.height)): first row under the block \(asked) → \(landed)")
                #expect(landed == asked,
                        "\(count) key(s), \(loc.resolvedLanguage), \(Int(size.width)) wide: the page moved \(landed - asked) pt when the keys loaded")

                // A registry that never answers: the rows keep their place;
                // only its one sentence is new, under them.
                let third = try await hosted.show(screen(silent, loc)) {
                    $0.says(loc.t(I18nKeys.SettingsUi.keysFromDevice))
                }
                let note = try #require(third.labelled(loc.t(I18nKeys.SettingsUi.keysFromDevice)))
                let name = { (seen: HostedScreen.Seen) in seen.containing(Self.names[0])?.frame.minY }
                #expect(name(third) == name(first), "the key's row moved when the registry stayed silent")
                #expect(try under(third, base) - asked <= note.frame.height + Tokens.Space.s16 + 0.5,
                        "more than the note's own line moved the page")
            }
        }
    }

    /// What it was: with nothing but a skeleton row to stand for the keys,
    /// the page drops by the keys' own height when they land. Kept as the
    /// measure of what the device's rows are there to prevent.
    @Test func aSkeletonRowAloneIsShorterThanTheKeysItStandsFor() async throws {
        let base = SettingsFixtures.build(.st1, loc: zh)
        let skeleton = SettingsLive.withWalletKeys(nil, device: nil, backup: nil, on: base, loc: zh)
        let answered = SettingsLive.withWalletKeys(registry(3), device: nil, backup: nil, on: base, loc: zh)
        let hosted = HostedScreen(screen(skeleton, zh))
        defer { hosted.close() }
        let before = try under(try await hosted.read(), base)
        let after = try under(try await hosted.show(screen(answered, zh)) { $0.says(Self.names[2]) }, base)
        #expect(after - before > 150, "three keys under one skeleton row: \(after - before) pt")
        print("MEASURE settings-keys skeleton→3 keys: \(before) → \(after)")
    }
}
