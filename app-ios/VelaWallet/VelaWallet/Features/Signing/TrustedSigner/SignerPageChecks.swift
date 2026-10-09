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
//  line reads "checking" and the page is fetched again before it opens.
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

    /// The device's lists (FR-009, FR-010): hashes the person trusted, and
    /// hashes they blocked. Never synced.
    static let trustedKey = "vela.signerPage.trusted"
    static let blockedKey = "vela.signerPage.blocked"

    private let fetchIndex: Fetch
    private let fetchPage: Fetch
    private let lists: () -> (trusted: [String], blocked: [String])
    private let addTrusted: (String) -> Void
    private let clock: () -> UInt64

    /// The last ruling per page, admitted or refused, by its normalised base.
    private var admissions: [String: SignerPageAdmission] = [:]
    /// Checks under way, so a card and a Settings row asking at once share
    /// one fetch.
    private var running: [String: Task<SignerPageAdmission, Never>] = [:]
    /// Bumped on every ruling — what a SwiftUI view watches.
    private(set) var revision = 0

    init(
        fetchIndex: Fetch? = nil,
        fetchPage: Fetch? = nil,
        lists: (() -> (trusted: [String], blocked: [String]))? = nil,
        addTrusted: ((String) -> Void)? = nil,
        clock: @escaping () -> UInt64 = { UInt64(Date().timeIntervalSince1970 * 1000) }
    ) {
        self.fetchIndex = fetchIndex ?? { await Self.get($0, limit: 256 * 1024) }
        self.fetchPage = fetchPage ?? { await Self.get($0, limit: Self.maxBytes) }
        self.lists = lists ?? { Self.storedLists(VelaStore()) }
        self.addTrusted = addTrusted ?? { Self.storeTrusted($0, in: VelaStore()) }
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
        guard let admission = admission(for: base), admission.opens(),
              admission.line(nowMs: nowMs).opens
        else { return nil }
        return admission
    }

    /// The integrity line to draw for `base`: "checking" until a check has
    /// ruled (or once it is too old), else the ruling's own line.
    func line(for base: String) -> SignerIntegrityLine {
        guard let admission = admission(for: base) else { return Self.checking }
        return admission.line(nowMs: nowMs)
    }

    /// The full version a custom page's own index proposed and this device
    /// has not decided about (`AskToTrust`) — what "Confirm" trusts. `nil`
    /// otherwise; the official page never asks (D-7).
    func versionAskingTrust(_ base: String) -> String? {
        guard case .askToTrust(let actual)? = admission(for: base)?.verdict() else { return nil }
        return Self.normalizedHash(actual)
    }

    /// The person trusts `version` on this device (FR-009) — added to
    /// `vela.signerPage.trusted` — and the page is checked again under it.
    func trust(_ base: String, version: String) async {
        guard let hash = Self.normalizedHash(version) else { return }
        addTrusted(hash)
        await check(base)
    }

    /// Is a check of `base` running right now?
    func isChecking(_ base: String) -> Bool {
        _ = revision
        return running[Self.key(base)] != nil
    }

    // MARK: - Checking

    /// Check `base` unless a fresh ruling stands; answers the ruling.
    @discardableResult
    func ensure(_ base: String) async -> SignerPageAdmission {
        if let standing = admission(for: base), standing.line(nowMs: nowMs).state != .checking {
            return standing
        }
        return await check(base)
    }

    /// Check `base` now, whatever stands — the card's "try again". Coalesced:
    /// a second caller waits for the first fetch.
    @discardableResult
    func check(_ base: String) async -> SignerPageAdmission {
        let key = Self.key(base)
        if let running = running[key] { return await running.value }
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
        let admission = await task.value
        running[key] = nil
        admissions[key] = admission
        revision += 1
        return admission
    }

    /// Fire-and-forget, for a row that has just been drawn.
    func prime(_ base: String) {
        guard admission(for: base) == nil || line(for: base).state == .checking,
              running[Self.key(base)] == nil
        else { return }
        Task { await ensure(base) }
    }

    /// R6 through this shell's own I/O. The index only NARROWS the choice
    /// (the core decides); one that cannot be read is no reason to stop — the
    /// launch version is asked for directly.
    private func run(_ base: String) async -> SignerPageAdmission {
        let (trusted, blocked) = lists()
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
        running.values.forEach { $0.cancel() }
        running.removeAll()
        revision += 1
    }

    // MARK: - Pure helpers

    /// "checking": the line before any ruling.
    static var checking: SignerIntegrityLine {
        SignerIntegrityLine(
            state: .checking, version: "", checkedAtMs: nil,
            key: "componentsUi.signing.integrity.checking", opens: false
        )
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

    /// The two per-device lists, as stored (JSON arrays of hashes).
    static func storedLists(_ store: VelaStore) -> (trusted: [String], blocked: [String]) {
        func list(_ key: String) -> [String] {
            guard let raw = store.rawValue(key), let data = raw.data(using: .utf8),
                  let rows = (try? JSONSerialization.jsonObject(with: data)) as? [Any]
            else { return [] }
            return rows.compactMap { $0 as? String }.compactMap(normalizedHash)
        }
        return (list(trustedKey), list(blockedKey))
    }

    /// One hash added to the device's trusted list, once.
    static func storeTrusted(_ hash: String, in store: VelaStore) {
        var trusted = storedLists(store).trusted
        guard !trusted.contains(hash) else { return }
        trusted.append(hash)
        let json = (try? JSONSerialization.data(withJSONObject: trusted)).map { String(decoding: $0, as: UTF8.self) }
        store.writeString(trustedKey, json)
    }

    /// Bases are compared as the core normalises them.
    static func key(_ base: String) -> String {
        base.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    }

    /// One plain GET: ephemeral (no cookie, no cache), bounded in time and in
    /// size. Every failure is the same answer — this device could not see the
    /// page — because that is what the core does with it: nothing opens.
    nonisolated static func get(_ url: URL, limit: Int) async -> Data? {
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)
        request.httpMethod = "GET"
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
    /// The line in words: the core names the corpus key, the shell fills the
    /// version and "checked {{time}}" — the check's moment as the person reads
    /// times (their own format; a check vouches for a day at most).
    func text(_ loc: Loc) -> String {
        var vars: [String: String] = [:]
        if !version.isEmpty { vars["version"] = version }
        if let checkedAtMs {
            vars["time"] = Formats.time(Date(timeIntervalSince1970: Double(checkedAtMs) / 1000))
        }
        return vars.isEmpty ? loc.t(key) : loc.t(key, vars: vars)
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
