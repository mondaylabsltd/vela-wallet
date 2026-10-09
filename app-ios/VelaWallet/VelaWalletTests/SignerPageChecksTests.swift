//
//  SignerPageChecksTests.swift
//  VelaWalletTests
//
//  Spec 102 R6 / P2-06 on the phone: before a signing page opens, this device
//  fetches the very version it will open, hashes it, and the core rules — and
//  only that ruling builds a launch URL. A failed, missing or mismatched check
//  opens nothing, and the line says why.
//
//  Hermetic: the two fetches are injected, the bytes are made up, and every
//  verdict is the core's (`SignerPageTarget.choose`, `signerPageHash`,
//  `signerPageAdmit`).
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SignerPageChecksTests {

    private let custom = "https://sign.example.com/"
    private let official = "https://sign.getvela.app/"
    private let bytes = Data("<!doctype html><title>a page somebody built</title>".utf8)
    private var hash: String { signerPageHash(bytes: bytes) }

    /// Records what was fetched, and answers from a script. A page fetch can
    /// be held open (`hold`) to look at the checker while a check runs.
    @MainActor
    final class Server {
        var index: Data?
        var page: Data?
        private(set) var indexAsked: [URL] = []
        private(set) var pageAsked: [URL] = []
        /// Set: the next page fetch waits until `release()`.
        var hold = false
        private var held: CheckedContinuation<Void, Never>?

        func release() {
            hold = false
            held?.resume()
            held = nil
        }

        func checks(
            trusted: [String] = [], blocked: [String] = [], clock: @escaping () -> UInt64
        ) -> SignerPageChecks {
            SignerPageChecks(
                fetchIndex: { [self] url in indexAsked.append(url); return index },
                fetchPage: { [self] url in
                    pageAsked.append(url)
                    if hold { await withCheckedContinuation { held = $0 } }
                    return page
                },
                trusted: { _ in trusted },
                blocked: { blocked },
                clock: clock
            )
        }
    }

    private static let start: UInt64 = 1_800_000_000_000
    private static let hour: UInt64 = 60 * 60 * 1000

    /// A person's own build, listed by its index and trusted on this device:
    /// the bytes ARE the version the URL names, so the page opens — at the
    /// address that was fetched, and nowhere else — in the app's language.
    @Test func aPageWhoseBytesAreTheVersionItNamesOpensAtTheCheckedUrl() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        let checks = server.checks(trusted: [hash], clock: { Self.start })

        let admission = await checks.ensure(custom)
        #expect(admission.opens())
        let line = checks.line(for: custom)
        #expect(line.state == .trustedHere && line.opens)
        #expect(line.version == String(hash.prefix(8)))
        #expect(server.indexAsked == [URL(string: custom + "index.json")!])
        let fetched = try #require(server.pageAsked.first)
        #expect(fetched.absoluteString == custom + "b/\(hash)/sign.html")

        // R6: the launch is built from the SAME admission, so it opens the
        // URL that was fetched — and tells the page the app's language
        // (core round 6), so it speaks it too.
        let launch = try admission.urlLaunch(
            requestJson: #"{"intent":{},"context":{}}"#, token: "t", lang: "zh-HK", nowMs: Self.start
        )
        #expect(launch.hasPrefix(fetched.absoluteString))
        #expect(launch.contains("lang=zh-HK"))
        #expect(checks.openable(custom) != nil)
        // The channel's launcher carries the language it is given.
        let viaChannel = TrustedSignerChannel.launcher(admission, lang: "ja")(#"{"intent":{},"context":{}}"#, "t")
        #expect(viaChannel?.contains("lang=ja") == true)
    }

    /// Bytes that are not the version the URL names are refused: nothing
    /// opens, no launch URL is built, and the line says the version is not on
    /// the published list — never "verified".
    @Test func bytesThatAreNotTheVersionNamedOpenNothing() async throws {
        let server = Server()
        server.index = nil
        server.page = bytes
        let checks = server.checks(clock: { Self.start })

        let admission = await checks.ensure(official)
        #expect(!admission.opens())
        let line = checks.line(for: official)
        #expect(line.state == .mismatch && !line.opens)
        #expect(line.key == "componentsUi.signing.integrity.mismatch")
        #expect(checks.openable(official) == nil)
        #expect(throws: (any Error).self) {
            try admission.urlLaunch(
                requestJson: #"{"intent":{},"context":{}}"#, token: "t", lang: "en", nowMs: Self.start
            )
        }
        // An index that could not be read did not stop the check: the
        // official page's launch version was asked for directly.
        #expect(server.pageAsked.first?.absoluteString.hasPrefix(official + "b/") == true)
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let text = line.text(loc)
        #expect(text.contains("isn't on Vela's published build list"))
        #expect(!text.localizedCaseInsensitiveContains("verified"))
        #expect(!text.localizedCaseInsensitiveContains("untampered"))
    }

    /// A page this device could not fetch is not opened on trust: "couldn't
    /// check", and the card offers to try again.
    @Test func aPageThatCannotBeFetchedOpensNothingAndCanBeCheckedAgain() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = nil
        let checks = server.checks(trusted: [hash], clock: { Self.start })

        await checks.ensure(custom)
        let line = checks.line(for: custom)
        #expect(line.state == .couldNotCheck && !line.opens)
        #expect(TrustedSignerSheetModel.offersRecheck(line))

        server.page = bytes
        await checks.check(custom)
        #expect(checks.line(for: custom).opens, "the second check did not stand")
        #expect(server.pageAsked.count == 2)
    }

    /// A check vouches for 24 hours (the core's). After that the line reads
    /// "checking", nothing opens on the old ruling, and `ensure` fetches again.
    @Test func aStaleCheckOpensNothingUntilItRunsAgain() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        var now = Self.start
        let checks = server.checks(trusted: [hash], clock: { now })

        await checks.ensure(custom)
        #expect(checks.openable(custom) != nil)
        await checks.ensure(custom)
        #expect(server.pageAsked.count == 1, "a fresh ruling was fetched again")

        now += 25 * Self.hour
        #expect(checks.line(for: custom).state == .checking)
        #expect(checks.openable(custom) == nil, "a day-old check still opened the page")
        await checks.ensure(custom)
        #expect(server.pageAsked.count == 2)
        #expect(checks.openable(custom) != nil)
    }

    /// D-14: a page in use is checked again in the background once its check
    /// is half a day old — not before; an offline refresh keeps the ruling
    /// that still vouches (Open does not wait), and rests before it tries
    /// again; a refresh that completes replaces it.
    @Test func theBackgroundRefreshKeepsAGoodCheckThroughAnOfflineAttempt() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        var now = Self.start
        let checks = server.checks(trusted: [hash], clock: { now })

        // Nothing stands yet: due at once.
        #expect(checks.refreshDue(custom))
        await checks.ensure(custom)
        #expect(!checks.refreshDue(custom), "a fresh check was due again")
        checks.refresh([custom, custom + "/"])
        await Task.yield()
        #expect(server.pageAsked.count == 1, "a fresh page was fetched in the background")

        // Thirteen hours on: due, and an attempt that cannot complete keeps
        // the standing check — it still opens, its line unchanged.
        now += 13 * Self.hour
        #expect(checks.refreshDue(custom))
        server.page = nil
        let kept = await checks.check(custom)
        #expect(kept.isFresh(nowMs: now))
        #expect(checks.openable(custom) != nil, "an offline refresh threw away a good check")
        #expect(checks.line(for: custom).state == .trustedHere)
        // …and the core rests it before the next try.
        #expect(!checks.refreshDue(custom), "an offline page was asked again at once")
        now += 11 * 60 * 1000
        #expect(checks.refreshDue(custom))

        // A refresh that completes wins — a mismatch included.
        server.page = Data("other bytes".utf8)
        await checks.check(custom)
        #expect(checks.line(for: custom).state == .mismatch)
        #expect(checks.openable(custom) == nil)
    }

    /// While a check runs, a line that still vouches stays as it is (no
    /// flicker); with nothing standing, it reads "checking".
    @Test func aRunningCheckKeepsAGoodLineAndOtherwiseSaysChecking() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        var now = Self.start
        let checks = server.checks(trusted: [hash], clock: { now })

        server.hold = true
        let first = Task { await checks.check(custom) }
        while !checks.isChecking(custom) { await Task.yield() }
        #expect(checks.line(for: custom).state == .checking)
        server.release()
        _ = await first.value
        #expect(checks.line(for: custom).state == .trustedHere)

        now += 13 * Self.hour
        server.hold = true
        let refresh = Task { await checks.check(custom) }
        while !checks.isChecking(custom) { await Task.yield() }
        #expect(checks.line(for: custom).state == .trustedHere, "a background refresh made a good line flicker")
        server.release()
        _ = await refresh.value
    }

    /// A version this device blocked is never fetched as the page to open.
    @Test func aBlockedVersionIsNotOpened() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        let checks = server.checks(trusted: [hash], blocked: [hash], clock: { Self.start })
        await checks.ensure(custom)
        #expect(!checks.line(for: custom).opens)
        #expect(checks.openable(custom) == nil)
    }

    /// D-15: a self-hosted page's index proposes its own build, which nobody
    /// here knows — the line asks, nothing opens, and "Trust this version"
    /// stores exactly that version on THAT page (`version_trusted`); the page
    /// is checked again and opens. Another page serving the same bytes still
    /// asks. The official page never asks.
    @Test func aSelfHostedBuildIsTrustedOnThatPageOnly() async throws {
        let server = Server()
        server.index = Data(#"{"versions":["\#(hash)"]}"#.utf8)
        server.page = bytes
        final class Box { var byPage: [String: [String]] = [:]; var recorded: [(String, String)] = [] }
        let box = Box()
        let checks = SignerPageChecks(
            fetchIndex: { [server] _ in server.index },
            fetchPage: { [server] _ in server.page },
            trusted: { box.byPage[SignerPageChecks.key($0)] ?? [] },
            blocked: { [] },
            recordTrust: { url, version in
                box.recorded.append((url, version))
                box.byPage[SignerPageChecks.key(url), default: []].append(version)
            },
            clock: { Self.start }
        )
        let other = "https://sign.other.example/"
        await checks.ensure(custom)
        await checks.ensure(other)
        #expect(checks.line(for: custom).state == .askToTrust)
        #expect(!checks.line(for: custom).opens)
        let asked = try #require(checks.versionAskingTrust(custom))
        #expect(asked == hash, "the version to trust is the full sha256")

        await checks.trustAsked(custom)
        #expect(box.recorded.count == 1 && box.recorded.first?.0 == custom && box.recorded.first?.1 == hash)
        #expect(checks.line(for: custom).state == .trustedHere)
        #expect(checks.openable(custom) != nil)
        #expect(checks.versionAskingTrust(custom) == nil)
        // Trust vouches for that deployment, not for the bytes anywhere.
        await checks.check(other)
        #expect(checks.line(for: other).state == .askToTrust)
    }

    /// The trusted versions are read from the saved page itself
    /// (`vela.signingPages`, through the core); the device-wide list is not.
    @Test func trustIsReadFromTheSavedPage() {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let h = String(repeating: "cd", count: 32)
        store.writeString(VelaStore.Key.signingPages, #"[{"url":"https://sign.example.com/","name":"","trusted":["\#(h)"]}]"#)
        store.writeString("vela.signerPage.trusted", #"["\#(String(repeating: "ef", count: 32))"]"#)
        #expect(SignerPageChecks.storedTrusted(store, url: custom) == [h])
        #expect(SignerPageChecks.storedTrusted(store, url: "https://sign.other.example/").isEmpty)
        #expect(SignerPageChecks.storedTrusted(store, url: official).isEmpty, "the official page is never trusted into anything")
        store.writeString(SignerPageChecks.blockedKey, #"["\#(h)","not a hash"]"#)
        #expect(SignerPageChecks.storedBlocked(store) == [h])
    }

    /// The check fetches what a browser gets: exactly the core's headers.
    @Test func theCheckSendsTheCoresHeaders() throws {
        let headers = signerPageCheckHeaders()
        #expect(!headers.isEmpty)
        let request = SignerPageChecks.request(try #require(URL(string: custom)), headers: headers)
        for header in headers {
            #expect(request.value(forHTTPHeaderField: header.name) == header.value)
        }
        #expect(request.value(forHTTPHeaderField: "Accept")?.hasPrefix("text/html") == true)
        #expect(request.httpMethod == "GET")
    }

    /// The index is read as the desktop reads it: `{"versions": […]}` or a
    /// bare list, hashes only; anything else is an empty list.
    @Test func theIndexIsReadLeniently() {
        let h = String(repeating: "ab", count: 32)
        #expect(SignerPageChecks.listed(Data(#"{"versions":["\#(h)","x"]}"#.utf8)) == [h])
        #expect(SignerPageChecks.listed(Data(#"["\#(h.uppercased())"]"#.utf8)) == [h])
        #expect(SignerPageChecks.listed(Data("nonsense".utf8)).isEmpty)
        #expect(SignerPageChecks.indexUrl("https://a.example/x")?.absoluteString == "https://a.example/x/index.json")
        #expect(SignerPageChecks.indexUrl("https://a.example/x/")?.absoluteString == "https://a.example/x/index.json")
    }

    /// D-13: "checked {{time}}" is the core's moment — the clock time in the
    /// person's format today, with its date when the check was not today.
    @Test func checkedTimeIsTheCoresMomentInThePersonsFormat() throws {
        let saved = Formats.current
        defer { Formats.current = saved }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        var parts = DateComponents()
        (parts.year, parts.month, parts.day, parts.hour, parts.minute) = (2026, 6, 13, 12, 0)
        let noon = try #require(Calendar(identifier: .gregorian).date(from: parts))
        let now = UInt64(noon.timeIntervalSince1970 * 1000)
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let line = { (at: UInt64) in
            SignerIntegrityLine(state: .matches, version: "0ba8ee8c", checkedAtMs: at,
                                key: "componentsUi.signing.integrity.matches", opens: true)
        }
        let today = line(now - 2 * 60 * 1000).text(loc, nowMs: now)
        #expect(today.hasSuffix("checked 11:58"), "\(today)")
        let yesterday = line(now - 24 * Self.hour - 2 * 60 * 1000).text(loc, nowMs: now)
        #expect(yesterday.contains("2026-06-12") && yesterday.contains("11:58"), "\(yesterday)")
        #expect(!today.contains("{{") && !yesterday.contains("{{"))
        let zh = line(now - 2 * 60 * 1000).text(Loc(overrideTag: "zh", preferredLanguages: []), nowMs: now)
        #expect(zh.hasSuffix("检查于 11:58"), "\(zh)")
    }
}

/// The hand-off card (D4): what it says, in the corpus's words.
@MainActor
struct HandoffCardTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    @Test func theCardSaysWhichPageWhichKeyAndWhetherItMayOpen() {
        let line = SignerIntegrityLine(
            state: .matches, version: "0ba8ee8c", checkedAtMs: UInt64(Date().timeIntervalSince1970 * 1000),
            key: "componentsUi.signing.integrity.matches", opens: true
        )
        let card = HandoffCardModel.build(
            page: "https://sign.getvela.app/", keyLabel: TrustedSigner.keyLabel(.platform, loc: loc),
            line: line, loc: loc
        )
        #expect(card.title == "Review and sign on your trusted page")
        #expect(card.page == "sign.getvela.app")
        #expect(card.key == "Confirm with \(methodCopy(.platform, chooser: .signIn, loc: loc).title)")
        #expect(card.opens)
        let text = line.text(loc)
        #expect(text.hasPrefix("Version 0ba8ee8c · matches Vela's published build list · checked "))
        #expect(!text.contains("{{"), "a placeholder was left unfilled: \(text)")

        let refused = HandoffCardModel.build(
            page: "https://sign.getvela.app/", keyLabel: nil,
            line: SignerIntegrityLine(state: .couldNotCheck, version: "", checkedAtMs: nil,
                                      key: "componentsUi.signing.integrity.couldNotCheck", opens: false),
            loc: loc
        )
        #expect(!refused.opens)
        #expect(refused.key == nil)
        #expect(refused.line.text(loc) == "Couldn't check the page, so it won't open.")
    }

    /// The waiting and ending lines no longer name a "Trusted Signer".
    @Test func theWaitingAndEndingLinesAreReworded() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        for loc in [loc, zh] {
            let waiting = TrustedSignerSheetModel.copy(unreachable: false, loc: loc)
            for text in [waiting.title, waiting.hint ?? ""]
                + [TrustedSignerNotice.closed, .refused, .mismatch, .timeout].map({ $0.text(loc) }) {
                #expect(!text.contains("Trusted Signer") && !text.contains("可信签名器"), "\(text)")
            }
        }
    }

    /// Every integrity state has its sentence, and none promises more than a
    /// check of published bytes can.
    @Test func everyIntegrityLineHasWordsAndNoneOverclaims() {
        let states: [(SignerIntegrityState, String)] = [
            (.checking, "checking"), (.matches, "matches"), (.trustedHere, "trusted"), (.unchecked, "unchecked"),
            (.mismatch, "mismatch"), (.blocked, "blocked"), (.askToTrust, "askTrust"),
            (.couldNotCheck, "couldNotCheck"), (.noVersion, "noVersion"), (.allBlocked, "allBlocked"),
        ]
        for (state, leaf) in states {
            let line = SignerIntegrityLine(
                state: state, version: "12345678", checkedAtMs: 1, key: "componentsUi.signing.integrity.\(leaf)",
                opens: state == .matches || state == .trustedHere
            )
            let text = line.text(loc)
            #expect(text != line.key, "\(state) has no sentence")
            #expect(!text.contains("{{"))
            #expect(!text.localizedCaseInsensitiveContains("untampered"))
            #expect(!text.localizedCaseInsensitiveContains("certified"))
            #expect(IntegrityLineView.id(state) == leaf)
        }
    }
}
