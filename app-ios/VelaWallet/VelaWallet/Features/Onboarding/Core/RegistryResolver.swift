//
//  RegistryResolver.swift
//  VelaWallet
//
//  The three layers behind a sign-in read (064): the index, the registry
//  contract on Gnosis, and its backup on Ethereum.
//
//  Every rule — which layer is asked, when the index's answer is PROVED against
//  the contract's content hash, when it is thrown away, whose unit ids a listing
//  speaks — is `vela_core::registry_resolve`. What is left here is what only a
//  shell can do: perform an `eth_call`. `RegistryClient` performs the index GET
//  and drives the walk.
//

import Foundation
import VelaCore

struct RegistryResolver: Sendable {
    /// The RAW `result` hex, or `nil` when that chain did not answer.
    let ethCall: @Sendable (_ chainId: Int, _ to: String, _ data: String) async -> String?

    /// The core's walks. Replaceable only so a test can script them.
    var keyStep: @Sendable (_ publicKeyHex: String, _ answersJson: String) -> String = {
        registryResolveKeyStep(publicKeyHex: $0, answersJson: $1)
    }
    var unitStep: @Sendable (_ unitId: UInt32, _ source: String, _ answersJson: String) -> String = {
        registryResolveUnitStep(unitId: $0, source: $1, answersJson: $2)
    }
}
