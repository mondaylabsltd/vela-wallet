//
//  DbrWire.swift
//  VelaWallet
//
//  The `dapp_browser` machine's view model and its value types, in Swift
//  (spec 070).
//
//  ## What this machine owns, and what it leaves to the shell
//
//  Every decision a page's message leads to: which document it came from,
//  whether the frame may speak, what the answer is, which tab and which
//  document the answer goes to, the per-origin chain, the one signing sheet at
//  a time, the read bounds. The shell owns the web views, posts strings,
//  performs reads and draws — and reports the one fact only it can see, the
//  PLATFORM's origin of the frame that sent a message.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations stay
//  dictionaries, the house rule every wire file here repeats.
//  `BrowserWireDriftTests` decodes these from the real core.
//

import Foundation

/// One per-origin grant, one-for-one with the `vela.perm.<origin>` document
/// (`dapp_permissions::DpermGrant`, which this machine reuses verbatim).
struct DpermGrantWire: Codable, Equatable {
    let origin: String
    /// The address the site was shown. The core re-pins it when the wallet's
    /// active account changes (the extension's `followActiveAccount`).
    let address: String
    let chainId: Int
    let grantedAtMs: Double

    /// The stored shape: snake_case, exactly what the core serialises and
    /// what this app has written under `vela.perm.<origin>` since spec 053.
    var stored: [String: Any] {
        ["origin": origin, "address": address, "chain_id": chainId, "granted_at_ms": grantedAtMs]
    }

    /// A stored grant, or `nil` when the text is not one — a corrupt key is
    /// one site forgotten, never the whole list refused.
    static func read(_ text: String?) -> DpermGrantWire? {
        guard let text, let data = text.data(using: .utf8) else { return nil }
        return try? CoreJSON.decoder.decode(DpermGrantWire.self, from: data)
    }
}

/// The connection sheet's contents: a site is ASKING.
struct DbrConsentViewWire: Decodable, Equatable {
    /// The tab whose page asked first.
    let tab: String
    let origin: String
    /// One entry per coalesced request — the sheet names the origin once.
    let methods: [String]
    /// The account a grant would be made for (the active one).
    let address: String?
    /// The chain the grant would start on — the site's own.
    let chainId: Int
}

/// One open tab, as the core sees it.
struct DbrTabViewWire: Decodable, Equatable, Identifiable {
    let tab: String
    /// The origin of the tab's document, or of the URL it shows while no
    /// document has said hello yet.
    let origin: String?
    /// The address this origin may see. `nil` is the disconnected chip.
    let connectedAddress: String?
    /// The site's chain — per origin, persisted (`vela.chain.<origin>`).
    let chainId: Int
    /// https, or a loopback / private-network http host. The lock is drawn
    /// from this and from nothing else.
    let secure: Bool
    /// The renderer died. Cleared by the next document's hello.
    let crashed: Bool

    var id: String { tab }
}

/// One connected site — Settings' list.
struct DbrSiteViewWire: Decodable, Equatable, Identifiable {
    let origin: String
    let address: String
    let chainId: Int
    let grantedAtMs: Double

    var id: String { origin }
}

/// The request whose signing sheet should be up.
struct DbrSigningViewWire: Decodable, Equatable {
    let tab: String
    let id: String
}

struct DbrViewWire: Decodable, Equatable {
    /// The stored sites have been read.
    let ready: Bool
    let consent: DbrConsentViewWire?
    let tabs: [DbrTabViewWire]
    /// Every connected site, newest first.
    let sites: [DbrSiteViewWire]
    let signing: DbrSigningViewWire?
    let queuedSigning: Int

    static let empty = DbrViewWire(
        ready: false, consent: nil, tabs: [], sites: [], signing: nil, queuedSigning: 0
    )

    /// The core's facts about one tab, if it has any yet.
    func tab(_ id: String?) -> DbrTabViewWire? {
        guard let id else { return nil }
        return tabs.first { $0.tab == id }
    }
}

/// A request the core forwarded to the signing sheet (`forward_to_signing`).
struct DbrForward: Equatable {
    let tab: String
    let id: String
    let method: String
    let paramsJson: String
    let origin: String
    /// The SITE's chain — the request is signed on it, never on a global one.
    let chainId: Int
    /// The address the site was shown. The signer is pinned to it.
    let grantedAddress: String

    init(tab: String, id: String, method: String, paramsJson: String,
         origin: String, chainId: Int, grantedAddress: String) {
        self.tab = tab
        self.id = id
        self.method = method
        self.paramsJson = paramsJson
        self.origin = origin
        self.chainId = chainId
        self.grantedAddress = grantedAddress
    }

    init(operation: [String: Any]) {
        self.init(
            tab: operation["tab"] as? String ?? "",
            id: operation["id"] as? String ?? "",
            method: operation["method"] as? String ?? "",
            paramsJson: operation["params_json"] as? String ?? "[]",
            origin: operation["origin"] as? String ?? "",
            chainId: (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
            grantedAddress: operation["granted_address"] as? String ?? ""
        )
    }
}
