//
//  AgentPage.swift
//  VelaWallet
//
//  What only the drawn root can do for the agent (spec 104 phase 7): open a
//  screen, say which page is in front, open and close a send with its
//  screens, erase, clear a store. `RootView` owns that state — the section,
//  the contact or group on show, the flow stack, the storage page's tick — so
//  it registers these when it appears, as it does the scanner's callback.
//  Each is the same function its own control calls.
//

import Foundation

@MainActor
final class AgentPage {
    /// Open an intent's screen (`Screen` JSON, as an object) for the person.
    /// A refusal, in words, when it cannot be shown.
    var show: (_ screen: [String: Any]) -> String? = { _ in "the wallet is not open" }
    /// The page in front: `wallet`, `contacts`, `explore`, `settings`, or
    /// `welcome` while nobody is in.
    var name: () -> String = { "welcome" }
    /// The dApp signing request on its sheet (`SignRequestView` JSON), if any.
    var signing: () -> String? = { nil }
    /// A send, opened as every entry into Send opens one (a new journey), on
    /// `params` (`SendOpenParams`).
    var openSend: (_ params: [String: Any]) -> Void = { _ in }
    /// Whether a send screen is up.
    var sendShowing: () -> Bool = { false }
    /// Erase This Device; a refusal, in words, when something survived.
    var erase: @MainActor () async -> String? = { "the wallet is not open" }
    /// Clear one of the storage page's items.
    var clearStorageItem: (_ item: String) -> Void = { _ in }
    /// Clear the caches.
    var clearCaches: () -> Void = {}
    /// Read the balances again, as a pull does.
    var refreshBalances: () -> Void = {}
    /// Make `address` the account in front; `false` when no wallet has it.
    var switchAccount: @MainActor (_ address: String) async -> Bool = { _ in false }
    /// Spec 107: the signing sheet's own "no"; `false` when no sheet is up.
    var rejectSigning: () -> Bool = { false }
}
