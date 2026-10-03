//
//  RelayRpcHeaderTests.swift — spec 098 §5.
//
//  Every request to the relay names the RPC this wallet uses for the chain,
//  under the name the relay reads (`x-vela-rpc-url`). Before 081 the app sent
//  `X-Rpc-Url`, which the relay never read; 081 removed it as inert, and a
//  network the relay's directory could not reach was then never served — the
//  send went on without a word. To the relay only: the core sets `x_rpc_url`
//  on bundler calls alone, so an RPC provider is never sent one.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct RelayRpcHeaderTests {
    @Test func aBundlerPostNamesTheChainsRpcUnderTheRelaysName() {
        let headers = RpcPool.postHeaders([
            "type": "json_rpc_post", "call_id": "c1", "url": "https://relay.example",
            "method": "eth_sendUserOperation", "x_rpc_url": "https://rpc.one/v2/KEY",
        ])
        #expect(headers == ["x-vela-rpc-url": "https://rpc.one/v2/KEY"])
        #expect(headers["X-Rpc-Url"] == nil)
    }

    @Test func aPlainRpcPostNamesNone() {
        let headers = RpcPool.postHeaders([
            "type": "json_rpc_post", "call_id": "c1", "url": "https://rpc.two",
            "method": "eth_call", "x_rpc_url": NSNull(),
        ])
        #expect(headers.isEmpty)
    }

    @Test func theRestDoorUsesTheSameName() {
        #expect(CoreHTTP.relayRpcHeaders("https://rpc.one") == ["x-vela-rpc-url": "https://rpc.one"])
        #expect(CoreHTTP.relayRpcHeaders(nil).isEmpty)
        #expect(CoreHTTP.relayRpcHeaders("").isEmpty)
    }
}
