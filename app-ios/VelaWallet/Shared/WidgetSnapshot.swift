//
//  WidgetSnapshot.swift
//  VelaWallet + VelaWidgets
//
//  What the widgets may show, and the one place it is kept (native parity
//  N-I2).
//
//  **The app writes it; the widget only reads it.** The widget extension
//  holds no key, makes no network call and links no core: it draws the hero
//  the app last drew — the same figure, split the same way, in the words
//  the app resolved from the corpus in the person's language — and says
//  when that figure was read ("Updated 14:32"). A total the extension
//  worked out for itself would be a second opinion about somebody's money,
//  and a stale one it did not label would be a lie.
//
//  The figure is only ever what the home showed: hidden stays hidden (the
//  mask, never the digits), a total the core could not state stays unstated,
//  a partial total says so, and signing out (or erasing the device) removes
//  the file — a Lock Screen must not keep a balance for a wallet that is
//  gone.
//

import Foundation

struct WidgetSnapshot: Codable, Equatable, Sendable {
    /// The shape this build writes. A file in another shape is not read: a
    /// widget that guessed at it could draw the wrong number.
    static let currentVersion = 1

    enum Figure: String, Codable, Sendable {
        /// The total, as `integer` + `.decimals`.
        case amount
        /// The person hid the balance in the app: the mask, never the digits.
        case hidden
        /// No total can be stated (nothing read yet, testnet space): the
        /// `note` stands in its place, or nothing does.
        case none
    }

    var version = Self.currentVersion
    /// "Total balance" — `home.totalBalance`, in the person's language.
    var title: String
    /// The display currency's code, as the hero's label names it (`USD`).
    var currency: String
    var figure: Figure
    /// "$1,383" — the glyph and the whole part, as the hero draws it.
    var integer: String?
    /// "28" — drawn quieter after the separator.
    var decimals: String?
    /// The whole figure as VoiceOver says it, or the mask's own name.
    var spoken: String
    /// The figure is a floor, not a sum: some networks did not answer.
    var partial: Bool
    /// The hero's own line when it has one ("Some balances are still
    /// updating.", "Checking…", the testnet space's words) — `nil` when it
    /// has nothing to say.
    var note: String?
    /// When the figure was read — the core's `last_refreshed_at_ms`. `nil`
    /// when nothing has been read, which the widget says by saying nothing.
    var asOf: Date?
    /// "Updated" — `home.balanceDetailUpdatedLabel`.
    var updatedLabel: String
    /// The account in front, by the name the person gave it.
    var accountName: String?
    /// The doors' names, in the person's language.
    var sendLabel: String
    var receiveLabel: String
    var scanLabel: String
    /// The app's language (a BCP 47 tag), for the time the widget prints.
    var language: String

    /// A snapshot the widget may draw: its own version, and no figure
    /// without digits.
    var readable: Bool {
        guard version == Self.currentVersion else { return false }
        if figure == .amount { return !(integer ?? "").isEmpty }
        return true
    }

    func label(for entry: VelaEntry) -> String {
        switch entry {
        case .send: sendLabel
        case .receive: receiveLabel
        case .scan: scanLabel
        }
    }
}

/// The file both sides agree on, in the App Group's container.
///
/// One JSON file rather than shared `UserDefaults`: it is written whole and
/// atomically, so the widget can never read half of a new figure beside half
/// of an old one, and its protection class is stated (a Lock Screen widget
/// is drawn while the phone is locked, so it is readable after the first
/// unlock — the same class the system gives a widget's own data).
struct WidgetSnapshotStore: Sendable {
    /// The App Group both targets carry in their entitlements.
    static let appGroup = "group.app.getvela.VelaWallet"
    static let fileName = "widget-snapshot.json"

    /// Where the file lives. `nil` when the container is not there — an
    /// unsigned build has no App Group — and then nothing is written or read.
    let directory: URL?

    init(directory: URL?) {
        self.directory = directory
    }

    /// The App Group's container.
    static var shared: WidgetSnapshotStore {
        WidgetSnapshotStore(
            directory: FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup)
        )
    }

    private var file: URL? { directory?.appendingPathComponent(Self.fileName, isDirectory: false) }

    func read() -> WidgetSnapshot? {
        guard let file, let data = try? Data(contentsOf: file),
              let snapshot = try? Self.decoder.decode(WidgetSnapshot.self, from: data),
              snapshot.readable
        else { return nil }
        return snapshot
    }

    /// Write the app's snapshot, whole. Whether it was written.
    @discardableResult
    func write(_ snapshot: WidgetSnapshot) -> Bool {
        guard let file, let data = try? Self.encoder.encode(snapshot) else { return false }
        do {
            try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
            return true
        } catch {
            return false
        }
    }

    /// Nothing to show: signed out, erased, or a figure the app withdrew.
    func clear() {
        guard let file else { return }
        try? FileManager.default.removeItem(at: file)
    }

    // Made per call: cheap, and a coder shared between the app's main actor
    // and the widget's timeline thread would be shared mutable state.
    private static var encoder: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .millisecondsSince1970
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }

    private static var decoder: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .millisecondsSince1970
        return decoder
    }
}
