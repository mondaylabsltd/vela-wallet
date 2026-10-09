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

    /// Records what was fetched, and answers from a script.
    @MainActor
    final class Server {
        var index: Data?
        var page: Data?
        private(set) var indexAsked: [URL] = []
        private(set) var pageAsked: [URL] = []

        func checks(trusted: [String] = [], blocked: [String] = [], clock: @escaping () -> UInt64) -> SignerPageChecks {
            SignerPageChecks(
                fetchIndex: { [self] url in indexAsked.append(url); return index },
                fetchPage: { [self] url in pageAsked.append(url); return page },
                lists: { (trusted, blocked) },
                clock: clock
            )
        }
    }

    private static let start: UInt64 = 1_800_000_000_000

    /// A person's own build, listed by its index and trusted on this device:
    /// the bytes ARE the version the URL names, so the page opens — at the
    /// address that was fetched, and nowhere else.
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
        // URL that was fetched.
        let launch = try admission.urlLaunch(
            requestJson: #"{"intent":{},"context":{}}"#, token: "t", nowMs: Self.start
        )
        #expect(launch.hasPrefix(fetched.absoluteString))
        #expect(checks.openable(custom) != nil)
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
            try admission.urlLaunch(requestJson: #"{"intent":{},"context":{}}"#, token: "t", nowMs: Self.start)
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

        now += 25 * 60 * 60 * 1000
        #expect(checks.line(for: custom).state == .checking)
        #expect(checks.openable(custom) == nil, "a day-old check still opened the page")
        await checks.ensure(custom)
        #expect(server.pageAsked.count == 2)
        #expect(checks.openable(custom) != nil)
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

    /// The device's two lists are read from their keys, hashes only.
    @Test func theDevicesListsAreReadFromTheirKeys() {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let h = String(repeating: "cd", count: 32)
        store.writeString(SignerPageChecks.trustedKey, #"["\#(h)","not a hash"]"#)
        let lists = SignerPageChecks.storedLists(store)
        #expect(lists.trusted == [h])
        #expect(lists.blocked.isEmpty)
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
            let text = line.text(loc, nowMs: 2)
            #expect(text != line.key, "\(state) has no sentence")
            #expect(!text.contains("{{"))
            #expect(!text.localizedCaseInsensitiveContains("untampered"))
            #expect(!text.localizedCaseInsensitiveContains("certified"))
            #expect(IntegrityLineView.id(state) == leaf)
        }
    }
}
