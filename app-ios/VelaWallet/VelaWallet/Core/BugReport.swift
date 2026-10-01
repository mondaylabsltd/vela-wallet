//
//  BugReport.swift
//  VelaWallet
//
//  The in-app report, actually sent (round 3, founder: "要补") — the web's
//  `services/bug-report.ts` and the desktop's `executor/bug_report.rs`, one
//  for one: the same five-field payload, the same redaction, the same 16 000
//  character cap, the same 10 s budget, the same fallback road.
//
//  ## Two roads
//
//  The first is `getvela.app/api/bug-report`: a server-side token files the
//  issue, so somebody without a GitHub account can still report a bug. It
//  answers 503 `not_configured` while the token is not provisioned, 429 after
//  five reports in ten minutes from one address, 413 past the cap. None of
//  those is the person's fault and none may end with their report on the
//  floor — every non-2xx and every network fault hands back the second road,
//  the prefilled GitHub issue form.
//
//  ## The GitHub gotcha this file encodes
//
//  With `template=bug.yml` GitHub IGNORES `&body=`: an issue-form prefill
//  addresses the form's own field ids (`what`, `steps`, `environment`,
//  `area`). A URL built the obvious way opens an empty form.
//
//  ## What may never be in a report
//
//  Addresses, balances, endpoint or RPC URLs, raw `vela.*` values. The payload
//  is ASSEMBLED from what the person typed and the preview lines the sheet
//  shows — which `SettingsLive.withFeedback` builds from an allowlist and
//  redacts — so there is no road from the wallet's shelf to the wire.
//

import Foundation

enum BugReport {

    /// The site's proxy. The token lives there; nothing here has one.
    static let endpoint = "https://getvela.app/api/bug-report"
    /// The repository's issue form, for the fallback road and the sheet's link.
    static let issueForm = "https://github.com/mondaylabsltd/vela-wallet/issues/new"
    /// The endpoint's own cap (`MAX_BODY_CHARS`), honoured before the request.
    static let maxChars = 16_000
    /// The web's and the desktop's budget (`NET_TIMEOUTS.bundlerRest`).
    static let timeout: TimeInterval = 10
    /// With screenshots attached: up to five ~300 KB images ride in the body.
    static let screenshotTimeout: TimeInterval = 30
    /// The issue form's `area` dropdown, spelled as `.github/ISSUE_TEMPLATE/
    /// bug.yml` spells it; the sheet has no picker, and the person's own words
    /// are where the area actually is.
    static let areaOther = "Other (explain above)"

    /// What this client is, for the issue title's tag (spec §E): the backend
    /// titles an iOS report "[iOS] …" and opens it "Platform: iOS 26.0. App v0.9.4."
    static let client = "ios"

    /// The OS as this phone reports it — "iOS 26.5.2", the same string the
    /// preview's Platform line shows. Never a user agent, never an address.
    static var deviceOS: String {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "iOS \(v.majorVersion).\(v.minorVersion)" + (v.patchVersion > 0 ? ".\(v.patchVersion)" : "")
    }

    /// The text fields the endpoint accepts, and the screenshots.
    struct Payload: Codable, Equatable {
        let what: String
        let steps: String
        let area: String
        /// The preview lines, joined — IDENTICAL to what the sheet showed.
        let environment: String
        /// Dedup marker: stable for the same complaint, meaningless alone.
        let fingerprint: String
        /// Spec §E: which client, its OS in one short line, and the app's
        /// version without a "v" or a commit. A backend older than these
        /// fields ignores them (it reads only the keys it knows).
        var client: String = BugReport.client
        var os: String = ""
        var appVersion: String = ""
        /// Plain base64 (standard alphabet, padded, no line breaks, no
        /// `data:` prefix) of each prepared JPEG, in tile order. ABSENT —
        /// not an empty list — when none is attached, so a text-only report
        /// is byte for byte what it was.
        var screenshots: [String]? = nil

        /// The same report without its images — what the text cap measures.
        var textOnly: Payload {
            Payload(what: what, steps: steps, area: area, environment: environment, fingerprint: fingerprint,
                    client: client, os: os, appVersion: appVersion)
        }
    }

    enum Outcome: Equatable {
        /// Filed on the tracker, as a new issue or a +1 on an open one.
        /// `screenshotsDropped` > 0: filed, but that many images could not be
        /// stored — the sheet says so.
        case filed(number: Int, url: String, deduped: Bool, screenshotsDropped: Int)
        /// The endpoint could not file it; `url` is the prefilled form, and a
        /// caller that shows anything else is dropping the person's report.
        case fallback(reason: Reason, url: String)
    }

    enum Reason: String, Equatable {
        case notConfigured = "not_configured"
        case rateLimited = "rate_limited"
        case tooLarge = "too_large"
        case rejected
        case unreachable
    }

    /// The whole payload, from the person's words and the shown lines only.
    static func build(what: String, steps: String, area: String = areaOther,
                      environmentLines: [String], version: String, os: String = deviceOS,
                      screenshots: [Data] = []) -> Payload {
        let what = what.trimmingCharacters(in: .whitespacesAndNewlines)
        return Payload(
            what: what,
            steps: steps.trimmingCharacters(in: .whitespacesAndNewlines),
            area: area,
            environment: environmentLines.joined(separator: "\n"),
            fingerprint: fingerprint(what: what, area: area, version: version),
            client: client,
            os: os,
            appVersion: version,
            screenshots: screenshots.isEmpty
                ? nil
                : screenshots.prefix(ScreenshotPrep.maxCount).map { $0.base64EncodedString() }
        )
    }

    /// FNV-1a over the UTF-16 units the web hashes (`charCodeAt`), so an
    /// iPhone's report and a browser's report of the same words on the same
    /// build collide — which is the point, and safe only because the value
    /// says nothing about either person.
    static func fingerprint(what: String, area: String, version: String) -> String {
        let words = what.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().utf16.prefix(120)
        var hash: UInt32 = 0x811c_9dc5
        for unit in Array(words) + Array("|\(area)|\(version)".utf16) {
            hash ^= UInt32(unit)
            hash = hash &* 0x0100_0193
        }
        return String(format: "%08x", hash)
    }

    /// The prefilled issue form — by the form's FIELD IDS, in the web's order,
    /// encoded as `URLSearchParams` writes them.
    static func prefilledIssueURL(_ payload: Payload) -> String {
        var params: [(String, String)] = [
            ("template", "bug.yml"),
            ("title", issueTitle(payload.what)),
            ("what", payload.what),
            // The form marks steps required; an empty box beats a missing one.
            ("steps", payload.steps),
            ("environment", payload.environment),
        ]
        if !payload.area.isEmpty { params.append(("area", payload.area)) }
        let query = params.map { "\(formEncode($0.0))=\(formEncode($0.1))" }.joined(separator: "&")
        return "\(issueForm)?\(query)"
    }

    /// "[iOS] <first line of what, ≤ 80 characters>" (spec §E) — the tag the
    /// backend puts on a filed report, so the form's issue reads the same.
    static func issueTitle(_ what: String) -> String {
        let line = what.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty } ?? ""
        return "[iOS] " + String(decoding: Array(line.utf16.prefix(80)), as: UTF16.self)
    }

    /// `application/x-www-form-urlencoded`: unreserved bytes as they are, a
    /// space as `+`, every other UTF-8 byte as `%XX`.
    static func formEncode(_ value: String) -> String {
        var out = ""
        for byte in value.utf8 {
            switch byte {
            case UInt8(ascii: "a")...UInt8(ascii: "z"), UInt8(ascii: "A")...UInt8(ascii: "Z"),
                 UInt8(ascii: "0")...UInt8(ascii: "9"),
                 UInt8(ascii: "*"), UInt8(ascii: "-"), UInt8(ascii: "."), UInt8(ascii: "_"):
                out.append(Character(Unicode.Scalar(byte)))
            case UInt8(ascii: " "):
                out.append("+")
            default:
                out += String(format: "%%%02X", byte)
            }
        }
        return out
    }

    /// One HTTP exchange. A seam so tests stub the network: nothing a test
    /// runs may ever file a real issue.
    typealias Transport = (URLRequest) async throws -> (Data, URLResponse)

    static let liveTransport: Transport = { request in
        let config = URLSessionConfiguration.ephemeral
        config.waitsForConnectivity = false
        return try await URLSession.vela(config).data(for: request)
    }

    /// Where reports go: the site's proxy, or — `VELA_BUG_REPORT_ENDPOINT` — a
    /// stand-in, so a verification pass never files a real issue.
    static var configuredEndpoint: String {
        ProcessInfo.processInfo.environment["VELA_BUG_REPORT_ENDPOINT"] ?? endpoint
    }

    /// Send it, or hand back the road that still works.
    static func send(
        _ payload: Payload, endpoint: String = configuredEndpoint, transport: Transport = liveTransport
    ) async -> Outcome {
        let fallback = prefilledIssueURL(payload)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.withoutEscapingSlashes]
        guard let body = try? encoder.encode(payload) else {
            return .fallback(reason: .rejected, url: fallback)
        }
        // Checked here as well as there: a 413 round trip costs a spinner to
        // learn what this line knows before the request leaves. The cap is on
        // the TEXT; the images have their own, applied when they were made.
        let text = (try? encoder.encode(payload.textOnly)) ?? body
        if String(decoding: text, as: UTF8.self).utf16.count > maxChars {
            return .fallback(reason: .tooLarge, url: fallback)
        }
        guard let url = URL(string: endpoint) else { return .fallback(reason: .unreachable, url: fallback) }
        let budget = (payload.screenshots ?? []).isEmpty ? timeout : screenshotTimeout
        var request = URLRequest(url: url, timeoutInterval: budget)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await transport(request)
        } catch {
            print("[vela-wallet] bug report: network error — falling back")
            return .fallback(reason: .unreachable, url: fallback)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(status) else {
            print("[vela-wallet] bug report: endpoint answered \(status) — falling back")
            // 400 too_many/invalid_screenshot, 413 screenshot_too_large and
            // 415 unsupported_screenshot are refusals like any other: the
            // GitHub form carries the words, and the sheet says the images
            // must be added there.
            let reason: Reason = switch status {
            case 503: .notConfigured
            case 429: .rateLimited
            case 413: .tooLarge
            default: .rejected
            }
            return .fallback(reason: reason, url: fallback)
        }
        // A 200 this client cannot read is not a filed report it can point at.
        guard let filed = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let number = (filed["number"] as? NSNumber)?.intValue,
              let issue = filed["url"] as? String
        else { return .fallback(reason: .rejected, url: fallback) }
        return .filed(
            number: number, url: issue, deduped: (filed["deduped"] as? Bool) ?? false,
            screenshotsDropped: (filed["screenshotsDropped"] as? NSNumber)?.intValue ?? 0
        )
    }
}
