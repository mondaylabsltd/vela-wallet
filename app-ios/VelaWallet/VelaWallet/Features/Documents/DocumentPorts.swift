//
//  DocumentPorts.swift
//  VelaWallet
//
//  Three verbs, because iOS has three sheets and they mean different things.
//
//  `pick` reads a file somebody chose. `create` keeps a file somewhere they
//  chose. `share` hands a file to whatever they want to do with it. Collapsing
//  the last two would put "AirDrop this to a colleague" and "keep a copy"
//  behind one control, which is the kind of merge that reads as tidy in a
//  diagram and as confusing on a phone.
//
//  ## A cancel is not a failure
//
//  Every method answers, always. Cancelling resumes the continuation with
//  `nil` or `false` — the same discipline `RpcPool` learned in 051, applied to
//  UIKit: a delegate that simply stops being called leaves a `Task` suspended
//  forever, and the screen waits for something that will never arrive.
//
//  Contract: `specs/054-ios-send-contacts-parity/research.md` D1.
//

import Foundation
import UIKit
import UniformTypeIdentifiers

/// A file somebody chose.
struct PickedDocument {
    let name: String
    let bytes: Data
}

@MainActor
protocol DocumentPorts {
    /// Let somebody choose a file. `nil` is a cancel.
    func pick(types: [UTType]) async -> PickedDocument?
    /// Write bytes somewhere they choose to keep them.
    func create(name: String, type: UTType, bytes: Data) async -> Bool
    /// Hand the bytes on.
    func share(name: String, type: UTType, bytes: Data) async -> Bool
}

enum DocumentTypes {
    /// The three a recipient list can arrive as.
    ///
    /// `xlsx`'s identifier is declared by the system only when an app that
    /// handles it is installed, so the extension is the fallback — and `.data`
    /// behind THAT, because a picker that lists nothing is a picker somebody
    /// cannot use to open the file sitting in front of them.
    static let xlsx = UTType("org.openxmlformats.spreadsheetml.sheet")
        ?? UTType(filenameExtension: "xlsx")
        ?? .data

    static let recipientTable: [UTType] = [.commaSeparatedText, .plainText, xlsx]

    /// An address book leaves as JSON or CSV.
    static let addressBook: [UTType] = [.json, .commaSeparatedText, .plainText]

    static func type(forMime mime: String) -> UTType {
        switch mime {
        case "application/json": .json
        case "text/csv": .commaSeparatedText
        default: .plainText
        }
    }
}

@MainActor
final class UIKitDocumentPorts: NSObject, DocumentPorts {

    /// The window the sheets present from — the same resolution the passkey
    /// ceremony already performs, reused rather than rewritten.
    private var presenter: UIViewController? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }?
            .windows
            .first { $0.isKeyWindow }?
            .rootViewController?
            .topmost
    }

    private var pickContinuation: CheckedContinuation<PickedDocument?, Never>?
    private var saveContinuation: CheckedContinuation<Bool, Never>?

    func pick(types: [UTType]) async -> PickedDocument? {
        guard let presenter else { return nil }
        return await withCheckedContinuation { continuation in
            pickContinuation = continuation
            // `asCopy: true`: the file is read into the app's own sandbox, so
            // no security-scoped bookmark has to be held open and a file on
            // iCloud is downloaded before it arrives.
            let picker = UIDocumentPickerViewController(
                forOpeningContentTypes: types, asCopy: true
            )
            picker.allowsMultipleSelection = false
            picker.delegate = self
            presenter.present(picker, animated: true)
        }
    }

    func create(name: String, type: UTType, bytes: Data) async -> Bool {
        guard let presenter, let url = write(name: name, bytes: bytes) else { return false }
        return await withCheckedContinuation { continuation in
            saveContinuation = continuation
            let picker = UIDocumentPickerViewController(forExporting: [url], asCopy: true)
            picker.delegate = self
            presenter.present(picker, animated: true)
        }
    }

    func share(name: String, type: UTType, bytes: Data) async -> Bool {
        guard let presenter, let url = write(name: name, bytes: bytes) else { return false }
        return await withCheckedContinuation { continuation in
            let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
            // iPhone-only, so no popover anchor is needed. On an iPad this
            // would crash without one, and this app is a phone app — said
            // here so a future iPad build finds the reason.
            sheet.completionWithItemsHandler = { _, completed, _, _ in
                continuation.resume(returning: completed)
            }
            presenter.present(sheet, animated: true)
        }
    }

    /// The bytes, as a real file with the name a person will see.
    ///
    /// Both sheets take URLs, not data: the name in the Files app and the name
    /// in the share sheet are this file's name, so a wallet export is not
    /// called `Untitled`.
    private func write(name: String, bytes: Data) -> URL? {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        do {
            try bytes.write(to: url, options: .atomic)
            return url
        } catch {
            print("[vela-wallet] documents: could not stage \(name): \(error)")
            return nil
        }
    }

    private func finishPick(_ document: PickedDocument?) {
        pickContinuation?.resume(returning: document)
        pickContinuation = nil
    }

    private func finishSave(_ saved: Bool) {
        saveContinuation?.resume(returning: saved)
        saveContinuation = nil
    }
}

extension UIKitDocumentPorts: UIDocumentPickerDelegate {

    nonisolated func documentPicker(
        _ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]
    ) {
        MainActor.assumeIsolated {
            // The export picker answers with the destination; the open picker
            // answers with a copy in this app's sandbox. One delegate, two
            // questions, told apart by which continuation is waiting.
            if saveContinuation != nil {
                finishSave(!urls.isEmpty)
                return
            }
            guard let url = urls.first, let bytes = try? Data(contentsOf: url) else {
                finishPick(nil)
                return
            }
            finishPick(PickedDocument(name: url.lastPathComponent, bytes: bytes))
        }
    }

    nonisolated func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        MainActor.assumeIsolated {
            // Both, because only one is ever waiting — and answering the one
            // that is not is a no-op rather than a hang.
            finishPick(nil)
            finishSave(false)
        }
    }
}

private extension UIViewController {
    /// The controller actually on top, so a sheet does not present from
    /// underneath one that is already up.
    var topmost: UIViewController {
        presentedViewController?.topmost ?? self
    }
}
