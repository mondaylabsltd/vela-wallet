//
//  RelayReportTests.swift
//  VelaWalletTests
//
//  Issue #466: "Report this" on both relay stops files the report the core
//  built for the stop — through the app's own report sheet, under the core's
//  area and fingerprint — instead of opening an empty GitHub form. The network
//  is ALWAYS stubbed: no test here may file a real issue.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct RelayReportTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let stubURL = "https://example.test/api/bug-report"
    private let treasury = "0x3e59292e18417f814112f731e7163534c6d2fe3c"

    private struct Missing: Error {}

    private func sendView(_ patch: [String: Any]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private var emptyFloat: [String: Any] {
        [
            "chain_id": 130, "address": treasury,
            "asset": "native", "balance": "0", "floor": "100000000000000",
            "bootstrap_needed": true, "operator_served": true,
            "coin": ["symbol": "ETH", "balance": "0", "floor": "0.0001", "suggested": "0.0001"],
        ]
    }

    /// The core's report, as `send.rs` builds it for this stop.
    private var gasReport: [String: Any] {
        [
            "what": "Relayer out of gas on Unichain (130)\n\nTreasury: \(treasury)\n"
                + "Has 0 ETH of its 0.0001 ETH floor (short 0.0001 ETH).",
            "steps": "1. Send on Unichain (130)\n2. Continue: the relay's treasury check stopped the send",
            "area": "Send",
            "fingerprint": "relay-gas-130",
        ]
    }

    private func confirm(_ view: SendViewWire) throws -> SendConfirmModel {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            throw Missing()
        }
        return SendLive.confirm(
            view, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawn, loc: loc
        )
    }

    // MARK: - The button

    /// Both stops carry "Report this" — on the form, where the core opens
    /// them at Continue, and on confirm — in each stop's own words.
    @Test func bothStopsOfferReportThisOnBothPages() throws {
        let gasForm = try sendView([
            "stage": "enter_details", "treasury_bootstrap": emptyFloat, "relay_report": gasReport,
        ])
        #expect(SendLive.reportLabel(gasForm, loc: loc) == loc.t("componentsUi.treasuryBootstrap.reportBtn"))
        let gasConfirm = try sendView([
            "stage": "confirm", "treasury_bootstrap": emptyFloat, "relay_report": gasReport,
        ])
        #expect(try confirm(gasConfirm).noticeReport == loc.t("componentsUi.treasuryBootstrap.reportBtn"))

        var unreachable = gasReport
        unreachable["what"] = "Relay can't reach Unichain (130)"
        unreachable["fingerprint"] = "relay-unreachable-130"
        let stop: [String: Any] = ["chain_id": 130, "operator_served": true]
        let cantForm = try sendView([
            "stage": "enter_details", "relay_unreachable": stop, "relay_report": unreachable,
        ])
        #expect(SendLive.reportLabel(cantForm, loc: loc) == loc.t("componentsUi.relayUnreachable.reportBtn"))
        let cantConfirm = try sendView([
            "stage": "confirm", "relay_unreachable": stop, "relay_report": unreachable,
        ])
        #expect(try confirm(cantConfirm).noticeReport == loc.t("componentsUi.relayUnreachable.reportBtn"))
        #expect(!loc.t("componentsUi.relayUnreachable.reportBtn").contains("reportBtn"), "the corpus has the key")
    }

    /// No report from the core, no button: a network the person added has no
    /// operator to tell, and with no stop up there is nothing to report.
    @Test func noReportNoButton() throws {
        var custom = emptyFloat
        custom["operator_served"] = false
        let added = try sendView(["stage": "enter_details", "treasury_bootstrap": custom])
        #expect(SendLive.reportLabel(added, loc: loc) == nil)
        #expect(try confirm(sendView(["stage": "confirm", "treasury_bootstrap": custom])).noticeReport == nil)
        #expect(SendLive.reportLabel(try sendView(["stage": "enter_details"]), loc: loc) == nil)
        #expect(SendLive.reportSeed(try sendView(["stage": "enter_details"])) == nil)
    }

    // MARK: - What it files

    /// The tap snapshots the core's report whole: its words for the sheet,
    /// its area and its fingerprint for the send.
    @Test func theTapSeedsTheSheetWithTheCoresReport() throws {
        let view = try sendView([
            "stage": "enter_details", "treasury_bootstrap": emptyFloat, "relay_report": gasReport,
        ])
        let seed = try #require(SendLive.reportSeed(view))
        #expect(seed.what.hasPrefix("Relayer out of gas on Unichain (130)"))
        #expect(seed.what.contains(treasury), "the treasury rides in `what`, whole")
        #expect(seed.steps.contains("Continue"))
        #expect(seed.area == "Send")
        #expect(seed.fingerprint == "relay-gas-130")
    }

    /// The payload files under the core's area and fingerprint — not "Other"
    /// and not a hash of the words, which would open one issue per app
    /// version and per balance reading. The treasury survives: `what` is kept
    /// as written, only the environment lines are redacted.
    @Test func theReportFilesUnderTheCoresAreaAndFingerprint() throws {
        let seed = try #require(SendLive.reportSeed(sendView([
            "stage": "enter_details", "treasury_bootstrap": emptyFloat, "relay_report": gasReport,
        ])))
        let payload = BugReport.build(
            what: seed.what, steps: seed.steps, area: seed.area,
            environmentLines: ["Version: v0.9.7 (abc)"], version: "0.9.7",
            fingerprint: seed.fingerprint
        )
        #expect(payload.area == "Send")
        #expect(payload.fingerprint == "relay-gas-130")
        #expect(payload.what.contains(treasury))
        // Edited words still land on the same issue.
        let edited = BugReport.build(
            what: seed.what + "\nStill stuck an hour later.", steps: seed.steps, area: seed.area,
            environmentLines: [], version: "0.9.8", fingerprint: seed.fingerprint
        )
        #expect(edited.fingerprint == payload.fingerprint)
        // Without a seed, the word-derived fingerprint, as before.
        let plain = BugReport.build(what: "Froze", steps: "", environmentLines: [], version: "1")
        #expect(plain.fingerprint == BugReport.fingerprint(what: "Froze", area: BugReport.areaOther, version: "1"))
        #expect(plain.area == BugReport.areaOther)
        // The fallback form carries the area and the title line.
        let url = BugReport.prefilledIssueURL(payload)
        #expect(url.contains("area=Send"))
        #expect(url.contains("title=%5BiOS%5D+Relayer+out+of+gas+on+Unichain+%28130%29"))
    }

    /// The sheet's sender, seeded: what leaves the phone is the seed's words,
    /// area and fingerprint, to the (stubbed) endpoint.
    @Test func theSeededSenderPostsTheCoresReport() async throws {
        let stub = StubEndpoint()
        stub.answer = .status(200, #"{"number":4243,"url":"https://github.com/x/y/issues/4243","deduped":true}"#)
        let sender = FeedbackSender(endpoint: stubURL, transport: stub.transport)
        let seed = BugReport.Seed(what: gasReport["what"] as! String, steps: gasReport["steps"] as! String,
                                  area: "Send", fingerprint: "relay-gas-130")
        await sender.send(what: seed.what, steps: seed.steps, previewLines: ["Version: v0.9.7"],
                          version: "0.9.7", seed: seed)
        #expect(sender.state == .filed(number: 4243, url: "https://github.com/x/y/issues/4243",
                                       deduped: true, screenshotsDropped: 0))
        let request = try #require(stub.requests.first)
        #expect(request.url?.absoluteString == stubURL)
        let body = try #require(request.httpBody)
        let sent = try JSONDecoder().decode(BugReport.Payload.self, from: body)
        #expect(sent.area == "Send")
        #expect(sent.fingerprint == "relay-gas-130")
        #expect(sent.what.hasPrefix("Relayer out of gas on Unichain (130)"))
        #expect(sent.steps.hasPrefix("1. Send on Unichain (130)"))
        #expect(sent.client == "ios")
    }
}
