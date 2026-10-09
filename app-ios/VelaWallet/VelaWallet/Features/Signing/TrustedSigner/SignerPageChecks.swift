//
//  SignerPageChecks.swift
//  VelaWallet
//
//  The phone's integrity check of a signing page (spec 102 R6, spec 076
//  T034): before the word "trusted" is used about a page, this device fetches
//  the very bytes it will open, hashes them, and lets the core rule.
//
//  The rule is the core's, end to end, and there is ONE of it for the check
//  and the launch:
//
//      SignerPageTarget.choose(base, index?, trusted, blocked)
//          → GET target.url()  (plain HTTPS, ephemeral, no WebView)
//          → signerPageHash(bytes)
//          → signerPageAdmit(target, hash?, failure, …)  → SignerPageAdmission
//
//  and only an admission builds a launch URL (`urlLaunch`). The URL that is
//  fetched is the URL that is opened, so "the phone checked one version and
//  opened another" cannot be written here. A failed, missing or mismatched
//  check admits nothing: the hand-off card says why and nothing opens.
//
//  What this does NOT claim (research §3): a server that serves one person
//  different bytes from the ones this fetch saw is not caught by any check
//  outside the browser. The line says "matches Vela's published build list",
//  never "verified" or "untampered".
//
//  A check vouches for 24 hours (the core's `MAX_CHECK_AGE_MS`); after that the
//  line reads "checking" and the page is fetched again before it opens. So
//  that Open almost never waits for that (D-14), every page in use is checked
//  again in the background once its check is half a day old — on start, on
//  every return to the foreground, and hourly — when the core says it is due
//  (`signerPageRefreshDue`). A refresh that could not complete keeps a ruling
//  that still vouches (`signerPageKeepOrReplace`), and a good line does not
//  flicker to "checking" while one runs (`signerIntegrityLineWhileChecking`).
//
//  Trust is per page (D-15): the versions a person trusted live on that saved
//  page (`vela.signingPages`, read through `signingPageTrusted`), and "Trust
//  this version" is the signing pages machine's `version_trusted`. The
//  device-wide `vela.signerPage.trusted` list is no longer read or written.
//

import Foundation
import Observation
import VelaCore

@MainActor
@Observable
final class SignerPageChecks {

    /// The app's one checker: the hand-off card, the signing sheet and
    /// Settings → Signing pages read the same admissions, so a row cannot say
    /// "matches" about a page the card would refuse.
    static let shared = SignerPageChecks()

    /// One GET: the body on HTTP 200, `nil` for anything else. Injected by
    /// tests, which never reach a network.
    typealias Fetch = @MainActor (URL) async -> Data?

    /// Where the endpoint lists what it still publishes (`dist/index.json`).
    static let indexPath = "index.json"

    /// The biggest page this reads. The signing page is one file of about
    /// 300 KB; a few megabytes is room to grow and a refusal to hash
    /// something absurd.
    static let maxBytes = 4 * 1024 * 1024

    /// How often a running app asks which pages are due a background check
    /// (the core's `SignerRefreshSchedule.poll_ms`, an hour).
    static var pollMs: UInt64 { signerPageRefreshSchedule().pollMs }

    /// The device's blocked versions (FR-010). Never synced.
    static let blockedKey = "vela.signerPage.blocked"

    private let fetchIndex: Fetch
    private let fetchPage: Fetch
    /// The versions trusted for ONE page — that page's own list (D-15).
    private let trustedFor: (String) -> [String]
    private let blocked: () -> [String]
    private let clock: () -> UInt64

    /// Stores "Trust this version" for a page: the signing pages machine's
    /// `version_trusted {url, version}`, answered once it is written. Set by
    /// the app (the machine lives in `SettingsStore`); `nil` trusts nothing.
    @ObservationIgnored var recordTrust: (@MainActor (_ url: String, _ version: String) async -> Void)?

    /// The last ruling per page, admitted or refused, by its normalised base.
    private var admissions: [String: SignerPageAdmission] = [:]
    /// Checks under way, so a card and a Settings row asking at once share
    /// one fetch.
    private var running: [String: Task<SignerPageAdmission, Never>] = [:]
    /// When a check of each page last started, completed or not — what the
    /// core's refresh rule rests an unreachable page by.
    private var lastAttempt: [String: UInt64] = [:]
    /// Bumped on every ruling — what a SwiftUI view watches.
    private(set) var revision = 0

    init(
        fetchIndex: Fetch? = nil,
        fetchPage: Fetch? = nil,
        trusted: ((String) -> [String])? = nil,
        blocked: (() -> [String])? = nil,
        recordTrust: (@MainActor (String, String) async -> Void)? = nil,
        clock: @escaping () -> UInt64 = { UInt64(Date().timeIntervalSince1970 * 1000) }
    ) {
        self.fetchIndex = fetchIndex ?? { await Self.get($0, limit: 256 * 1024) }
        self.fetchPage = fetchPage ?? { await Self.get($0, limit: Self.maxBytes, headers: signerPageCheckHeaders()) }
        self.trustedFor = trusted ?? { Self.storedTrusted(VelaStore(), url: $0) }
        self.blocked = blocked ?? { Self.storedBlocked(VelaStore()) }
        self.recordTrust = recordTrust
        self.clock = clock
    }

    var nowMs: UInt64 { clock() }

    // MARK: - Reading

    /// The ruling for `base` as it stands — fresh or not. `nil` before any
    /// check has finished.
    func admission(for base: String) -> SignerPageAdmission? {
        _ = revision
        return admissions[Self.key(base)]
    }

    /// The admission a launch may use NOW: admitted, and young enough to
    /// vouch for the page. `nil` means "check first".
    func openable(_ base: String) -> SignerPageAdmission? {
        guard let admission = admission(for: base), admission.isFresh(nowMs: nowMs) else { return nil }
        return admission
    }

    /// The integrity line to draw for `base`: while a check runs, the core's
    /// line for that (the standing verdict while it still vouches, else
    /// "checking"); "checking" before any ruling; else the ruling's own line
    /// (itself "checking" once too old).
    func line(for base: String) -> SignerIntegrityLine {
        _ = revision
        let key = Self.key(base)
        if running[key] != nil {
            return signerIntegrityLineWhileChecking(previous: admissions[key], nowMs: nowMs)
        }
        guard let admission = admissions[key] else { return Self.checking }
        return admission.line(nowMs: nowMs)
    }

    /// The full version a self-hosted page's own index proposed and this
    /// device has not decided about (`AskToTrust`) — what "Trust this
    /// version" stores. `nil` otherwise; the official page never asks.
    func versionAskingTrust(_ base: String) -> String? {
        admission(for: base)?.versionToTrust()
    }

    /// The person trusts `version` of the page at `base` on this device
    /// (D-15): stored on that page by the signing pages machine, then the
    /// page is checked again under it.
    func trust(_ base: String, version: String) async {
        guard let recordTrust else { return }
        await recordTrust(base, version)
        await check(base)
    }

    /// "Trust this version" where the line asks it: the version the standing
    /// check asked about, stored and checked again. Nothing when it did not
    /// ask.
    func trustAsked(_ base: String) async {
        guard let version = versionAskingTrust(base) else { return }
        await trust(base, version: version)
    }

    /// Is a check of `base` running right now?
    func isChecking(_ base: String) -> Bool {
        _ = revision
        return running[Self.key(base)] != nil
    }

    /// Is a background check of `base` due now (D-14)? The core's rule, over
    /// the standing check's time and the last attempt.
    func refreshDue(_ base: String) -> Bool {
        let key = Self.key(base)
        return signerPageRefreshDue(
            checkedAtMs: admissions[key]?.checkedAtMs(), lastAttemptMs: lastAttempt[key], nowMs: nowMs
        )
    }

    // MARK: - Checking

    /// Check `base` unless a ruling stands that has not gone stale; answers
    /// the ruling.
    @discardableResult
    func ensure(_ base: String) async -> SignerPageAdmission {
        if let standing = admission(for: base), standing.line(nowMs: nowMs).state != .checking {
            return standing
        }
        return await check(base)
    }

    /// Check `base` now, whatever stands — the card's "try again". Coalesced:
    /// a second caller waits for the first fetch. The result is kept through
    /// the core's rule: a check that could not complete does not throw away a
    /// ruling that still vouches.
    @discardableResult
    func check(_ base: String) async -> SignerPageAdmission {
        let key = Self.key(base)
        if let running = running[key] { return await running.value }
        lastAttempt[key] = nowMs
        let task = Task { @MainActor [weak self] () -> SignerPageAdmission in
            guard let self else {
                return signerPageAdmit(
                    target: SignerPageTarget.choose(base: base, index: nil, trusted: [], blocked: []),
                    observedHash: nil, failure: .notChecked, trusted: [], blocked: [],
                    verificationOff: false, checkedAtMs: 0
                )
            }
            return await self.run(base)
        }
        running[key] = task
        revision += 1
        let next = await task.value
        running[key] = nil
        let kept = signerPageKeepOrReplace(previous: admissions[key], next: next, nowMs: nowMs)
        admissions[key] = kept
        revision += 1
        return kept
    }

    /// Fire-and-forget, for a row that has just been drawn: checked when
    /// nothing stands or what stands has gone stale.
    func prime(_ base: String) {
        let key = Self.key(base)
        guard running[key] == nil,
              admissions[key] == nil || admissions[key]?.line(nowMs: nowMs).state == .checking
        else { return }
        Task { await ensure(base) }
    }

    /// The background refresh (D-14): every page in use whose check the core
    /// says is due is checked again, quietly — its line keeps the standing
    /// verdict meanwhile. Called on start, on every return to the foreground
    /// and every `signerPageRefreshSchedule().pollMs` while the app runs.
    func refresh(_ pages: [String]) {
        var seen: Set<String> = []
        for page in pages where seen.insert(Self.key(page)).inserted {
            guard running[Self.key(page)] == nil, refreshDue(page) else { continue }
            Task { await check(page) }
        }
    }

    /// R6 through this shell's own I/O. The index only NARROWS the choice
    /// (the core decides); one that cannot be read is no reason to stop — the
    /// launch version is asked for directly.
    private func run(_ base: String) async -> SignerPageAdmission {
        let trusted = trustedFor(base)
        let blocked = blocked()
        let index: [String]? = await Self.indexUrl(base).asyncFlatMap { url in
            await fetchIndex(url).map(Self.listed)
        }
        let target = SignerPageTarget.choose(base: base, index: index, trusted: trusted, blocked: blocked)
        var hash: String?
        var failure = SignerCheckFailure.notChecked
        if let url = target.url().flatMap(URL.init(string:)) {
            if let bytes = await fetchPage(url) {
                hash = signerPageHash(bytes: bytes)
            } else {
                failure = .unreachable
            }
        }
        let admission = signerPageAdmit(
            target: target, observedHash: hash, failure: failure,
            trusted: trusted, blocked: blocked,
            // Checking is never off on a phone: there is no setting for it.
            verificationOff: false,
            checkedAtMs: nowMs
        )
        VelaLog.notice(.sign, "signer page \(SigningPageNames.host(base)): \(admission.line(nowMs: nowMs).state)")
        return admission
    }

    /// Forget every ruling — a test's reset, and nothing else.
    func reset() {
        admissions.removeAll()
        lastAttempt.removeAll()
        running.values.forEach { $0.cancel() }
        running.removeAll()
        revision += 1
    }

    // MARK: - Pure helpers

    /// "checking": the line before any ruling.
    static var checking: SignerIntegrityLine {
        signerIntegrityLineWhileChecking(previous: nil, nowMs: 0)
    }

    /// The deployment's index, beside its pages.
    static func indexUrl(_ base: String) -> URL? {
        let trimmed = base.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let withSlash = trimmed.hasSuffix("/") ? trimmed : trimmed + "/"
        return URL(string: withSlash + indexPath)
    }

    /// The index's hashes: `{"versions": [...]}` or a bare list. Anything else
    /// is an empty list — the index has no authority, so a malformed one is
    /// not an error.
    static func listed(_ data: Data) -> [String] {
        let value = try? JSONSerialization.jsonObject(with: data)
        let rows = ((value as? [String: Any])?["versions"] as? [Any]) ?? (value as? [Any]) ?? []
        return rows.compactMap { $0 as? String }.compactMap(normalizedHash)
    }

    /// 64 hex characters, lower-cased; anything else is not a version.
    static func normalizedHash(_ text: String) -> String? {
        let hash = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard hash.count == 64, hash.allSatisfy(\.isHexDigit) else { return nil }
        return hash
    }

    /// The versions trusted for the page at `url`, as its saved entry keeps
    /// them (`vela.signingPages`) — the core reads the list.
    static func storedTrusted(_ store: VelaStore, url: String) -> [String] {
        signingPageTrusted(savedJson: store.rawValue(VelaStore.Key.signingPages) ?? "[]", url: url)
    }

    /// The device's blocked versions, as stored (a JSON array of hashes).
    static func storedBlocked(_ store: VelaStore) -> [String] {
        guard let raw = store.rawValue(blockedKey), let data = raw.data(using: .utf8),
              let rows = (try? JSONSerialization.jsonObject(with: data)) as? [Any]
        else { return [] }
        return rows.compactMap { $0 as? String }.compactMap(normalizedHash)
    }

    /// Bases are compared as the core normalises them.
    static func key(_ base: String) -> String {
        base.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    }

    /// The check's request: a plain GET with exactly the headers the core
    /// names (`signerPageCheckHeaders` — a browser's navigation `Accept`), so
    /// the host answers the check with the bytes it answers the launch with.
    nonisolated static func request(_ url: URL, headers: [SignerHttpHeader]) -> URLRequest {
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)
        request.httpMethod = "GET"
        for header in headers { request.setValue(header.value, forHTTPHeaderField: header.name) }
        return request
    }

    /// One plain GET: ephemeral (no cookie, no cache), bounded in time and in
    /// size. Every failure is the same answer — this device could not see the
    /// page — because that is what the core does with it: nothing opens.
    nonisolated static func get(_ url: URL, limit: Int, headers: [SignerHttpHeader] = []) async -> Data? {
        let request = request(url, headers: headers)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 10
        configuration.timeoutIntervalForResource = 15
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        let session = URLSession.vela(configuration)
        defer { session.finishTasksAndInvalidate() }
        do {
            let (data, response) = try await session.data(for: request)
            guard (response as? HTTPURLResponse)?.statusCode == 200, data.count <= limit else { return nil }
            return data
        } catch {
            return nil
        }
    }
}

private extension Optional {
    func asyncFlatMap<T>(_ transform: (Wrapped) async -> T?) async -> T? {
        guard let value = self else { return nil }
        return await transform(value)
    }
}

extension SignerIntegrityLine {
    /// The line in words: the core names the corpus key and writes "checked
    /// {{time}}" (`signerIntegrityTime`, D-13 — the clock time in the person's
    /// format, with its date when the check was not today); the shell fills
    /// the version.
    func text(_ loc: Loc, nowMs: UInt64 = UInt64(Date().timeIntervalSince1970 * 1000)) -> String {
        var vars: [String: String] = [:]
        if !version.isEmpty { vars["version"] = version }
        if let checkedAtMs {
            vars["time"] = Self.checkedTime(checkedAtMs, nowMs: nowMs, loc: loc)
        }
        return vars.isEmpty ? loc.t(key) : loc.t(key, vars: vars)
    }

    /// `{{time}}`: the core's words for the moment a check ran, in the
    /// person's date and time presets (`auto` resolved) and the app's
    /// language, at the device's UTC offset now.
    static func checkedTime(_ checkedAtMs: UInt64, nowMs: UInt64, loc: Loc) -> String {
        let now = Date(timeIntervalSince1970: Double(nowMs) / 1000)
        return signerIntegrityTime(
            checkedAtMs: checkedAtMs, nowMs: nowMs,
            utcOffsetMinutes: Int32(TimeZone.current.secondsFromGMT(for: now) / 60),
            dateFormat: Formats.resolve(Formats.current.date).rawValue,
            timeFormat: Formats.resolve(Formats.current.time).rawValue,
            language: loc.resolvedLanguage
        )
    }

    /// How the line is drawn: a quiet check for a page that may open, a
    /// warning for one that will not, and a spinner while the check runs.
    var tone: SignerIntegrityTone {
        switch state {
        case .checking: .checking
        case .matches, .trustedHere: .ok
        case .unchecked: .caution
        case .mismatch, .blocked, .askToTrust, .couldNotCheck, .noVersion, .allBlocked: .refused
        }
    }
}

enum SignerIntegrityTone: Equatable {
    case checking, ok, caution, refused
}
