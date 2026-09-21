//
//  ExploreScan.swift
//  VelaWallet
//
//  What the Explore start page's scan button does with a code (issue 273,
//  decision D1 option b, adopted by spec 070 US5).
//
//  Four outcomes, and each one is said honestly:
//
//  - a **web address** opens in this browser;
//  - an **account address or `ethereum:` payment code** goes to 发送, through
//    the core's `scan_resolved` — the same door the home scanner uses;
//  - a **WalletConnect** pairing code is named as unsupported, with the way
//    that does work (open the dApp here and connect from its page);
//  - anything else is "not a web address or a wallet address".
//
//  Before 070 everything but a web address was one generic refusal, and
//  before 273 a pairing code was "opened" as a page called `https://wc`.
//

import Foundation
import VelaCore

enum ExploreScan {

    enum Route: Equatable {
        case open(String)
        case send(String)
        case walletConnect
        case unrecognized
    }

    static func route(_ payload: String) -> Route {
        let text = payload.trimmingCharacters(in: .whitespacesAndNewlines)
        if let url = url(from: text) { return .open(url) }
        if Eip681.parse(text) != nil || Eip681.isHexAddress(text) { return .send(text) }
        if text.lowercased().hasPrefix("wc:") { return .walletConnect }
        return .unrecognized
    }

    /// The address a scanned payload opens, in the form the browser's open path
    /// takes — or `nil` when the code is not a web address.
    ///
    /// Accepted: an `http(s)://` URL with a host, and a bare domain
    /// (`app.uniswap.org/swap`), which the search field would also open. Refused:
    /// every other scheme (`wc:`, `ethereum:`), a bare account address, and
    /// anything with whitespace in it — a scanned code is never a search.
    static func url(from payload: String) -> String? {
        let text = payload.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.rangeOfCharacter(from: .whitespacesAndNewlines) == nil
        else { return nil }

        let lower = text.lowercased()
        for scheme in ["https://", "http://"] where lower.hasPrefix(scheme) {
            let url = scheme + text.dropFirst(scheme.count)
            guard let host = URLComponents(string: url)?.host, !host.isEmpty else { return nil }
            return url
        }

        // A bare domain. Read as the search field would read it, and kept only
        // when what comes out is a real host name, so `ethereum:0x…` (host
        // `ethereum`) and `wc:…@2` (user `wc`) fall out.
        guard let parts = URLComponents(string: "https://" + text),
              parts.user == nil, parts.password == nil,
              let host = parts.host, isDomain(host)
        else { return nil }
        return dappBrowserInput(text: text)
    }

    /// Dot-separated labels of letters, digits and hyphens, ending in a
    /// letters-only top-level label.
    private static func isDomain(_ host: String) -> Bool {
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        guard labels.count >= 2, let tld = labels.last, tld.count >= 2,
              tld.allSatisfy({ $0.isASCII && $0.isLetter })
        else { return false }
        return labels.allSatisfy { label in
            !label.isEmpty && label.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-") }
        }
    }
}
