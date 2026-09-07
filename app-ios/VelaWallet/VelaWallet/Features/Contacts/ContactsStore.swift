//
//  ContactsStore.swift
//  VelaWallet
//
//  The `contacts` machine, app-resident.
//
//  Thin on purpose. It owns no rules: the whole file is a lifetime, a decoded
//  view, and the events the drawn controls can actually raise. Anything that
//  looks like a decision — whether a delete is allowed, what order rows come
//  in, whether an address is already saved — is in `vela-core` and stays there.
//
//  ## Why the account address is the first thing it hears
//
//  `AccountSwitched` is what scopes the book to one wallet. The core resets
//  everything on it: saved contacts, tombstones, groups, and the per-account
//  history and identity caches. Booting without it — or forgetting to re-send
//  it after a sign-in — shows one person another person's address book, which
//  is the single worst outcome this screen has.
//
//  ## What is deliberately absent
//
//  `Save` and `ToggleFavorite` are complete in the core and have **no drawn
//  control** on iOS: `design/contacts/C5` is a menu of choices (添加与导入导出),
//  not a form, and no mock anywhere carries a favourite affordance. Adding an
//  event with nothing to raise it would be dead code that reads as a feature;
//  the boundary is recorded in the spec instead (founder decision 2026-09-05).
//

import Foundation
import Observation
import VelaCore

/// uniffi generates one class per exported machine with no shared supertype, so
/// each one opts into `CoreBridge` where it is used. Declared here rather than
/// beside the other three conformances so that adding a machine touches no
/// shared file — which is what spec 050 SC-004 measures.
extension ContactsCore: CoreBridge {}

@MainActor
@Observable
final class ContactsStore {

    /// The core's view, decoded. `nil` until the screen boots the machine —
    /// a real state, and the one the neutral surface renders from. It is never
    /// a stand-in for fixture data.
    private(set) var view: ContactsViewWire?

    /// What is in the search box. A display concern, not the core's: the
    /// machine has no list-search event, so the narrowing happens in the
    /// builder (`ContactsLive`, judgement 2).
    var query: String?

    /// `true` once the core has read all three stores. Mutations sent before
    /// this are dropped by the core, so the screen must not offer them.
    var isLoaded: Bool { view?.loaded == true }

    private let executor: ContactsExecutor
    private var core: CoreStore<ContactsViewWire>!
    /// The address the book is currently scoped to, so a repeated appearance
    /// does not re-announce the same account and reset the machine for nothing.
    private var scopedTo: String??

    init(store: VelaStore, identity: RecipientIdentity? = nil, pool: RpcPool? = nil) {
        self.executor = ContactsExecutor(store: store, identity: identity, pool: pool)
        self.core = CoreStore(
            bridge: ContactsCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view }
        )
    }

    /// Called from the screen's `.task`, with the signed-in address.
    ///
    /// Idempotent in both directions: the first call boots the machine, later
    /// calls only speak up when the account actually changed.
    func open(myAddress: String?) {
        let address = myAddress?.isEmpty == true ? nil : myAddress
        if core.boot(Self.accountSwitched(address)) {
            scopedTo = .some(address)
            return
        }
        guard scopedTo != .some(address) else { return }
        scopedTo = .some(address)
        core.dispatch(Self.accountSwitched(address))
    }

    // MARK: - What the drawn controls raise

    /// Row swipe → 删除 → the confirm sheet's 删除.
    ///
    /// `now_ms` is the shell's clock because the core has none; it is what
    /// dates the tombstone, and a tombstone is what keeps a deleted
    /// history-derived contact from being re-derived on the next launch.
    func delete(address: String) {
        core.dispatch(CoreJSON.string([
            "type": "delete",
            "address": address,
            "now_ms": Self.nowMs,
        ]))
    }

    func deleteGroup(id: String) {
        core.dispatch(CoreJSON.string(["type": "group_delete", "id": id]))
    }

    func setGroupMembers(id: String, members: [String]) {
        core.dispatch(CoreJSON.string([
            "type": "set_group_members",
            "id": id,
            "members": members,
        ]))
    }

    // MARK: - Lookups the screens need

    func contact(at address: String) -> ContactWire? {
        view?.contacts.first { $0.address == address }
    }

    func group(id: String) -> ContactGroupWire? {
        view?.groups.first { $0.id == id }
    }

    private static func accountSwitched(_ address: String?) -> String {
        CoreJSON.string([
            "type": "account_switched",
            "my_address": address ?? NSNull(),
        ])
    }

    private static var nowMs: Double {
        Date().timeIntervalSince1970 * 1000
    }
}
