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

import CoreText
import Foundation
import UIKit
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

    /// PR 3 note 14: the moment is ONE unbreakable unit, CJK included. A
    /// no-break space keeps "2:32 PM" together and is not enough for a day
    /// period written in Han or kana — a line may break between any two such
    /// characters, and 「下午」 split after 「下」 at some widths. The core now
    /// puts a WORD JOINER (U+2060) between them. This shell neither strips
    /// nor re-inserts it: the line carries the core's string byte for byte,
    /// and laid out by CoreText in the line's own face at every width from
    /// a sliver to a full phone, the moment is never broken across lines.
    @Test func aCheckedTimeIsOneUnbreakableUnitInCJKToo() throws {
        let saved = Formats.current
        defer { Formats.current = saved }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h12)
        var parts = DateComponents()
        (parts.year, parts.month, parts.day, parts.hour, parts.minute) = (2026, 6, 13, 15, 0)
        let three = try #require(Calendar(identifier: .gregorian).date(from: parts))
        let now = UInt64(three.timeIntervalSince1970 * 1000)
        let at = now - 28 * 60 * 1000 // 2:32 in the afternoon, today
        let yesterday = at - 24 * Self.hour

        // The core's own words for the moment, in each script.
        func moment(_ tag: String, _ checked: UInt64) -> String {
            SignerIntegrityLine.checkedTime(checked, nowMs: now, loc: Loc(overrideTag: tag, preferredLanguages: []))
        }
        // A Latin-script moment is byte-identical to before.
        #expect(moment("en", at) == "2:32\u{a0}PM")
        // Han and kana carry the joiner inside the day period.
        #expect(moment("zh", at) == "下\u{2060}午\u{a0}2:32")
        #expect(moment("ja", at) == "午\u{2060}後\u{a0}2:32")
        // Nothing breakable is left inside any of them: no plain space, and
        // every pair of neighbours that could be parted is glued.
        for tag in ["en", "zh", "zh-TW", "zh-HK", "ja", "ko", "de", "ru", "tr", "vi"] {
            for checked in [at, yesterday] {
                let text = moment(tag, checked)
                #expect(!text.contains(" "), "\(tag): a plain space in \(text.debugDescription)")
                #expect(!text.isEmpty)
            }
        }

        // The line is the core's string, untouched.
        let key = "componentsUi.signing.integrity.matches"
        for tag in ["zh", "ja", "en"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            for checked in [at, yesterday] {
                let line = SignerIntegrityLine(state: .matches, version: "0ba8ee8c", checkedAtMs: checked, key: key, opens: true)
                let text = line.text(loc, nowMs: now)
                let time = moment(tag, checked)
                #expect(text.contains(time), "\(tag): the line re-spelled the moment: \(text.debugDescription)")
                #expect(!text.contains("{{"))

                // Laid out in the line's own face, at every width: the
                // moment's characters all sit on one line.
                let attributed = NSAttributedString(string: text, attributes: [.font: Typography.flowCaption.uiFont])
                let whole = text as NSString
                let range = whole.range(of: time)
                #expect(range.location != NSNotFound)
                let setter = CTFramesetterCreateWithAttributedString(attributed)
                let own = (time as NSString).size(withAttributes: [.font: Typography.flowCaption.uiFont]).width
                var widths = 0
                for width in stride(from: CGFloat(60), through: 400, by: 1) where width >= own + 2 {
                    widths += 1
                    let path = CGPath(rect: CGRect(x: 0, y: 0, width: width, height: 10_000), transform: nil)
                    let frame = CTFramesetterCreateFrame(setter, CFRange(location: 0, length: 0), path, nil)
                    let lines = (CTFrameGetLines(frame) as? [CTLine]) ?? []
                    let holders = lines.filter { line in
                        let span = CTLineGetStringRange(line)
                        return NSIntersectionRange(NSRange(location: span.location, length: span.length), range).length > 0
                    }
                    if holders.count != 1 {
                        Issue.record("\(tag): the moment \(time.debugDescription) is split over \(holders.count) lines at \(width) pt")
                        break
                    }
                }
                #expect(widths > 100, "\(tag): no width was wide enough to hold the moment (\(own) pt)")
            }
        }

        // And the joiner is what does it: the same day period without one
        // DOES break at some width — so the check above is not vacuous.
        let bare = "检查于 下午\u{a0}2:32"
        let bareAttributed = NSAttributedString(string: bare, attributes: [.font: Typography.flowCaption.uiFont])
        let bareSetter = CTFramesetterCreateWithAttributedString(bareAttributed)
        let period = (bare as NSString).range(of: "下午")
        var splits = false
        for width in stride(from: CGFloat(20), through: 200, by: 1) {
            let path = CGPath(rect: CGRect(x: 0, y: 0, width: width, height: 10_000), transform: nil)
            let frame = CTFramesetterCreateFrame(bareSetter, CFRange(location: 0, length: 0), path, nil)
            let lines = (CTFrameGetLines(frame) as? [CTLine]) ?? []
            let holders = lines.filter { line in
                let span = CTLineGetStringRange(line)
                return NSIntersectionRange(NSRange(location: span.location, length: span.length), period).length > 0
            }
            if holders.count > 1 { splits = true; break }
        }
        #expect(splits, "without the joiner 下午 never splits either — this layout cannot see the defect")
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
            page: "https://sign.getvela.app/",
            keyLabel: KeyLabelWire(placeKey: "onboarding.create.methodHybridTitle"),
            line: line, loc: loc
        )
        #expect(card.title == "Review and sign on a trusted signing page")
        // D6/D-19: the official page is named, not tagged.
        #expect(card.pageName == "Vela's official signing page")
        #expect(card.page == "sign.getvela.app")
        // A row, as the sheet draws "Signing account | name" — never a
        // sentence with the place inflected inside it.
        #expect(card.key == HandoffKeyRow(label: "Confirm with", value: "Phone or tablet"))
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let zhCard = HandoffCardModel.build(
            page: "https://sign.getvela.app/",
            keyLabel: KeyLabelWire(placeKey: "onboarding.create.methodHybridTitle"), line: line, loc: zh
        )
        #expect(zhCard.title == "在可信签名页上预览并签名")
        #expect(zhCard.key == HandoffKeyRow(label: "确认方式", value: "手机或平板"))
        #expect(card.opens)
        #expect(card.trust == nil, "the official page never asks to be trusted")
        let text = line.text(loc)
        // The "·" is bound to the word before it (U+00A0, PR 3): a line —
        // a CJK one above all — never starts with the dot.
        #expect(text.hasPrefix("Version 0ba8ee8c\u{00A0}· matches Vela's published build list\u{00A0}· checked "))
        #expect(!text.contains(" · "), "a breakable space before a separator dot: \(text)")
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

    /// D-17: a key the person named is named; a key that carries the
    /// wallet's name is named by its place — the core's label, drawn.
    @Test func theKeyIsNamedAsThePlanNamesIt() {
        #expect(KeyLabelWire(name: "YubiKey 5C", placeKey: "onboarding.create.methodSecurityKeyTitle").text(loc) == "YubiKey 5C")
        // A label written before the row had one reads "Confirm with".
        let old = try? CoreJSON.decoder.decode(
            KeyLabelWire.self, from: Data(#"{"place_key":"onboarding.create.methodPlatformTitle"}"#.utf8)
        )
        #expect(old?.labelKey == "componentsUi.signing.confirmWithLabel")
        #expect(old?.label(loc) == "Confirm with")
        #expect(KeyLabelWire(placeKey: "onboarding.create.methodPlatformTitle").text(loc)
                == loc.t("onboarding.create.methodPlatformTitle"))
        let plan = SigningPlanWire.of(accountJson: TrustedSignerFixture().pageRecordJson)
        #expect(plan?.keyLabel?.placeKey == "onboarding.create.methodPlatformTitle")
        #expect(plan?.keyLabel?.labelKey == "componentsUi.signing.confirmWithLabel")
        #expect(plan?.keyLabel?.name == nil, "a key named after the wallet repeated the account row")
    }

    /// D6: a self-hosted page is "Self-hosted · {{domain}}" unless the person
    /// named it; its own build asks to be trusted, with the answer beside it.
    @Test func aSelfHostedPageIsNamedAndAsksToBeTrusted() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let ask = SignerIntegrityLine(state: .askToTrust, version: "3f9a1c22", checkedAtMs: 1,
                                      key: "componentsUi.signing.integrity.askTrust", opens: false)
        let card = HandoffCardModel.build(page: "https://sign.example.com/", keyLabel: nil, line: ask, loc: zh)
        #expect(card.pageName == "自己部署的签名页\u{00A0}· \(signingPageDomain(url: "https://sign.example.com/"))")
        #expect(card.trust == "信任这个版本")
        #expect(!card.opens)
        let named = HandoffCardModel.build(page: "https://sign.example.com/", keyLabel: nil, line: ask, loc: zh, name: "Work")
        #expect(named.pageName == "Work")
        for text in [card.pageName, card.title, card.lineText] {
            #expect(!text.contains("我自己的签名页"))
        }
    }

    /// Core round 12: a ceremony on a page has its own title, never "Review
    /// and sign".
    @Test func aCeremonyHasItsOwnTitle() throws {
        let page = "https://sign.example.com/"
        let create = try #require(trustedSignerCeremonyTitleKey(
            operationJson: #"{"type":"register_passkey","name":"Mine","page":"\#(page)"}"#
        ))
        #expect(loc.t(create) == "Create a key on the signing page")
        let card = HandoffCardModel.build(page: page, keyLabel: nil, line: SignerPageChecks.checking,
                                          loc: loc, title: loc.t(create))
        #expect(card.title == "Create a key on the signing page")
        #expect(trustedSignerCeremonyTitleKey(operationJson: #"{"type":"nothing"}"#) == nil)
    }

    /// Spec 102 integration: a ceremony's card has its key row, the core's —
    /// "New key on | This device" while a key is made, "Confirm with | Phone
    /// or tablet" when one signs in. By place, never a name.
    @Test func aCeremonyHasItsKeyRow() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let create = try #require(KeyLabelWire.ofCeremony(
            operationJson: #"{"type":"register_passkey","name":"Everyday wallet","method":"platform"}"#
        ))
        #expect(create.name == nil, "a key being made was named")
        #expect(HandoffKeyRow(create, loc: loc) == HandoffKeyRow(label: "New key on", value: "This device"))
        #expect(HandoffKeyRow(create, loc: zh) == HandoffKeyRow(label: "新钥匙存在", value: "这台设备"))
        let signIn = try #require(KeyLabelWire.ofCeremony(
            operationJson: #"{"type":"authenticate_passkey","method":"hybrid"}"#
        ))
        #expect(HandoffKeyRow(signIn, loc: zh) == HandoffKeyRow(label: "确认方式", value: "手机或平板"))
        #expect(KeyLabelWire.ofCeremony(operationJson: #"{"type":"nothing"}"#) == nil)
    }

    /// Polish 3: "Keys on {{domain}}" only where the domain is not the page's
    /// own host.
    @Test func keysOnIsSaidOnlyWhereItIsNews() {
        #expect(SigningPageNames.keysOnLine(url: "https://sign.getvela.app/", domain: "getvela.app", loc: loc)
                == "Keys on getvela.app")
        #expect(SigningPageNames.keysOnLine(url: "https://sign.example.com/", domain: "sign.example.com", loc: loc) == nil)
        #expect(SigningPageNames.keysOnLine(url: "https://SIGN.example.com/x/", domain: "sign.example.com", loc: loc) == nil)
        #expect(SigningPageNames.keysOnLine(url: "https://sign.example.com/", domain: "", loc: loc) == nil)
        // The choosers say the same.
        let choices = SigningPagePickerModel.choices(
            pages: SigningPageFixtures.pages, selected: nil, loc: loc, line: SigningPageFixtures.askingLine,
            asksTrust: SigningPageFixtures.asksTrust
        )
        #expect(choices.map(\.domainLine) == [nil, "Keys on getvela.app", nil])
        // …and offer the answer where the line asks it — never for the
        // official page or Vela's own sheet.
        #expect(choices.map { $0.trust?.version } == [nil, nil, SigningPageFixtures.askingVersion])
    }

    /// Core round 5: the card's fee row is the fee settled for the speed in
    /// force, with that speed — no row while it is measured, not ready, or
    /// another speed's; no speed restated where there is one.
    @Test func theFeeRowIsTheSettledFeeForTheSpeedInForce() throws {
        let row = try #require(HandoffFeeModel.of(
            feeJson: HandoffFeeFixtures.feeJson(tier: "fast"), speedJson: HandoffFeeFixtures.speedJson(tier: "fast"),
            fee: nil, display: .usd, networks: .builtin, loc: loc
        ))
        #expect(row.label == "Network fee")
        #expect(row.value.hasPrefix("~") && row.value.contains("USDC"), "\(row.value)")
        #expect(row.tier == loc.t("send.gasTier.fast"))
        let single = HandoffFeeModel.of(
            feeJson: HandoffFeeFixtures.feeJson(), speedJson: HandoffFeeFixtures.speedJson(single: true),
            fee: nil, display: .usd, networks: .builtin, loc: loc
        )
        #expect(single != nil && single?.tier == nil)
        for (fee, speed) in [
            (HandoffFeeFixtures.feeJson(busy: true), HandoffFeeFixtures.speedJson()),
            (HandoffFeeFixtures.feeJson(ready: false), HandoffFeeFixtures.speedJson()),
            (HandoffFeeFixtures.feeJson(tier: "standard"), HandoffFeeFixtures.speedJson(tier: "fast")),
        ] {
            #expect(HandoffFeeModel.of(feeJson: fee, speedJson: speed, fee: nil, display: .usd,
                                       networks: .builtin, loc: loc) == nil)
        }
        #expect(HandoffFeeModel.of(feeJson: nil, speedJson: nil, fee: nil, display: .usd,
                                   networks: .builtin, loc: loc) == nil, "a message has no fee row")
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
