//
//  VelaLog.swift
//  VelaWallet
//
//  One place the app says what happened, in the system log (spec 082 RE11,
//  FR-018, FR-019).
//
//  Until 082 this client only had `print`, and `print` reaches nothing but a
//  `devicectl --console` session — which cancels Face ID sheets (050/052), so
//  every signing row of a device pass ran with no trace at all. `os.Logger`
//  lines survive into `log collect`, a sysdiagnose and Console.app, with no
//  attached debugger.
//
//  ## What a line may carry
//
//  Kinds, classes, chain ids, an `NSError`'s domain and code, short hashes
//  (10 hex digits), durations. Never an address, a full URL, calldata, a
//  signature or a credential. A dApp's host is public in a DEBUG build (the
//  device pass needs to read it) and an FNV token in Release — a host in the
//  system log of a shipped phone is browsing history.
//
//  Every line goes through `format` before it is logged, and `format` scrubs
//  what a caller might have let through anyway: an address becomes
//  `[address]`, a longer hex run is cut to its first ten digits, a URL is
//  `[url]`. The rules above are the contract; the scrub is the net.
//
//  ## The report's "Recent failures" line
//
//  The last eight failures, as `scope: kind` and nothing else, feed the bug
//  report's existing line (`componentsUi.bugReport.previewFailures`). No host
//  and no hash ever enters the ring: it is read into a report a person sends.
//

import Foundation
import os

nonisolated enum VelaLog {

    /// The subsystem Console.app and `log collect --predicate` filter on.
    static let subsystem = "app.getvela.VelaWallet"

    /// One category per area a failure row names (contract §15).
    enum Scope: String, CaseIterable {
        case browser, sign, relay, rpc, fee, tracker, balance, net
    }

    /// How many failures the report's line keeps.
    static let ringCap = 8

    // MARK: - Writing

    /// An event: something happened that a device pass reads back.
    static func notice(_ scope: Scope, _ message: String) {
        let line = format(message)
        logger(scope).notice("\(line, privacy: .public)")
    }

    /// A failure a person may have seen. Logged at `.error`, and its
    /// `scope: kind` enters the report's ring. `kind` is a class name
    /// (`timeout`, `offline`, `maybe_sent`…), never a sentence with data in it.
    static func failure(_ scope: Scope, kind: String, _ detail: String = "") {
        let line = format(detail.isEmpty ? kind : "\(kind) \(detail)")
        logger(scope).error("\(line, privacy: .public)")
        remember("\(scope.rawValue): \(kind)")
    }

    // MARK: - The fee's two lines (spec 082 RJ12, G47)

    /// `fee: quote failed chain=… cause=… re-quote #n in N ms` — the words
    /// every client logs when a quote fails. `cause` is the failure's wire
    /// name (`FeeFailureText.cause`) or `timeout`; `inMs` `nil` means no
    /// re-quote follows (the failure is not the network's, or the sheet can
    /// no longer use a fee). The report's ring gets `fee: quote_failed`.
    static func feeQuoteFailed(chain: Int, cause: String, requote: UInt32, inMs: UInt32?) {
        let next = inMs.map { "re-quote #\(requote) in \($0) ms" } ?? "no re-quote"
        let line = format("quote failed chain=\(chain) cause=\(cause) \(next)")
        logger(.fee).error("\(line, privacy: .public)")
        remember("fee: quote_failed")
    }

    /// `fee: quote back chain=… after n re-quotes` — the fee is on screen again.
    static func feeQuoteBack(chain: Int, after requotes: UInt32) {
        notice(.fee, "quote back chain=\(chain) after \(requotes) re-quotes")
    }

    // MARK: - Fields

    /// A host as a line may carry it: itself in DEBUG, an FNV-1a token in
    /// Release. Lower-cased first, so one site is one token.
    static func host(_ host: String) -> String {
        let clean = host.lowercased()
        guard !clean.isEmpty else { return "-" }
        return isRelease ? "h-" + fnv1a(clean) : clean
    }

    /// The host of a URL string, as `host(_:)` writes it; `-` when there is
    /// none. The path, the query and the fragment never reach a line.
    static func hostOf(url: String?) -> String {
        guard let url, let parsed = URL(string: url), let name = parsed.host else { return "-" }
        let port = parsed.port.map { ":\($0)" } ?? ""
        return host(name + port)
    }

    /// The first ten hex digits of a hash — enough to find it in the relay's
    /// log, too few to be anything else.
    static func short(_ hash: String?) -> String {
        guard let hash, !hash.isEmpty else { return "-" }
        let body = hash.hasPrefix("0x") || hash.hasPrefix("0X") ? String(hash.dropFirst(2)) : hash
        return "0x" + String(body.prefix(10))
    }

    /// `domain code` of an error — what the platform says, not its sentence.
    static func error(_ error: Error) -> String {
        let ns = error as NSError
        return "\(ns.domain) \(ns.code)"
    }

    /// Milliseconds since `start`, rounded.
    static func ms(since start: Date) -> String {
        "\(Int(Date().timeIntervalSince(start) * 1000))ms"
    }

    // MARK: - The report's line

    /// The last failures, oldest first, as `scope: kind`.
    static var recentFailures: [String] {
        lock.lock()
        defer { lock.unlock() }
        return ring
    }

    /// Test seam: forget the ring.
    static func resetRecentFailures() {
        lock.lock()
        ring.removeAll()
        lock.unlock()
    }

    // MARK: - The scrub

    /// What a line looks like after the net: no address, no long hex, no URL.
    static func format(_ message: String) -> String {
        message
            // A URL first: its path may carry an address or a key.
            .replacingOccurrences(
                of: "\\b[a-zA-Z][a-zA-Z0-9+.-]*://\\S+", with: "[url]", options: .regularExpression
            )
            // Longer than an address (a hash, calldata, a signature): its
            // first ten digits.
            .replacingOccurrences(
                of: "0x([0-9a-fA-F]{10})[0-9a-fA-F]{31,}", with: "0x$1…", options: .regularExpression
            )
            // Exactly an address.
            .replacingOccurrences(
                of: "0x[0-9a-fA-F]{40}(?![0-9a-fA-F])", with: "[address]", options: .regularExpression
            )
    }

    // MARK: - Release seam

    /// Test seam: `true` makes this build log like a Release one.
    nonisolated(unsafe) static var releaseOverride: Bool?

    static var isRelease: Bool {
        if let releaseOverride { return releaseOverride }
        #if DEBUG
        return false
        #else
        return true
        #endif
    }

    /// FNV-1a, 32-bit, as hex — the same hash the bug report fingerprints
    /// with. Not a security primitive: it keeps a host out of plain sight in
    /// a shipped phone's log and lets two lines about one site be matched.
    static func fnv1a(_ text: String) -> String {
        var hash: UInt32 = 0x811c9dc5
        for byte in text.utf8 {
            hash ^= UInt32(byte)
            hash = hash &* 0x01000193
        }
        return String(format: "%08x", hash)
    }

    // MARK: - Private

    private static let lock = NSLock()
    nonisolated(unsafe) private static var ring: [String] = []

    private static func remember(_ entry: String) {
        lock.lock()
        ring.append(entry)
        if ring.count > ringCap { ring.removeFirst(ring.count - ringCap) }
        lock.unlock()
    }

    private static let loggers: [Scope: Logger] = Dictionary(
        uniqueKeysWithValues: Scope.allCases.map { ($0, Logger(subsystem: subsystem, category: $0.rawValue)) }
    )

    private static func logger(_ scope: Scope) -> Logger {
        loggers[scope] ?? Logger(subsystem: subsystem, category: scope.rawValue)
    }
}
