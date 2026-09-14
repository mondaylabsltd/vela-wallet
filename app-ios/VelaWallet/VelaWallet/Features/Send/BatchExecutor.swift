//
//  BatchExecutor.swift
//  VelaWallet
//
//  The `batch_import` machine's three arms: a rate, a file, a template.
//
//  Ported from `app-android/.../feature/send/core/BatchExecutor.kt` (spec 045).
//  Three rules here are not obvious and each one is load-bearing.
//
//  **The rate is `nil` unless it is finite and above zero.** The display
//  currency's own resolver falls back to 1 so a screen always has something to
//  print; that fallback arriving here would read as "the rate really is 1", and
//  somebody's 5,000 of a currency worth a fraction of a dollar would be sent as
//  5,000 tokens. The core refuses to price on `nil`, which is the whole point.
//
//  **The file's EXTENSION decides text or matrix**, not the type the system
//  guessed. A CSV exported from Excel is routinely typed
//  `application/vnd.ms-excel`, and reading it as a zip would fail for a reason
//  nobody could act on.
//
//  **A missing document layer is a failure, not a cancel.** A cancel means
//  somebody changed their mind; if the ports were never attached, nobody was
//  asked anything, and telling the core "they cancelled" would be a lie the
//  screen then shows.
//

import Foundation

@MainActor
final class BatchExecutor {

    static let operations = ["fetch_usd_fiat_rate", "pick_file", "save_template_file"]

    /// USD → this currency. `nil` is the honest "no source could price it",
    /// **including when the source threw**.
    private let fiatRate: (String) async -> Double?
    private let documents: () -> DocumentPorts?

    init(fiatRate: @escaping (String) async -> Double?, documents: @escaping () -> DocumentPorts?) {
        self.fiatRate = fiatRate
        self.documents = documents
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "fetch_usd_fiat_rate":
            let code = operation["code"] as? String ?? ""
            let rate = await fiatRate(code)
            return CoreJSON.string([
                "type": "rate_resolved",
                "code": code,
                "rate": (rate?.isFinite == true && (rate ?? 0) > 0) ? rate as Any : NSNull(),
            ])

        case "pick_file":
            guard let ports = documents() else {
                print("[vela-wallet] batch_import: no document ports attached")
                return CoreJSON.string(["type": "file_pick_failed"])
            }
            guard let picked = await ports.pick(types: DocumentTypes.recipientTable) else {
                return CoreJSON.string(["type": "file_pick_cancelled"])
            }
            let content: [String: Any]
            if picked.name.lowercased().hasSuffix(".xlsx") {
                guard let rows = XlsxMatrix.rows(from: picked.bytes) else {
                    // A workbook this build cannot read is a FILE error, not an
                    // empty list of people.
                    return CoreJSON.string(["type": "file_pick_failed"])
                }
                content = ["type": "matrix", "rows": rows]
            } else {
                content = ["type": "text", "text": String(decoding: picked.bytes, as: UTF8.self)]
            }
            return CoreJSON.string([
                "type": "file_picked", "name": picked.name, "content": content,
            ])

        case "save_template_file":
            let name = operation["name"] as? String ?? "template.csv"
            let contents = operation["contents"] as? String ?? ""
            let mime = operation["mime"] as? String ?? "text/csv"
            let saved = await documents()?.create(
                name: name, type: DocumentTypes.type(forMime: mime), bytes: Data(contents.utf8)
            ) ?? false
            return CoreJSON.string(["type": saved ? "template_saved" : "template_save_failed"])

        default:
            print("[vela-wallet] batch_import: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "fetch_usd_fiat_rate":
            return CoreJSON.string([
                "type": "rate_resolved",
                "code": operation["code"] as? String ?? "",
                "rate": NSNull(),
            ])
        case "pick_file":
            return CoreJSON.string(["type": "file_pick_failed"])
        default:
            return CoreJSON.string(["type": "template_save_failed"])
        }
    }
}
