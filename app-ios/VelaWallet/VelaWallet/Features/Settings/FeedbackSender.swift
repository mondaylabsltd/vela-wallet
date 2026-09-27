//
//  FeedbackSender.swift
//  VelaWallet
//
//  The feedback sheet's model: the screenshots being attached, and the one
//  action with its states (round 3; screenshots 2026-09-26). The sheet had a
//  Send button with an empty action and an unbound field: a person typed,
//  pressed 发送 and watched nothing happen.
//
//  Two outcomes and no third: filed (the issue it became, to open), or the
//  prefilled GitHub form — the road that still works when the endpoint could
//  not file it. "Try again" stays available after a fallback, because it is a
//  real answer to a 429 or a dropped connection.
//
//  Screenshots are PUBLIC (founder's ruling) and leave the phone only as the
//  pixels `ScreenshotPrep` re-encoded — never the picked file. Send never
//  leaves one behind: pressed while a tile is still being prepared, it waits
//  (busy, "Sending…") for every tile in flight, then sends; a tile that could
//  not be prepared becomes the unsupported line and is dropped.
//

import Foundation
import Observation

@MainActor
@Observable
final class FeedbackSender {

    enum State: Equatable {
        case idle
        case sending
        case filed(number: Int, url: String, deduped: Bool, screenshotsDropped: Int)
        /// `screenshots`: how many rode on the report the endpoint refused —
        /// the GitHub form cannot carry them, and the sheet says so.
        case fallback(url: String, screenshots: Int)
    }

    /// One tile. `prepared` is `nil` while its bytes are still being decoded
    /// and re-encoded (the tile shows a spinner).
    struct Shot: Identifiable, Equatable {
        let id: UUID
        var prepared: ScreenshotPrep.Prepared?
    }

    /// Why the last pick was not taken whole — shown in the public-warning
    /// line's place until the next change.
    enum Notice: Equatable {
        case limit
        case unsupported
    }

    private(set) var state: State = .idle
    private(set) var shots: [Shot] = []
    private(set) var notice: Notice?

    @ObservationIgnored private let transport: BugReport.Transport
    @ObservationIgnored private let endpoint: String
    /// Picks still being loaded and prepared — what Send waits for.
    @ObservationIgnored private var inFlight: [UUID: Task<Void, Never>] = [:]

    init(endpoint: String = BugReport.configuredEndpoint, transport: @escaping BugReport.Transport = BugReport.liveTransport) {
        self.endpoint = endpoint
        self.transport = transport
    }

    var sending: Bool { state == .sending }

    /// How many more may be attached.
    var room: Int { max(0, ScreenshotPrep.maxCount - shots.count) }

    /// Nothing typed is nothing to file — the endpoint refuses it with a 400.
    static func ready(_ what: String) -> Bool {
        !what.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    // MARK: - Screenshots

    /// Attach picked items, in order. Each is a way to load its bytes (a
    /// `PhotosPickerItem`, or plain data in a test). Past the limit the first
    /// `room` are taken and the rest refused with `.limit`; one that is not a
    /// decodable image is refused with `.unsupported`.
    func add(_ loaders: [() async -> Data?]) async {
        // The form is inert while the report is on its way (v3 B9).
        guard !sending else { return }
        notice = nil
        let taken = Array(loaders.prefix(room))
        if taken.count < loaders.count { notice = .limit }
        let ids = taken.map { _ in UUID() }
        // Placeholders first, in pick order, so the tiles appear at once and
        // keep their order whichever finishes first.
        shots.append(contentsOf: ids.map { Shot(id: $0, prepared: nil) })
        await withTaskGroup(of: (UUID, ScreenshotPrep.Prepared?).self) { group in
            for (id, load) in zip(ids, taken) {
                group.addTask {
                    guard let data = await load() else { return (id, nil) }
                    let prepared = await Task.detached(priority: .userInitiated) {
                        ScreenshotPrep.prepare(data)
                    }.value
                    return (id, prepared)
                }
            }
            for await (id, prepared) in group {
                guard let index = shots.firstIndex(where: { $0.id == id }) else { continue }
                if let prepared {
                    shots[index].prepared = prepared
                } else {
                    shots.remove(at: index)
                    notice = .unsupported
                }
            }
        }
    }

    /// Start attaching picked items and return at once — the sheet's entry.
    /// The work is tracked, so a Send pressed meanwhile waits for it.
    func attach(_ loaders: [() async -> Data?]) {
        let key = UUID()
        inFlight[key] = Task { [weak self] in
            await self?.add(loaders)
            self?.inFlight[key] = nil
        }
    }

    /// Plain bytes — the tests', and the gallery's.
    func add(datas: [Data?]) async {
        await add(datas.map { data in { data } })
    }

    func remove(_ id: UUID) {
        guard !sending else { return }
        notice = nil
        shots.removeAll { $0.id == id }
    }

    /// A fresh form for a sheet opened anew — never while a report is on its
    /// way. The sender outlives the sheet (the settings page owns it), so a
    /// report whose sheet was closed mid-send still lands, and its outcome is
    /// said on the page; the NEXT sheet must not reopen on that old outcome.
    func reset() {
        guard !sending else { return }
        for task in inFlight.values { task.cancel() }
        inFlight = [:]
        state = .idle
        shots = []
        notice = nil
    }

    // MARK: - Sending

    /// Send the report: the person's words, the lines the sheet showed, and
    /// the screenshots that are ready, in tile order.
    func send(what: String, steps: String, previewLines: [String], version: String) async {
        guard !sending, Self.ready(what) else { return }
        state = .sending
        // Every tile still being prepared is part of this report: wait for
        // them (the button is already busy), never send without them. One
        // that fails has been removed with the unsupported line by then.
        while let pending = inFlight.values.first {
            await pending.value
        }
        let images = shots.compactMap { $0.prepared?.jpeg }
        let payload = BugReport.build(
            what: what, steps: steps, environmentLines: previewLines, version: version,
            screenshots: images
        )
        switch await BugReport.send(payload, endpoint: endpoint, transport: transport) {
        case .filed(let number, let url, let deduped, let dropped):
            state = .filed(number: number, url: url, deduped: deduped, screenshotsDropped: dropped)
        case .fallback(_, let url):
            state = .fallback(url: url, screenshots: images.count)
        }
    }
}
