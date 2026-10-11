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
//  And while it is being asked, a row says nothing the answer may take back
//  (owner's iPhone, 2026-10-11: the device's record does not know a key's
//  kind for certain — its method falls back to "platform" — so a security
//  key read 「内置通行密钥」 with a person glyph for three seconds, then
//  「安全密钥」 with a key). Where the key lives, its mark and a name the
//  record does not hold are quiet shapes of their own size until the answer
//  fills them in place; the name the record holds and the short id stay.
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

    /// The registry's answer where it CONTRADICTS what the device's record
    /// would have guessed (every key "platform"): the first key is a security
    /// key, the second a passkey in a named vault, the third lives on a phone.
    private func contradicting(_ count: Int) -> WalletKeys.Result {
        let methods: [KeyMethod] = [.securityKey, .platform, .hybrid]
        let attachments = ["cross-platform", "platform", "cross-platform"]
        let transports = ["usb", "internal", "hybrid"]
        let providers = ["", "Apple Passwords", ""]
        var rows: [WalletKeys.Row] = []
        for index in 0..<count {
            let synced = index != 0
            let key = CreateKeyRow(
                name: Self.names[index], authenticatorAttachment: attachments[index],
                transports: transports[index], confirmed: true, synced: synced, syncedKnown: true,
                aaguid: "", providerName: providers[index], method: methods[index], kind: methods[index]
            )
            let pair: String = index == 0 ? "ab" : "c\(index)"
            let publicKey: String = "04" + String(repeating: pair, count: 64)
            rows.append(WalletKeys.Row(
                key: key, synced: synced, publicKeyHex: publicKey,
                credentialId: "cred-\(index)", attestationHex: "", userVerified: true, signsHere: index == 0
            ))
        }
        return WalletKeys.Result(source: .registry, rows: rows)
    }

    /// The short id each row of `record` wears.
    private static let ids = ["abab…abab", "c1c1…c1c1", "c2c2…c2c2"]

    private static func lightness(_ p: (r: Int, g: Int, b: Int)) -> Int { (p.r * 299 + p.g * 587 + p.b * 114) / 1000 }

    /// How many pixels of `rect` are INK beside `fill` — plainly darker than
    /// the placeholder's own colour, as a glyph or a word is and a quiet
    /// shape is not.
    private func ink(_ picture: HostedScreen.Picture, in rect: CGRect, beside fill: (r: Int, g: Int, b: Int)) -> Int {
        let floor = Self.lightness(fill) - 24
        var count = 0
        for y in max(0, Int(rect.minY.rounded(.up)))..<min(picture.height, Int(rect.maxY)) {
            for x in max(0, Int(rect.minX.rounded(.up)))..<min(picture.width, Int(rect.maxX)) where Self.lightness(picture.pixel(x, y)) < floor {
                count += 1
            }
        }
        return count
    }

    /// The mark's box in a key row: at the row's leading edge, on its middle.
    private func markBox(_ row: HostedScreen.Element) -> CGRect {
        let side = LucideIconSize.action
        return CGRect(x: row.frame.minX, y: row.frame.midY - side / 2, width: side, height: side)
    }

    /// Where a row's words begin: the first points of its text column, the
    /// row's whole height.
    private func wordsStart(_ row: HostedScreen.Element) -> CGRect {
        CGRect(
            x: row.frame.minX + LucideIconSize.action + Tokens.Space.s12, y: row.frame.minY + 1,
            width: 20, height: row.frame.height - 2
        )
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
        // The name the record holds stays; one it does not hold is not made
        // up while the registry may still give it.
        #expect(pending?.rows.map(\.name) == ["Parallel One", "", ""])
        #expect(pending?.rows.map(\.nameRoom) == ["Key 1", "Key 2", "Key 3"])
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

    /// What a row still being asked about SAYS, as a model: the device's
    /// record reads every key as "platform" (it knows no better), and none of
    /// that reaches the row — no kind, nothing to draw a mark from, nothing
    /// to open. The answer says the first key is a security key; a registry
    /// that never answers leaves the device's own reading, under its note.
    @Test func aRowBeingAskedAboutCarriesNothingTheRegistryMayContradict() {
        for loc in [zh, en] {
            let k = I18nKeys.SettingsUi.self
            let platform = loc.t(k.keysProviderPlatform), security = loc.t(k.keysProviderSecurityKey)
            let base = SettingsFixtures.build(.st1, loc: loc)
            let own = device(3)
            #expect(own.rows.allSatisfy { $0.key.method == .platform }, "the record's method falls back to platform")

            let pending = SettingsLive.withWalletKeys(nil, device: own, backup: nil, on: base, loc: loc).keys
            let rows = pending?.rows ?? []
            #expect(rows.count == 3 && rows.allSatisfy(\.pending))
            #expect(rows.allSatisfy { $0.holder.isEmpty }, "a kind was said before the registry answered: \(rows.map(\.holder))")
            #expect(rows.allSatisfy { $0.key == nil }, "there is a key record to draw a guessed mark from")
            #expect(rows.allSatisfy { $0.details.isEmpty }, "a row that grows under the finger when the answer lands")
            #expect(rows.map(\.fingerprint) == Self.ids, "the short id is the device's fact and stays")
            // What VoiceOver gets: the name the record holds and the id; no kind.
            let spoken = rows.map(WalletKeysBlock.spoken)
            #expect(spoken == [
                ["Parallel One", Self.ids[0], loc.t(k.keysSignsHere)].joined(separator: ", "), Self.ids[1], Self.ids[2],
            ])
            for kind in pending?.holderRoom ?? [] {
                #expect(!spoken.contains { $0.contains(kind) }, "\(kind) is read out before anybody said it")
            }
            // The words' room is the same whatever the device believes: all
            // three kinds, so the bar is no hint of a guess either.
            #expect(Set(pending?.holderRoom ?? []) == Set([platform, security, loc.t(k.keysProviderGeneric)]))

            // A device that believed otherwise draws the very same rows.
            let other = WalletKeys.Result(source: .device, rows: own.rows.map { row in
                WalletKeys.Row(
                    key: CreateKeyRow(
                        name: row.key.name, authenticatorAttachment: "cross-platform", transports: "usb",
                        confirmed: true, synced: true, syncedKnown: false, aaguid: "", providerName: "",
                        method: .securityKey, kind: .securityKey
                    ),
                    synced: nil, publicKeyHex: row.publicKeyHex, credentialId: row.credentialId,
                    signsHere: row.signsHere
                )
            })
            let believed = SettingsLive.withWalletKeys(nil, device: other, backup: nil, on: base, loc: loc).keys?.rows ?? []
            #expect(believed.map(WalletKeysBlock.spoken) == spoken)
            #expect(believed.map(\.holder) == rows.map(\.holder) && believed.allSatisfy { $0.key == nil })

            // The answer: said once, by the one who knows.
            let answered = SettingsLive.withWalletKeys(contradicting(3), device: own, backup: nil, on: base, loc: loc).keys?.rows ?? []
            #expect(answered.allSatisfy { !$0.pending && $0.key != nil })
            #expect(answered.map(\.holder) == [security, "Apple Passwords", loc.t(k.keysProviderGeneric)])
            #expect(answered.map(\.name) == Self.names)
            #expect(answered.first.map(WalletKeysBlock.spoken)?.hasPrefix("Parallel One, \(security)") == true)

            // No answer at all — offline — is final: then, and only then, the
            // device's own reading, as it always was.
            let silent = SettingsLive.withWalletKeys(own, device: own, backup: nil, on: base, loc: loc).keys
            #expect(silent?.note == loc.t(k.keysFromDevice))
            #expect(silent?.rows.allSatisfy { !$0.pending && $0.key != nil } == true)
            #expect(silent?.rows.map(\.holder) == [platform, platform, platform])
            #expect(silent?.rows.map(\.name).dropFirst().allSatisfy { !$0.isEmpty } == true)
        }
    }

    /// On screen. The registry is asked: no kind is written, no glyph stands
    /// in the mark's place, an unheld name is not made up — and the key the
    /// record names is there, with its id. The answer says "security key":
    /// every row's box and everything under the block are where they were.
    /// One key and three, Chinese and English, the 390-wide phone and the SE.
    @Test(arguments: [1, 3])
    func theAnswerFillsTheRowsInPlaceAndTakesNothingBack(count: Int) async throws {
        let k = I18nKeys.SettingsUi.self
        for loc in [zh, en] {
            let kinds = [k.keysProviderPlatform, k.keysProviderSecurityKey, k.keysProviderGeneric].map { loc.t($0) }
            for size in [HostedScreen.phone, HostedScreen.se] {
                let at = "\(count) key(s), \(loc.resolvedLanguage), \(Int(size.width)) wide"
                let base = SettingsFixtures.build(.st1, loc: loc)
                let own = device(count)
                let pending = SettingsLive.withWalletKeys(nil, device: own, backup: nil, on: base, loc: loc)
                let answered = SettingsLive.withWalletKeys(contradicting(count), device: own, backup: nil, on: base, loc: loc)
                let silent = SettingsLive.withWalletKeys(own, device: own, backup: nil, on: base, loc: loc)

                // 1 — the registry is being asked.
                let hosted = HostedScreen(screen(pending, loc), size: size)
                defer { hosted.close() }
                let first = try await hosted.read { $0.elements("keys.row").count == count }
                let asking = try await hosted.picture(settled: true)
                let rows = first.elements("keys.row")
                try #require(rows.count == count, "\(at): the keys are on screen while the registry is asked")
                let asked = try under(first, base)
                try await hosted.save("keys-\(count)-\(loc.resolvedLanguage)-\(Int(size.width))-1-asking")

                #expect(rows[0].label.hasPrefix("Parallel One, \(Self.ids[0])"), "\(at): \(rows[0].label)")
                for kind in kinds {
                    #expect(!rows.contains { $0.label.contains(kind) }, "\(at): 「\(kind)」 is said before the registry answered")
                }
                // The mark's place holds a quiet shape and nothing drawn in ink.
                let fill = asking.pixel(Int(markBox(rows[0]).midX), Int(markBox(rows[0]).midY))
                for (index, row) in rows.enumerated() {
                    #expect(ink(asking, in: markBox(row), beside: fill) == 0, "\(at): a glyph stands for key \(index + 1) before anybody said whose it is")
                    // The id is written: the row is not a blank.
                    #expect(ink(asking, in: row.frame, beside: fill) > 0, "\(at): key \(index + 1) shows nothing at all")
                }
                // A key the record does not name: no made-up name, no kind —
                // where its words begin there are only the bars.
                for (index, row) in rows.enumerated().dropFirst() {
                    #expect(row.label == Self.ids[index], "\(at): \(row.label)")
                    #expect(ink(asking, in: wordsStart(row), beside: fill) == 0, "\(at): words are written on key \(index + 1) before the answer")
                }

                // 2 — the answer: the first key is a security key.
                let second = try await hosted.show(screen(answered, loc)) {
                    $0.elements("keys.row").first?.label.contains(kinds[1]) == true && $0.says(Self.names[count - 1])
                }
                let landed = try under(second, base)
                let answer = try await hosted.picture(settled: true)
                try await hosted.save("keys-\(count)-\(loc.resolvedLanguage)-\(Int(size.width))-2-answered")
                print("MEASURE settings-keys \(at): first row under the block \(asked) → \(landed); rows \(rows.map { "\($0.frame.minY)+\($0.frame.height)" }) → \(second.elements("keys.row").map { "\($0.frame.minY)+\($0.frame.height)" })")
                #expect(second.elements("keys.row").map(\.frame) == rows.map(\.frame), "\(at): a key's row changed its box when the answer landed")
                #expect(landed == asked, "\(at): the page moved \(landed - asked) pt when the keys loaded")
                // Everything under the rows, element by element: same words, same place.
                let floor = rows[count - 1].frame.maxY
                #expect(second.tree.filter { $0.frame.minY >= floor } == first.tree.filter { $0.frame.minY >= floor },
                        "\(at): something under the block is not where it was")
                #expect(second.elements("keys.row").first?.label.hasPrefix("Parallel One, \(kinds[1])") == true,
                        "\(at): \(second.elements("keys.row").first?.label ?? "no row")")
                #expect(!second.elements("keys.row").contains { $0.label.contains(kinds[0]) }, "\(at): the device's guess is on screen after the answer")
                #expect(ink(answer, in: markBox(rows[0]), beside: fill) > 0, "\(at): the mark was not filled in")

                // 3 — a registry that never answers: the rows keep their
                // place, and only now the device's own reading is shown, with
                // its one sentence under them.
                let third = try await hosted.show(screen(silent, loc)) {
                    $0.says(loc.t(I18nKeys.SettingsUi.keysFromDevice))
                }
                let offline = try await hosted.picture(settled: true)
                let note = try #require(third.labelled(loc.t(I18nKeys.SettingsUi.keysFromDevice)))
                #expect(third.elements("keys.row").map(\.frame) == rows.map(\.frame), "\(at): a key's row moved when the registry stayed silent")
                #expect(third.elements("keys.row").allSatisfy { $0.label.contains(kinds[0]) }, "\(at): the device's own reading is not shown when it is all there is")
                #expect(ink(offline, in: markBox(rows[0]), beside: fill) > 0, "\(at): no mark for the device's own reading")
                #expect(try under(third, base) - asked <= note.frame.height + Tokens.Space.s16 + 0.5,
                        "\(at): more than the note's own line moved the page")
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
