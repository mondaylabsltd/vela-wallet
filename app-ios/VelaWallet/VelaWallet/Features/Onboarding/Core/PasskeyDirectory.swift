//
//  PasskeyDirectory.swift
//  VelaWallet
//
//  Names for the authenticator models the compiled catalog cannot name.
//
//  The catalog carries software passkey providers; hardware keys live in the
//  FIDO metadata service, hundreds of models deep, which is what the directory
//  service answers for. It is OUR service and stores nothing (founder,
//  2026-08-26) — and the catalog still answers first, instantly and offline, so
//  this only ever runs for a key nothing on the device could name.
//
//  The core owns the contract: which AAGUIDs are worth asking about, and what
//  counts as an answer (the body must be about the question, and an icon path
//  must be the service's own shape before anything fetches it). This owns only
//  the transport and the memory.
//

import Foundation
import Observation
import SwiftUI
import VelaCore

/// A directory answer, in the shell's own vocabulary.
///
/// The FFI record stays inside this file: feature views speak the app's types,
/// which is the same rule the decoded core views follow.
struct PasskeyHolder: Equatable {
    let name: String
    let iconUrl: String?
}

@MainActor
@Observable
final class PasskeyDirectory {
    static let shared = PasskeyDirectory()

    /// A settled answer, or `nil` for "asked, nothing came back". Absent means
    /// nobody has asked yet.
    private var entries: [String: PasskeyHolder?] = [:]
    private var marks: [String: UIImage?] = [:]
    private var asking: Set<String> = []

    private let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        // A key list must not wait on a name. Nothing here is load-bearing.
        config.timeoutIntervalForRequest = 6
        return URLSession.vela(config)
    }()

    /// What the directory says about `aaguid`, asking it the first time.
    ///
    /// Reading this from a SwiftUI body subscribes that view to the answer, so
    /// the row redraws when it lands.
    func entry(aaguid: String, dark: Bool) -> PasskeyHolder? {
        let key = "\(aaguid.lowercased())|\(dark)"
        if let settled = entries[key] { return settled }
        guard !asking.contains(key) else { return nil }
        asking.insert(key)
        Task { await self.lookup(aaguid: aaguid, dark: dark, key: key) }
        return nil
    }

    /// The directory's mark for an entry, at `sizePx`. Fetched once per URL.
    func mark(for entry: PasskeyHolder, sizePx: UInt32) -> UIImage? {
        guard let url = entry.iconUrl else { return nil }
        let key = "\(url)|\(sizePx)"
        if let settled = marks[key] { return settled }
        guard !asking.contains(key) else { return nil }
        asking.insert(key)
        Task { await self.fetchMark(url: url, sizePx: sizePx, key: key) }
        return nil
    }

    private func lookup(aaguid: String, dark: Bool, key: String) async {
        defer { asking.remove(key) }
        guard let raw = passkeyDirectoryUrl(aaguid: aaguid), let url = URL(string: raw) else {
            entries[key] = .some(nil)
            return
        }
        var request = URLRequest(url: url)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await session.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200,
              let json = String(data: data, encoding: .utf8) else {
            // Offline, blocked, or a 404: remembered as "no answer" rather than
            // retried on every redraw. The row already shows something honest.
            entries[key] = .some(nil)
            return
        }
        let answer = passkeyDirectoryEntry(aaguid: aaguid, json: json, dark: dark)
        entries[key] = .some(answer.map { PasskeyHolder(name: $0.name, iconUrl: $0.iconUrl) })
    }

    private func fetchMark(url: String, sizePx: UInt32, key: String) async {
        defer { asking.remove(key) }
        guard let parsed = URL(string: url),
              let (data, response) = try? await session.data(from: parsed),
              (response as? HTTPURLResponse)?.statusCode == 200 else {
            marks[key] = .some(nil)
            return
        }
        // The service serves both: a PNG decodes directly, an SVG goes through
        // the same rasterizer every other piece of core artwork uses.
        if let image = UIImage(data: data) {
            marks[key] = .some(image)
            return
        }
        guard let svg = String(data: data, encoding: .utf8),
              let png = try? rasterizeSvgPng(svg: svg, sizePx: sizePx),
              let image = UIImage(data: png) else {
            marks[key] = .some(nil)
            return
        }
        marks[key] = .some(image)
    }
}
