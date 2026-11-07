//
//  CoreServiceDefaults.swift
//  VelaWallet
//
//  Every production service default — the core's (`service_defaults`, §28).
//  This app keeps no copy of its own: the key index, the multisig service,
//  the relay, the fiat rates, the bug report and PeerConnect's addresses are
//  read here, from the one spelling every shell reads.
//

import Foundation
import VelaCore

nonisolated enum CoreServiceDefaults {
    static let value: ServiceDefaults = serviceDefaults()
}
