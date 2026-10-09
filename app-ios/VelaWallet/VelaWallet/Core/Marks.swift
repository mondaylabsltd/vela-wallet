//
//  Marks.swift
//  VelaWallet
//
//  Which picture a token or a network wears — asked of the core, which holds
//  the one rule every shell draws by (`vela_core::app::remote_mark`;
//  `MarksTests` replays its vectors, `rust/crates/vela-core/tests/vectors/
//  marks.json`). This file decides nothing: it keeps the person's chain-data
//  endpoint as Settings stores it and hands it over with each question.
//
//  It used to be the fifth hand-written copy of the rule, and the copies had
//  begun to disagree (an empty contract read as the native coin here and as a
//  token everywhere else; a trailing slash; chain 0).
//
//  What the core decides:
//
//  1. Logos come from that endpoint — the built-in one when the field is
//     empty — with the drawn three-letter glyph as the whole fallback.
//  2. A native coin wears its OWN home chain's logo: ETH on Base is still
//     Ethereum's.
//  3. The badge is left off where it would repeat the coin: ETH on Ethereum,
//     XDAI on Gnosis, BNB on BNB Chain.
//  4. A token's logo is its asset entry, checksummed then lowercase, after any
//     URL an index already named. Chain 0 asks for nothing.
//
//  **The kind rule.** Anything that names a NETWORK (a network row or fact, a
//  notice that locks a chain, a receive row, the QR centre, a chip) wears
//  `chain` / `chainLogoURL`; anything that names a COIN wears `token`. The
//  network row of ETH sent on Base wears Base's logo, never Ethereum's.
//

import Foundation
import VelaCore

enum Marks {

    /// The person's ethereum-data endpoint as stored (Settings → 服务节点).
    /// Empty is not "no logos": the core reads it as the built-in endpoint.
    /// Set by the app at launch and again whenever Settings writes the
    /// endpoints — never by a test, so a test's stand-in endpoint cannot
    /// reach another test's marks.
    nonisolated(unsafe) static var base: String = ""

    /// Adopt the stored endpoints blob (`vela.serviceEndpoints`, camelCase).
    static func adopt(_ endpoints: [String: Any]) {
        base = endpoints["ethereumDataURL"] as? String ?? ""
    }

    // Each question takes `endpoint` for the tests that replay the core's
    // vectors through this module; the app leaves it out and asks with the
    // adopted `base`.

    /// `{endpoint}/chainlogos/eip155-{id}.png` — a network's own logo; `nil`
    /// on chain 0, which names no network.
    static func chainLogoURL(_ chainId: Int, endpoint: String? = nil) -> String? {
        chainLogoUrl(ethereumDataUrl: endpoint ?? base, chainId: wire(chainId))
    }

    /// A coin's mark. `tokenAddress` is the contract — `nil` for the chain's
    /// own coin, never "" (an empty contract is one the rule cannot place, so
    /// it gets no derived logo). `named` are logo URLs an index already gave
    /// it, tried first.
    static func token(
        chainId: Int,
        symbol: String,
        tokenAddress: String?,
        named: [String] = [],
        endpoint: String? = nil
    ) -> MarkView {
        tokenMark(ethereumDataUrl: endpoint ?? base, chainId: wire(chainId), symbol: symbol,
                  tokenAddress: tokenAddress, named: named)
    }

    /// A network drawn as itself: its own logo, its coin's letters under it,
    /// never a badge.
    static func chain(chainId: Int, nativeSymbol: String, endpoint: String? = nil) -> MarkView {
        chainMark(ethereumDataUrl: endpoint ?? base, chainId: wire(chainId),
                  nativeSymbol: nativeSymbol)
    }

    /// The core's chain ids are `u32`. One that is not (negative, or past
    /// `u32`) is chain 0, on which the core builds no URL — a glyph rather
    /// than a guess.
    private static func wire(_ chainId: Int) -> UInt32 {
        UInt32(exactly: chainId) ?? 0
    }
}
