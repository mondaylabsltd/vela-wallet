//
//  NetworkSpaceTests.swift
//  VelaWalletTests
//
//  The mainnet space and the testnet space (spec 105): one device-wide
//  switch, and every list a person sees is the space's alone.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct NetworkSpaceTests {

    private func store() -> VelaStore {
        let suite = "vela.tests.space.\(UUID().uuidString)"
        return VelaStore(defaults: UserDefaults(suiteName: suite)!)
    }

    @Test func aDeviceStartsInTheMainnetSpace() {
        let shelf = store()
        #expect(NetworkSpace.read(shelf) == .mainnet)
        shelf.writeString(VelaStore.Key.networkSpace, "nonsense")
        #expect(NetworkSpace.read(shelf) == .mainnet)
        let space = NetworkSpace(store: shelf)
        space.set(.testnet)
        #expect(space.testnet)
        #expect(NetworkSpace.read(shelf) == .testnet, "one value, for the device")
        #expect(NetworkSpace(store: shelf).current == .testnet, "and it holds across a launch")
    }

    @Test func theCatalogueSaysWhatTheCoreSays() {
        let testnets = ChainCatalog.chains.filter(\.testnet).map(\.chainId)
        #expect(testnets == [11_155_111, 84_532, 421_614, 11_155_420, 80_002, 97, 43_113, 5_042_002, 42_431])
        #expect(ChainCatalog.chains.filter { !$0.testnet }.count == 24)
    }

    @Test func eachSpaceReadsItsOwnChains() {
        let shelf = store()
        shelf.writeList(VelaStore.Key.customNetworks, [
            ["chainId": 4242, "displayName": "Mine", "isTestnet": false],
            ["chainId": 31337, "displayName": "Anvil"],
            ["chainId": 5_042_002, "displayName": "Arc Testnet"],
        ])
        let mainnet = NetworkSpace.chainIds(store: shelf)
        #expect(mainnet.contains(1))
        #expect(mainnet.contains(4242), "the person said mainnet")
        #expect(!mainnet.contains(11_155_111))
        #expect(!mainnet.contains(31337), "nobody vouched for it: a testnet")

        shelf.writeString(VelaStore.Key.networkSpace, "testnet")
        let testnet = NetworkSpace.chainIds(store: shelf)
        #expect(testnet.contains(11_155_111))
        #expect(testnet.contains(31337))
        #expect(!testnet.contains(1))
        #expect(!testnet.contains(4242))
        #expect(testnet.filter { $0 == 5_042_002 }.count == 1, "added and now built in: once")
    }
}
