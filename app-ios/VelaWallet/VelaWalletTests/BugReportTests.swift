//
//  BugReportTests.swift
//  VelaWalletTests
//
//  The one-click report (round 3), against the web's and the desktop's own
//  vectors. The network is ALWAYS stubbed: no test here may file a real issue.
//

import Foundation
import Testing
@testable import VelaWallet

/// A scripted endpoint: records every request, answers what it is told.
@MainActor
final class StubEndpoint {
    enum Answer {
        case status(Int, String)
        case offline
    }

    var answer: Answer = .status(200, "{}")
    private(set) var requests: [URLRequest] = []

    var transport: BugReport.Transport {
        { [self] request in
            requests.append(request)
            switch answer {
            case .offline:
                throw URLError(.notConnectedToInternet)
            case .status(let code, let body):
                let response = HTTPURLResponse(
                    url: request.url!, statusCode: code, httpVersion: nil, headerFields: nil
                )!
                return (Data(body.utf8), response)
            }
        }
    }
}

@MainActor
struct BugReportTests {
    private let stubURL = "https://example.test/api/bug-report"

    /// The four things a report may never carry (the web's `SECRETS`).
    private let address = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"
    private let rpcWithKey = "https://eth-mainnet.g.alchemy.com/v2/SUPER-SECRET-KEY"

    private func lines(unreachable: [String] = ["Gnosis"]) -> [String] {
        let loc = Loc(overrideTag: "en")
        return SettingsLive.withFeedback(
            SettingsLive.FeedbackFacts(
                version: "1.0.0", commit: "abc1234", platform: "iOS 26.2",
                language: "en", unreachable: unreachable
            ),
            on: SettingsFixtures.build(.st15, loc: loc), loc: loc
        ).feedback.previewLines
    }

    private func payload(_ what: String = "Send froze", steps: String = "1. tap send") -> BugReport.Payload {
        BugReport.build(what: what, steps: steps, environmentLines: lines(), version: "1.0.0")
    }

    // MARK: - What a report is allowed to know

    @Test func carriesFiveFieldsAndNoSixth() throws {
        let data = try JSONEncoder().encode(payload())
        let keys = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any]).keys
        #expect(keys.sorted() == ["area", "environment", "fingerprint", "steps", "what"])
    }

    @Test func noAddressOrEndpointURLReachesTheWire() throws {
        // A custom network can be named anything — including its own RPC URL.
        let shown = lines(unreachable: [rpcWithKey, address])
        let report = BugReport.build(
            what: "Balances stopped updating", steps: "1. open\n2. wait",
            environmentLines: shown, version: "1.0.0"
        )
        let wire = String(decoding: try JSONEncoder().encode(report), as: UTF8.self)
        #expect(!wire.contains(address))
        #expect(!wire.contains("SUPER-SECRET-KEY"))
        #expect(!wire.contains("vela."))
        #expect(wire.contains("[url]") && wire.contains("[address]"))
    }

    /// The consent line, literal: the preview IS the payload.
    @Test func sendsExactlyTheLinesTheSheetShowed() {
        let shown = lines()
        #expect(BugReport.build(what: "x", steps: "", environmentLines: shown, version: "1.0.0").environment
            == shown.joined(separator: "\n"))
    }

    @Test func trimsWhatThePersonTyped() {
        let report = BugReport.build(what: "  Send froze \n", steps: "  1. tap  ", environmentLines: [], version: "1")
        #expect(report.what == "Send froze")
        #expect(report.steps == "1. tap")
        #expect(report.area == BugReport.areaOther)
    }

    // MARK: - The dedup marker (the web's FNV-1a over UTF-16)

    @Test func theFingerprintMatchesTheWebAndTheDesktop() {
        #expect(BugReport.fingerprint(what: "  Send FAILED on Base ", area: BugReport.areaOther, version: "1.0.0") == "c7e1a5b2")
        #expect(BugReport.fingerprint(what: "转账失败", area: BugReport.areaOther, version: "1.0.0") == "f7bb6d3d")
        let a = BugReport.fingerprint(what: "Send froze", area: BugReport.areaOther, version: "1.0.0")
        #expect(BugReport.fingerprint(what: "  SEND FROZE ", area: BugReport.areaOther, version: "1.0.0") == a)
        #expect(BugReport.fingerprint(what: "Receive froze", area: BugReport.areaOther, version: "1.0.0") != a)
    }

    // MARK: - The fallback road

    /// By the form's FIELD IDS — `body` is ignored with `template=bug.yml`;
    /// the desktop's own vector, byte for byte.
    @Test func thePrefilledFormAddressesItsFields() throws {
        let report = BugReport.build(
            what: "It broke & stayed broken", steps: "1. open\n2. send",
            environmentLines: [], version: "1.0.0"
        )
        let url = BugReport.prefilledIssueURL(report)
        #expect(url.hasPrefix("https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=%5Bbug%5D+It+broke+%26+stayed+broken&what="))
        #expect(url.contains("&steps=1.+open%0A2.+send&"))
        #expect(url.contains("&area=Other+%28explain+above%29"))
        #expect(!url.contains("body="))
        let parsed = try #require(URLComponents(string: url))
        let items = Dictionary(uniqueKeysWithValues: (parsed.queryItems ?? []).map { ($0.name, $0.value ?? "") })
        #expect(items["template"] == "bug.yml")
        #expect(items["what"]?.replacingOccurrences(of: "+", with: " ") == "It broke & stayed broken")
    }

    @Test(arguments: [(503, BugReport.Reason.notConfigured), (429, .rateLimited), (413, .tooLarge), (500, .rejected)])
    func aRefusalFallsBackRatherThanLosingTheReport(status: Int, reason: BugReport.Reason) async {
        let stub = StubEndpoint()
        stub.answer = .status(status, "{}")
        let outcome = await BugReport.send(payload(), endpoint: stubURL, transport: stub.transport)
        guard case .fallback(let why, let url) = outcome else {
            Issue.record("\(status) did not fall back: \(outcome)")
            return
        }
        #expect(why == reason)
        #expect(url.contains("what=Send+froze"))
    }

    @Test func offlineFallsBack() async {
        let stub = StubEndpoint()
        stub.answer = .offline
        let outcome = await BugReport.send(payload(), endpoint: stubURL, transport: stub.transport)
        guard case .fallback(.unreachable, let url) = outcome else {
            Issue.record("offline did not fall back as unreachable: \(outcome)")
            return
        }
        #expect(url.hasPrefix(BugReport.issueForm))
    }

    @Test func aFiledReportSaysWhichIssueItBecame() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":42,"url":"https://github.com/x/y/issues/42","deduped":true}"#)
        let report = payload()
        let outcome = await BugReport.send(report, endpoint: stubURL, transport: stub.transport)
        #expect(outcome == .filed(number: 42, url: "https://github.com/x/y/issues/42", deduped: true, screenshotsDropped: 0))
        // The request itself: one POST of JSON, the payload and nothing else,
        // with the web's 10 s budget.
        let request = try #require(stub.requests.first)
        #expect(stub.requests.count == 1)
        #expect(request.httpMethod == "POST")
        #expect(request.url?.absoluteString == stubURL)
        #expect(request.value(forHTTPHeaderField: "Content-Type") == "application/json")
        #expect(request.timeoutInterval == 10)
        let sent = try JSONDecoder().decode(BugReport.Payload.self, from: try #require(request.httpBody))
        #expect(sent == report)
    }

    @Test func aTwoHundredItCannotReadIsNotAFiledReport() async {
        let stub = StubEndpoint()
        stub.answer = .status(200, "not json")
        guard case .fallback(.rejected, _) = await BugReport.send(payload(), endpoint: stubURL, transport: stub.transport) else {
            Issue.record("an unreadable 200 was taken as filed")
            return
        }
    }

    /// Past the endpoint's cap it never leaves the phone.
    @Test func anOversizedReportFallsBackWithoutARequest() async {
        let stub = StubEndpoint()
        let huge = BugReport.build(
            what: String(repeating: "x", count: BugReport.maxChars), steps: "",
            environmentLines: [], version: "1"
        )
        guard case .fallback(.tooLarge, _) = await BugReport.send(huge, endpoint: stubURL, transport: stub.transport) else {
            Issue.record("an oversized report did not fall back as too large")
            return
        }
        #expect(stub.requests.isEmpty)
    }

    // MARK: - The sheet's sender

    @Test func theSenderGoesSendingThenFiled() async {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":7,"url":"https://github.com/x/y/issues/7"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        #expect(sender.state == .idle)
        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: lines(), version: "1.0.0") }
        await sending.value
        #expect(sender.state == .filed(number: 7, url: "https://github.com/x/y/issues/7", deduped: false, screenshotsDropped: 0))
    }

    @Test func theSenderOffersGitHubWhenRefusedAndCanTryAgain() async {
        let stub = StubEndpoint()
        stub.answer = .status(503, #"{"error":"not_configured"}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        await sender.send(what: "Froze", steps: "", previewLines: lines(), version: "1.0.0")
        guard case .fallback(let url, _) = sender.state else {
            Issue.record("no fallback: \(sender.state)")
            return
        }
        #expect(url.contains("what=Froze"))
        // "Try again" is a real answer to a 429 or a dropped connection.
        stub.answer = .status(200, #"{"number":8,"url":"https://github.com/x/y/issues/8"}"#)
        await sender.send(what: "Froze", steps: "", previewLines: lines(), version: "1.0.0")
        #expect(sender.state == .filed(number: 8, url: "https://github.com/x/y/issues/8", deduped: false, screenshotsDropped: 0))
        #expect(stub.requests.count == 2)
    }

    @Test func nothingTypedIsNothingSent() async {
        let stub = StubEndpoint()
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        #expect(!FeedbackSender.ready("   \n"))
        await sender.send(what: "  ", steps: "", previewLines: lines(), version: "1.0.0")
        #expect(sender.state == .idle)
        #expect(stub.requests.isEmpty)
    }

    /// Only a live sheet sends — the gallery's is a picture of itself.
    @Test func onlyTheLiveSheetMaySend() {
        let loc = Loc(overrideTag: "en")
        #expect(!SettingsFixtures.build(.st15, loc: loc).feedback.live)
        #expect(!lines().isEmpty)
        let live = SettingsLive.withFeedback(
            SettingsLive.FeedbackFacts(version: "1", commit: "c", platform: "iOS", language: "en", unreachable: []),
            on: SettingsFixtures.build(.st15, loc: loc), loc: loc
        )
        #expect(live.feedback.live)
        #expect(live.feedback.successBodyNew.contains("{{number}}"))
    }
}
