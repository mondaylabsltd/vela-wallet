//
//  AppLink.swift
//  VelaWallet
//
//  Every link the app is opened with, read (native parity N-I3).
//
//  The installed app claims the links the wallet makes (owner decision
//  2026-10-30 #2) — a payment (`/pay?…`), a multisig proposal
//  (`/<locale>/wallet#ms=…`), an invite (`#msa=…`) and the connect page
//  (`/<locale>/connect#…`) — through the AASA in app-ios/universal-links/,
//  and the doors from outside (`velawallet://send` …, `VelaEntry`).
//
//  **The shell reads a link's SHAPE; the core says what it holds.** A
//  multisig link is read by `vela_core::multisig_link` over uniffi — the
//  same rules, character for character, the web reads it with
//  (`multisig/core/link.ts`) — and a payment by `payment_request` once it is
//  handed in (`PayLink`). A fragment this file split by itself would be a
//  second reading of a link that names somebody's multisig.
//
//  A link that is meant as one of these and does not read is SAID
//  (`.broken`), never dropped: the web's ms-22. A link that is not one of
//  these is ignored in silence — an app that answered every URL could be
//  made to say things by anybody with one.
//

import Foundation
import VelaCore

/// The transaction a proposal link names — `ProposalLink` in the core.
struct MultisigProposalRef: Decodable, Equatable {
    let chainId: Int
    let multisig: String
    let safeTxHash: String
}

/// One member as an invite names it — labels only; who may act is the
/// chain's to say.
struct InvitedMemberRef: Decodable, Equatable {
    var address: String?
    var name: String?
    /// `passkey` / `wallet` / `contract`, or absent.
    var kind: String?
}

/// An invite — `AddLink` in the core.
struct MultisigInviteRef: Decodable, Equatable {
    let multisig: String
    let chainIds: [Int]
    let members: [InvitedMemberRef]?
}

/// The connect page's fragment — `ConnectHash` in the core.
struct ConnectRef: Decodable, Equatable {
    let invite: String?
    let members: [String]
    let tx: MultisigProposalRef?
    let damaged: Bool
}

enum AppLink: Equatable {
    /// A door from outside: Send, Receive, Scan.
    case entry(VelaEntry)
    /// A payment link or `velawallet://open` — `PayLink`'s, as before.
    case pay(PayLink)
    /// A multisig transaction to approve.
    case proposal(MultisigProposalRef, url: URL)
    /// A multisig to add.
    case invite(MultisigInviteRef, url: URL)
    /// The page an outside owner approves on, with their own wallet.
    case connect(ConnectRef, url: URL)
    /// Meant as one of the multisig links, and it does not read.
    case broken(BrokenLink, url: URL)

    enum BrokenLink: Equatable {
        /// A proposal link cut short or damaged.
        case proposal
        /// An invite whose part is missing or wrong.
        case invite
        /// An invite whose address's letters were changed on the way
        /// (EIP-55, r5reg-22).
        case inviteChecksum
        /// A connect page link whose room does not read.
        case connect
    }

    /// The hosts the wallet's links live on: the web wallet's own, and the
    /// site's (whose `/pay` forwards to it).
    static let hosts: Set<String> = ["wallet.getvela.app", "getvela.app"]

    /// What `url` is, or `nil` for a link that is none of these.
    ///
    /// `call` is the core's `multisig_link` surface — injectable, so a test
    /// can count that every reading went through it.
    static func read(
        _ url: URL,
        call: (String, String) throws -> String = { try multisigLinkCall(method: $0, argsJson: $1) }
    ) -> AppLink? {
        if let entry = VelaEntry(url: url) { return .entry(entry) }
        if let pay = PayLink.parse(url.absoluteString) { return .pay(pay) }
        guard url.scheme?.lowercased() == "https", let host = url.host?.lowercased(), hosts.contains(host)
        else { return nil }
        let place = Place(path: url.path)
        // The fragment exactly as it rode: `URL.fragment` would decode it,
        // and the core's rules are the web's, which read `location.hash`.
        let hash = Self.rawFragment(url).map { "#\($0)" } ?? ""

        if place == .connect {
            guard let read: ConnectRef = decode(try? call("readConnectHash", args(["hash": hash]))) else { return nil }
            return read.damaged ? .broken(.connect, url: url) : .connect(read, url: url)
        }
        guard place == .wallet || place == .localeRoot, !hash.isEmpty else { return nil }
        let meant: Bool? = decode(try? call("isMultisigLink", args(["hash": hash])))
        guard meant == true else { return nil }
        if let proposal: MultisigProposalRef = decode(try? call("parseProposalLink", args(["hash": hash]))) {
            return .proposal(proposal, url: url)
        }
        if let invite: MultisigInviteRef = decode(try? call("parseAddLink", args(["hash": hash]))) {
            return .invite(invite, url: url)
        }
        // Meant as one, and it does not read: which one, and why.
        let kind: String? = decode(try? call("brokenLink", args(["hash": hash])))
        if kind == "add" {
            let fault: String? = decode(try? call("addLinkFault", args(["hash": hash])))
            return .broken(fault == "checksum" ? .inviteChecksum : .invite, url: url)
        }
        return .broken(.proposal, url: url)
    }

    /// Where on the web wallet a link points.
    enum Place: Equatable {
        /// `/<locale>/wallet` — proposals and invites.
        case wallet
        /// `/<locale>` — the site's front page, where an invite opened
        /// before onboarding lands (`/en#msa=…`, r4b-17).
        case localeRoot
        /// `/<locale>/connect` — the outside owner's page.
        case connect
        case elsewhere

        init(path: String) {
            var parts = path.split(separator: "/", omittingEmptySubsequences: true).map(String.init)
            guard let first = parts.first, Self.isLocale(first) else {
                self = .elsewhere
                return
            }
            parts.removeFirst()
            switch parts {
            case []: self = .localeRoot
            case ["wallet"]: self = .wallet
            case ["connect"]: self = .connect
            default: self = .elsewhere
            }
        }

        /// A locale segment as the web routes them: `en`, `zh-TW`, `es-MX`.
        static func isLocale(_ segment: String) -> Bool {
            let pieces = segment.split(separator: "-", omittingEmptySubsequences: false)
            guard let language = pieces.first, language.count == 2, language.allSatisfy(\.isLetter),
                  pieces.count <= 2
            else { return false }
            if pieces.count == 2 {
                let region = pieces[1]
                return region.count == 2 && region.allSatisfy(\.isLetter)
            }
            return true
        }
    }

    /// The fragment as written in the link, still percent-encoded.
    static func rawFragment(_ url: URL) -> String? {
        URLComponents(url: url, resolvingAgainstBaseURL: false)?.percentEncodedFragment
    }

    private static func args(_ values: [String: String]) -> String {
        (try? JSONSerialization.data(withJSONObject: values)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    /// The core's answer, decoded — `nil` for its `null`, or for no answer.
    private static func decode<T: Decodable>(_ json: String?) -> T? {
        guard let data = json?.data(using: .utf8) else { return nil }
        return try? CoreJSON.decoder.decode(T.self, from: data)
    }
}
