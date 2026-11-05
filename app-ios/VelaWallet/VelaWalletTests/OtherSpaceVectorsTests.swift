//
//  OtherSpaceVectorsTests.swift
//  VelaWalletTests
//
//  r13-1 (docs/multisig-machines.md §15.4, §17): where a multisig lives when
//  none of its networks is in the space in front is the core's rule
//  (`network_space::lives_elsewhere`, uniffi `network_lives_elsewhere`). The
//  shared vector (`rust/crates/vela-core/tests/vectors/multisig/other-space.json`)
//  replayed through the one call the home makes (`WalletLive.livesElsewhere`);
//  the shell keeps no copy of the rule.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct OtherSpaceVectorsTests {

    private static let vectorURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/multisig/other-space.json")

    @Test func theOtherSpaceVectorsHoldThroughTheShellsCall() throws {
        let data = try Data(contentsOf: Self.vectorURL)
        let doc = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let cases = try #require(doc["cases"] as? [[String: Any]])
        #expect(cases.count >= 6)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let front = try #require(NetworkSpaceKind(rawValue: c["front"] as? String ?? ""), "\(name)")
            let got = WalletLive.livesElsewhere(
                chainIds: c["chain_ids"] as? [Int] ?? [],
                here: c["here"] as? [Int] ?? [],
                other: c["other"] as? [Int] ?? [],
                front: front
            )
            guard let want = c["expect"] as? [String: Any] else {
                #expect(got == nil, "\(name)")
                continue
            }
            let answer = try #require(got, "\(name): the core said nothing")
            #expect(answer.chainIds == want["chain_ids"] as? [Int], "\(name)")
            #expect(answer.target.rawValue == want["space"] as? String, "\(name)")
            #expect(answer.lineKey == want["line_key"] as? String, "\(name)")
            #expect(answer.headingKey == want["heading_key"] as? String, "\(name)")
            #expect(answer.switchKey == want["switch_key"] as? String, "\(name)")
        }
    }
}
