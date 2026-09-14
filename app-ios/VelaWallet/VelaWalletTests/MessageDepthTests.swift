//
//  MessageDepthTests.swift
//  VelaWalletTests
//
//  A message signed for somebody else's site, driven through the REAL
//  `clear_signing` core.
//
//  This is the one attack EIP-4361 exists to prevent — a page asking somebody
//  to sign another page's login — and the check only works if the sentence on
//  screen names the two domains. Android 044 shipped a placeholder that never
//  interpolated, which is a warning that warns about nothing; these assert on
//  the RENDERED sentence rather than on the key.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MessageDepthTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    private func siweMessage(domain: String, chainId: Int) -> String {
        """
        \(domain) wants you to sign in with your Ethereum account:
        \(me)

        Sign in to continue.

        URI: https://\(domain)/login
        Version: 1
        Chain ID: \(chainId)
        Nonce: abc123xyz
        Issued At: 2026-09-14T12:00:00Z
        """
    }

    /// Hand the core a `personal_sign` as the browser would, and read back the
    /// message view it computed.
    private func presented(message: String, origin: String?) throws -> ClearSigningViewWire {
        let core = ClearSigningCore()
        let hex = "0x" + message.utf8.map { String(format: "%02x", $0) }.joined()
        let result = try core.dispatch(eventJson: CoreJSON.string([
            "type": "message_presented",
            "method": "personal_sign",
            "params": [hex, me],
            "request_origin": origin.map { $0 as Any } ?? NSNull(),
        ]))
        return try CoreJSON.decode(ClearSigningViewWire.self, from: try view(from: result))
    }

    private func blocks(_ clear: ClearSigningViewWire, origin: String?) -> [SigningBlock] {
        SigningLive.blocks(
            clear: clear, to: nil, valueHex: nil, dataBytes: 0,
            context: SigningLive.Context(
                loc: loc, chainName: "Gnosis", chainDot: .gray, nativeSymbol: "xDAI",
                walletName: "me", walletAddress: me, origin: origin
            )
        )
    }

    private func text(of block: SigningBlock) -> String {
        switch block {
        case .intent(let text, _): text
        case .sentence(let text, _): text
        case .warning(_, let text): text
        case .positive(let text): text
        default: ""
        }
    }

    // MARK: - The binding

    /// The site that asked and the site the message names are the same: a calm
    /// sign-in, with a VERIFIED line naming the domain that was adjudicated.
    @Test func aMatchingSignInIsCalmAndNamesItsDomain() throws {
        let clear = try presented(
            message: siweMessage(domain: "app.example", chainId: 100),
            origin: "https://app.example"
        )
        let message = try #require(clear.message)
        #expect(message.siwe?.domain == "app.example")
        #expect(message.binding == .ok)
        #expect(message.dangerClass == .siweOk)

        let rendered = blocks(clear, origin: "https://app.example").map(text(of:))
        let verified = loc.t("componentsUi.signing.siweOk", vars: ["domain": "app.example"])
        #expect(rendered.contains(verified), "the verified line is missing: \(rendered)")
        #expect(!verified.contains("{{"), "the sentence did not interpolate")
    }

    /// **The attack.** A page at `evil.example` asks somebody to sign a login
    /// for `app.example`. The sheet must name BOTH, in the core's words, in
    /// the danger tone.
    @Test func aMessageForAnotherSiteIsCalledPhishing() throws {
        let clear = try presented(
            message: siweMessage(domain: "app.example", chainId: 100),
            origin: "https://evil.example"
        )
        let message = try #require(clear.message)
        #expect(message.binding == .mismatch)
        #expect(message.dangerClass == .siwePhish)

        let built = blocks(clear, origin: "https://evil.example")
        let warning = built.first {
            if case .warning(let tone, _) = $0 { return tone == .danger }
            return false
        }
        let rendered = try #require(warning.map(text(of:)), "no danger warning at all")
        // BOTH names, and no leftover placeholder — the whole point of the
        // sentence is that a person can see the two are different.
        #expect(rendered.contains("app.example"))
        #expect(rendered.contains("evil.example"))
        #expect(!rendered.contains("{{"), "the phishing sentence did not interpolate")
        // The eyebrow escalates too, so the danger is visible before anybody
        // reads a paragraph.
        if case .intent(_, let tone)? = built.first {
            #expect(tone == .danger)
        } else {
            Issue.record("the sheet does not open with an intent")
        }
    }

    /// An origin nobody can parse is NOT evidence of phishing. It asserts no
    /// match either — the calm layout without the verified badge.
    @Test func anUnknownOriginNeitherVerifiesNorAccuses() throws {
        let clear = try presented(
            message: siweMessage(domain: "app.example", chainId: 100), origin: nil
        )
        let message = try #require(clear.message)
        #expect(message.binding == .unknown)
        #expect(message.dangerClass == .siweOk, "an unknown origin is not an accusation")

        let rendered = blocks(clear, origin: nil).map(text(of:))
        #expect(!rendered.contains { $0.contains("钓鱼") }, "an unknown origin was accused")
        let verified = loc.t("componentsUi.signing.siweOk", vars: ["domain": "app.example"])
        #expect(!rendered.contains(verified), "an unknown origin was verified")
    }

    // MARK: - The rows that were being dropped

    /// The chain and the nonce the message names are parsed by the core and
    /// were rendered nowhere. A sign-in for chain 1 presented on chain 100
    /// looked identical to one that matched.
    @Test func theChainAndNonceTheMessageNamesAreShown() throws {
        let clear = try presented(
            message: siweMessage(domain: "app.example", chainId: 1),
            origin: "https://app.example"
        )
        #expect(clear.message?.siwe?.chainId == 1)
        #expect(clear.message?.siwe?.nonce == "abc123xyz")

        let rows = blocks(clear, origin: "https://app.example").compactMap { block -> [SigningRow]? in
            if case .rows(let rows) = block { return rows }
            return nil
        }.flatMap { $0 }
        let chain = rows.first { $0.label == loc.t("componentsUi.signing.labelChain") }
        #expect(chain?.value == "Ethereum", "the message's chain is not on screen")
        let nonce = rows.first { $0.label == loc.t("componentsUi.signing.labelNonce") }
        #expect(nonce?.value == "abc123xyz")
    }

    // MARK: - eth_sign

    /// `eth_sign` signs an opaque hash and gets the hard warning, never the
    /// calm message layout — wherever it is reachable at all.
    @Test func ethSignKeepsItsDangerFace() throws {
        let core = ClearSigningCore()
        let result = try core.dispatch(eventJson: CoreJSON.string([
            "type": "message_presented",
            "method": "eth_sign",
            "params": [me, "0x" + String(repeating: "ab", count: 32)],
            "request_origin": "https://app.example",
        ]))
        let clear = try CoreJSON.decode(ClearSigningViewWire.self, from: try view(from: result))
        #expect(clear.message?.dangerClass == .ethSign)

        let rendered = blocks(clear, origin: "https://app.example")
        let danger = rendered.filter {
            if case .warning(let tone, _) = $0 { return tone == .danger }
            if case .sentence(_, let tone) = $0 { return tone == .danger }
            return false
        }
        #expect(danger.count >= 2, "eth_sign lost its warnings")
        #expect(danger.map(text(of:)).contains(loc.t("componentsUi.signing.ethSignWarning")))
    }
}
