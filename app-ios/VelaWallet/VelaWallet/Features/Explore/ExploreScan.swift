//
//  ExploreScan.swift
//  VelaWallet
//
//  What the Explore start page's scan button opens (issue 273).
//
//  The founder's ruling: the button reads a **web address** and opens it in
//  this browser. Nothing else — a WalletConnect pairing code is not something
//  this wallet speaks, and a payment code belongs to 发送. Both are refused in
//  one line rather than "opened" as a page called `https://wc` or `https://0x…`,
//  which is what handing them to the search field's own coercion would do.
//

import Foundation

enum ExploreScan {

    /// The address a scanned payload opens, in the form the browser's open path
    /// takes — or `nil` when the code is not a web address.
    ///
    /// Accepted: an `http(s)://` URL with a host, and a bare domain
    /// (`app.uniswap.org/swap`), which the search field would also open. Refused:
    /// every other scheme (`wc:`, `ethereum:`), a bare account address, and
    /// anything with whitespace in it.
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

        // A bare domain. Read as the search field would read it — `https://` in
        // front — and kept only when what comes out is a real host name, so
        // `ethereum:0x…` (host `ethereum`) and `wc:…@2` (user `wc`) fall out.
        guard let parts = URLComponents(string: "https://" + text),
              parts.user == nil, parts.password == nil,
              let host = parts.host, isDomain(host)
        else { return nil }
        return BrowserEngine.coerce(text)
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
