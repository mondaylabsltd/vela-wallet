//
//  AgentTokenVault.swift
//  VelaWallet
//
//  Where the AI agent's client token is kept: the Keychain (spec 104), not
//  with the other `vela.*` values in `UserDefaults`, which a device backup
//  carries in the clear. The token lets whoever holds it connect as this
//  wallet; it stays on this device (`…ThisDeviceOnly`) and is readable only
//  while the phone is unlocked — the agent answers only in the foreground.
//

import Foundation
import Security

/// The client token's one place.
nonisolated protocol AgentTokenVault {
    /// The token kept, or `nil`.
    func read() -> String?
    /// Keep `token`, replacing what was kept. `false` when the platform refused.
    @discardableResult func write(_ token: String) -> Bool
    /// Forget it — on an empty token, and on Erase This Device.
    func delete()
}

/// The Keychain's generic-password item for this app.
nonisolated struct KeychainTokenVault: AgentTokenVault {
    var service = "app.getvela.VelaWallet.agent"
    var account = "client-token"

    private var query: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    func read() -> String? {
        var lookup = query
        lookup[kSecReturnData as String] = true
        lookup[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        guard SecItemCopyMatching(lookup as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    func write(_ token: String) -> Bool {
        delete()
        var item = query
        item[kSecValueData as String] = Data(token.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        if status != errSecSuccess { print("[vela-wallet] agent: the token was not kept (\(status))") }
        return status == errSecSuccess
    }

    func delete() {
        SecItemDelete(query as CFDictionary)
    }

    /// Whether this build may use the Keychain at all. An unsigned build — CI's
    /// simulator runs — may not (`errSecMissingEntitlement`); every signed one
    /// may.
    static var available: Bool {
        let probe = KeychainTokenVault(service: "app.getvela.VelaWallet.agent.probe", account: "probe")
        defer { probe.delete() }
        return probe.write("probe")
    }
}

/// A vault in memory — for tests, and the fallback nothing persists to.
nonisolated final class MemoryTokenVault: AgentTokenVault, @unchecked Sendable {
    private var token: String?
    init(_ token: String? = nil) { self.token = token }
    func read() -> String? { token }
    func write(_ token: String) -> Bool { self.token = token; return true }
    func delete() { token = nil }
}
