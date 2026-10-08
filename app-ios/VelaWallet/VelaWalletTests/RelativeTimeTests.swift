//
//  RelativeTimeTests.swift
//  VelaWalletTests
//
//  "Updated 2m" is the core's sentence (issue 462): `I18n::format_relative_time`,
//  exported through uniffi, so this shell ports none of it. Before the export
//  iOS carried a copy of its first three branches and drew a date where the
//  core names the weekday. These replay the core's own vectors —
//  `rust/crates/vela-core/tests/vectors/relative-time.json`, the file the core,
//  the wasm and the Kotlin and Swift harnesses read — through THIS shell's
//  route (`Loc`), so a shell that grew its own copy again, or handed the core
//  milliseconds where it takes seconds, fails here.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct RelativeTimeTests {

    private struct Case {
        let name: String
        let lng: String
        let tsSeconds: Int64
        let nowMs: Int64
        let utcOffsetMinutes: Int32
        let dateFormat: String
        let value: String
    }

    private struct Missing: Error {}

    /// The vector file, read from the repository this app is built from.
    private static func cases() throws -> [Case] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
            .appendingPathComponent("rust/crates/vela-core/tests/vectors/relative-time.json")
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        guard json?["suite"] as? String == "relative-time",
              let raw = json?["cases"] as? [[String: Any]]
        else { throw Missing() }
        return try raw.map { entry in
            guard entry["fn"] as? String == "format_relative_time",
                  let name = entry["name"] as? String,
                  let input = entry["input"] as? [String: Any],
                  let lng = input["lng"] as? String,
                  let ts = (input["ts_seconds"] as? NSNumber)?.int64Value,
                  let now = (input["now_ms"] as? NSNumber)?.int64Value,
                  let offset = (input["utc_offset_minutes"] as? NSNumber)?.int32Value,
                  let date = input["date_format"] as? String,
                  let value = (entry["expect"] as? [String: Any])?["value"] as? String
            else { throw Missing() }
            return Case(name: name, lng: lng, tsSeconds: ts, nowMs: now,
                        utcOffsetMinutes: offset, dateFormat: date, value: value)
        }
    }

    /// Every case, through `Loc` in the case's language — the very object the
    /// hero's control asks.
    @Test func everyVectorAnswersTheSameThroughThisShellsLoc() throws {
        let cases = try Self.cases()
        #expect(cases.count >= 110, "the suite shipped with 110 hand-written cases")
        var engines: [String: Loc] = [:]
        for entry in cases {
            let loc = engines[entry.lng] ?? Loc(overrideTag: entry.lng, preferredLanguages: [])
            engines[entry.lng] = loc
            #expect(loc.resolvedLanguage == entry.lng, "\(entry.name): no catalog for \(entry.lng)")
            let got = loc.relativeTime(
                tsSeconds: entry.tsSeconds, nowMs: entry.nowMs,
                utcOffsetMinutes: entry.utcOffsetMinutes, dateFormat: entry.dateFormat
            )
            #expect(got == entry.value, "\(entry.name)")
        }
        #expect(engines.count == Loc.supported.count, "every language is replayed")
    }

    /// The everyday route: a read in epoch milliseconds becomes whole seconds,
    /// the zone is the one given at that moment, and the person's own date
    /// preset names an old read.
    @Test func aReadInMillisecondsIsTheCoresSentence() {
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let utc = TimeZone(secondsFromGMT: 0)!
        // Fri 2027-01-15 08:00:00.400 UTC: the fraction is floored away, as the
        // core floors the clock.
        let at = 1_800_000_000_400.0
        func later(_ seconds: Double) -> Date { Date(timeIntervalSince1970: 1_800_000_000 + seconds) }
        #expect(en.relativeTime(atMs: at, now: later(44.9), timeZone: utc) == "now")
        #expect(en.relativeTime(atMs: at, now: later(45), timeZone: utc) == "1m")
        #expect(en.relativeTime(atMs: at, now: later(2 * 86_400), timeZone: utc) == "Fri",
                "within the week the core names the day — the port drew a date")
        #expect(zh.relativeTime(atMs: at, now: later(120), timeZone: utc) == "2分钟前")
        #expect(zh.relativeTime(atMs: at, now: later(2 * 86_400), timeZone: utc) == "周五")
        // Late Friday in UTC is already Saturday in Tokyo: the zone is the
        // device's, at the moment of the read.
        let late = 1_800_050_400_000.0   // Fri 2027-01-15 22:00 UTC
        let tokyo = TimeZone(identifier: "Asia/Tokyo")!
        let sunday = Date(timeIntervalSince1970: 1_800_050_400 + 2 * 86_400)
        #expect(en.relativeTime(atMs: late, now: sunday, timeZone: utc) == "Fri")
        #expect(en.relativeTime(atMs: late, now: sunday, timeZone: tokyo) == "Sat")
        // A month on, the date — in the preset handed over.
        let month = 1_800_000_000 + 30 * 86_400
        #expect(en.relativeTime(tsSeconds: 1_800_000_000, nowMs: Int64(month) * 1000,
                                utcOffsetMinutes: 0, dateFormat: "dmy_dot") == "15.01.2027")
        #expect(en.relativeTime(tsSeconds: 1_800_000_000, nowMs: Int64(month) * 1000,
                                utcOffsetMinutes: 0, dateFormat: "iso") == "2027-01-15")
    }
}
