//
//  PayLink.swift
//  VelaWallet
//
//  A deep link's shape, tokenised only.
//
//  Ported from Android's `feature/wallet/core/PayLink.kt` (spec 047 D8). Three
//  forms carry a payment — `velawallet://pay?…`,
//  `https://wallet.getvela.app/pay?…` and `https://getvela.app/pay?…` — and one
//  names a page for the in-app browser: `velawallet://open?url=`.
//
//  **This file does not validate.** Whether a `/pay` query is a request this
//  wallet can honour is `payment_request`'s (`LinkOpened` → `pay_valid`), and
//  the shell handing over a verdict it made itself would be a second opinion
//  about somebody's money.
//

import Foundation

enum PayLink: Equatable {
    case pay(
        to: String?, chain: String?, token: String?,
        amount: String?, sym: String?, dec: String?, net: String?
    )
    case open(url: String)

    /// The event the core takes for a `/pay` link.
    var linkOpened: [String: Any]? {
        guard case .pay(let to, let chain, let token, let amount, let sym, let dec, let net) = self
        else { return nil }
        return [
            "type": "link_opened",
            "to": to.map { $0 as Any } ?? NSNull(),
            "chain": chain.map { $0 as Any } ?? NSNull(),
            "token": token.map { $0 as Any } ?? NSNull(),
            "amount": amount.map { $0 as Any } ?? NSNull(),
            "sym": sym.map { $0 as Any } ?? NSNull(),
            "dec": dec.map { $0 as Any } ?? NSNull(),
            "net": net.map { $0 as Any } ?? NSNull(),
        ]
    }

    static func parse(_ raw: String?) -> PayLink? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty,
              let components = URLComponents(string: raw),
              let scheme = components.scheme?.lowercased()
        else { return nil }
        let host = (components.host ?? "").lowercased()
        var path = components.path
        while path.hasSuffix("/") { path.removeLast() }
        let query = Dictionary(
            (components.queryItems ?? []).map { ($0.name, $0.value ?? "") },
            uniquingKeysWith: { first, _ in first }
        )

        switch (scheme, host) {
        case ("velawallet", "pay"):
            return pay(query)
        case ("velawallet", "open"):
            // Only a WEB page. A `velawallet://open?url=file:///…` would be a
            // link asking this app to open its own container, and an app that
            // honours it is one arbitrary link away from reading its own files
            // out to a page.
            guard let url = query["url"],
                  url.hasPrefix("http://") || url.hasPrefix("https://")
            else { return nil }
            return .open(url: url)
        case ("https", "wallet.getvela.app"), ("https", "getvela.app"):
            guard path == "/pay" else { return nil }
            return pay(query)
        default:
            return nil
        }
    }

    private static func pay(_ query: [String: String]) -> PayLink {
        .pay(
            to: query["to"], chain: query["chain"], token: query["token"],
            amount: query["amount"], sym: query["sym"], dec: query["dec"], net: query["net"]
        )
    }
}
