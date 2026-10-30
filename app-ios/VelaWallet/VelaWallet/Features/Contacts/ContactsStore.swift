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

    /// The view as the core wrote it — the AI agent reads the wallet whole
    /// (`AgentShellBridge.views`, spec 104).
    var viewJson: String? { core.json }

    /// No effect in flight: every read and write the machine asked for has
    /// been answered — `CoreDriver.isIdle`.
    var isIdle: Bool { core.isIdle }
    /// The address the book is currently scoped to, so a repeated appearance
    /// does not re-announce the same account and reset the machine for nothing.
    private var scopedTo: String??

    init(store: VelaStore, identity: RecipientIdentity? = nil, pool: RpcPool? = nil) {
        self.executor = ContactsExecutor(store: store, identity: identity, pool: pool)
        self.core = CoreStore(
            bridge: ContactsCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view },
            // An answer the core refused: unreadable, not kept — never a
            // silent success.
            neutralAnswer: { ContactsExecutor.neutralAnswer($0) },
            // The view as the core wrote it, for the AI agent's world (spec 104).
            keepsJson: true
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

    /// The form's 保存 — new or edited, the same event either way (it is
    /// idempotent on the address).
    ///
    /// **The validation is the CORE's.** The form does not decide whether an
    /// address is one; it hands over what was typed and shows what comes back.
    /// A shell that validated first would eventually disagree with the machine
    /// that stores the result.
    ///
    /// `note`: `nil` keeps the contact's note as it is; a string replaces it,
    /// and an empty one clears it (the core's `ContactSaveInput.note`).
    func save(address: String, name: String?, note: String? = nil) {
        let trimmed = (name ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        core.dispatch(CoreJSON.string([
            "type": "save",
            "input": [
                "address": address,
                "name": trimmed.isEmpty ? NSNull() : trimmed as Any,
                "note": note.map { Self.cleanNote($0) as Any } ?? NSNull(),
                "favorite": NSNull(),
                "kind": NSNull(),
                "resolved_name": NSNull(),
                "resolved_source": NSNull(),
            ],
            "now_ms": Self.nowMs,
        ]))
    }

    /// A note is at most this long (the core's `intents.rs` cap, the web's
    /// `NOTE_MAX_CHARS`).
    static let noteMaxChars = 280

    /// The web's `cleanNote`: control characters out (a line break and a tab
    /// stay), the bidirectional controls that turn text around out, trimmed,
    /// and no longer than the cap.
    static func cleanNote(_ raw: String) -> String {
        let bidi: Set<UInt32> = [0x200E, 0x200F, 0x202A, 0x202B, 0x202C, 0x202D, 0x202E,
                                 0x2066, 0x2067, 0x2068, 0x2069, 0x061C]
        let kept = raw.unicodeScalars.filter { scalar in
            let v = scalar.value
            if v == 0x0A || v == 0x09 { return true }
            if v < 0x20 || (v >= 0x7F && v <= 0x9F) { return false }
            return !bidi.contains(v)
        }
        let text = String(String.UnicodeScalarView(kept)).trimmingCharacters(in: .whitespacesAndNewlines)
        return String(text.prefix(noteMaxChars))
    }

    /// The send history the suggestions come from changed (a send was
    /// opened, the book shown): read it again, without unloading the book
    /// (`history_changed`).
    func historyChanged() { core.dispatch(CoreJSON.string(["type": "history_changed"])) }

    /// The star. Flips a saved contact's, or promotes an unsaved suggestion to
    /// a starred saved contact — which is why it carries a clock.
    func toggleFavorite(address: String) {
        core.dispatch(CoreJSON.string([
            "type": "toggle_favorite",
            "address": address,
            "now_ms": Self.nowMs,
        ]))
    }

    /// A picked file, as its BYTES. The core decodes them (issue 333), sniffs
    /// JSON from CSV, refuses a bad file before ANY write, and applies
    /// existing-wins.
    func importFile(bytes: Data, filename: String?, intoGroup: String? = nil) {
        core.dispatch(Self.importEvent(bytes: bytes, filename: filename, intoGroup: intoGroup, nowMs: Self.nowMs))
    }

    /// The `import_file` event: the bytes as the core's `Vec<u8>`, one number
    /// each, never decoded here. `String(decoding:as: UTF8.self)` used to
    /// decode them, and it turns bytes that are not UTF-8 into U+FFFD without
    /// a word — a GBK CSV from Excel imported every Chinese name as
    /// `jxjjx����`.
    nonisolated static func importEvent(bytes: Data, filename: String?, intoGroup: String?, nowMs: Double) -> String {
        CoreJSON.string([
            "type": "import_file",
            "bytes": bytes.map { Int($0) },
            "filename": filename.map { $0 as Any } ?? NSNull(),
            "into_group": intoGroup.map { $0 as Any } ?? NSNull(),
            "now_ms": nowMs,
        ])
    }

    /// The report (or the refusal) was read.
    func importAcknowledged() {
        core.dispatch(CoreJSON.string(["type": "import_acknowledged"]))
    }

    /// Write the book — or one group — into a file the shell then hands over.
    func exportRequested(groupId: String? = nil, json: Bool = true) {
        core.dispatch(CoreJSON.string([
            "type": "export_requested",
            "scope": groupId.map { ["type": "group", "id": $0] as [String: Any] }
                ?? ["type": "all"],
            "format": json ? "json" : "csv",
            "exported_at_iso": Self.nowIso,
        ]))
    }

    /// The file reached the share sheet; the one-shot leaves the view.
    func exportTaken() {
        core.dispatch(CoreJSON.string(["type": "export_taken"]))
    }

    /// Which groups hold one contact — the whole answer, not a delta: the
    /// contact joins every listed group and leaves every other.
    func setContactGroups(address: String, groupIds: [String]) {
        core.dispatch(CoreJSON.string([
            "type": "set_contact_groups",
            "address": address,
            "group_ids": groupIds,
        ]))
    }

    /// A recipient came on screen. The core resolves the identity and the
    /// classification and projects the trust line.
    func inspect(address: String, chainId: Int) {
        core.dispatch(CoreJSON.string([
            "type": "inspect_recipient",
            "chain_id": chainId,
            "address": address,
        ]))
    }

    /// 新建分组 / 重命名分组 — the same event for both, which is what `id`
    /// being optional in `ContactGroupInput` means: absent mints a group,
    /// present renames the one it names. **Members are not sent**: `nil` there
    /// tells the core to leave the membership alone, and `[]` would empty a
    /// group somebody only meant to rename.
    func saveGroup(id: String?, name: String) {
        core.dispatch(CoreJSON.string([
            "type": "group_save",
            "input": [
                "id": id ?? NSNull(),
                "name": name,
                "color": NSNull(),
                "members": NSNull(),
            ] as [String: Any],
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

    /// The shell's clock again, in the form the export stamps its backup with
    /// and dates its filename from. ISO-8601, UTC — the same string every
    /// other client writes, so a backup made on a phone reads on a desktop.
    private static var nowIso: String {
        let formatter = ISO8601DateFormatter()
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        return formatter.string(from: Date())
    }
}
