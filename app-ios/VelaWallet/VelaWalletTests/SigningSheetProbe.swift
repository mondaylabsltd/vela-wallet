//
//  SigningSheetProbe.swift
//  VelaWalletTests
//
//  The real `SigningSheet`, hosted in a window the size of a phone's sheet,
//  with what it draws read back from the tree an assistive client gets:
//  every word it says, where the confirm stands, whether it can be tapped,
//  and the line under it.
//
//  One live sheet per probe, its model swapped under it — so the view's own
//  state (the held note, the verdict's place) carries from step to step, as
//  it does on a phone. `SigningSheetSteadyTests` and
//  `SigningVerdictPlaceTests` each keep a reader of their own; this is the
//  same reader, for the suites of PR 3's last fixes.
//

import Foundation
import Observation
import SwiftUI
import UIKit
@testable import VelaWallet

@MainActor
final class SigningSheetProbe {

    @MainActor
    @Observable
    final class Box {
        var model: SigningModel
        init(_ model: SigningModel) { self.model = model }
    }

    struct Host: View {
        let box: Box
        var body: some View {
            SigningSheet(model: box.model, onClose: {}, onRefreshFee: {})
                .themed(.light)
        }
    }

    /// One node of the tree: something an assistive client reads.
    struct Element {
        let label: String
        let id: String
        let frame: CGRect
        /// Not dimmed: a control a tap reaches.
        let enabled: Bool
    }

    /// The sheet as it stands.
    struct Seen {
        let tree: [Element]

        /// Every word on the sheet, in tree order.
        var words: [String] { tree.map(\.label).filter { !$0.isEmpty } }

        func element(_ id: String) -> Element? { tree.first { $0.id == id } }

        /// The confirm: `signing.confirm`.
        var confirm: Element? { element("signing.confirm") }
        /// The line under a shut confirm: `signing.confirmBlock`. Absent
        /// while the gate is open (its room is kept, unseen and unread).
        var confirmLine: Element? { element("signing.confirmBlock") }

        /// `text` is one of the sheet's words, whole.
        func said(_ text: String) -> Bool { tree.contains { $0.label == text } }
        /// `text` is said somewhere on the sheet.
        func says(_ text: String) -> Bool { tree.contains { $0.label.contains(text) } }
    }

    private static let automation: Void = {
        guard let library = dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW) else { return }
        for name in ["_AXSSetAutomationEnabled", "_AXSApplicationAccessibilitySetEnabled"] {
            if let symbol = dlsym(library, name) {
                typealias Switch = @convention(c) (Int32) -> Void
                unsafeBitCast(symbol, to: Switch.self)(1)
            }
        }
    }()

    private let box: Box
    private let host: UIHostingController<Host>
    private let window: UIWindow

    init(_ model: SigningModel, size: CGSize = CGSize(width: 390, height: 844)) {
        _ = Self.automation
        box = Box(model)
        host = UIHostingController(rootView: Host(box: box))
        window = UIWindow(frame: CGRect(origin: .zero, size: size))
        window.rootViewController = host
        window.makeKeyAndVisible()
    }

    func close() { window.isHidden = true }

    /// Swap the model under the live sheet and read it once it has settled.
    func show(_ model: SigningModel) async throws -> Seen {
        box.model = model
        return try await read()
    }

    /// The tree, once two reads 150 ms apart agree on where the confirm and
    /// the line under it stand (up to ~6 s): beside a thousand other tests
    /// on one main actor, one read can come before the tree has followed
    /// the last commit.
    func read() async throws -> Seen {
        var last: [CGRect]?
        var seen = Seen(tree: [])
        for _ in 0..<40 {
            try await shown()
            var tree: [Element] = []
            collect(host.view as Any, depth: 0, into: &tree)
            seen = Seen(tree: tree)
            if let confirm = seen.confirm {
                let frames = [confirm.frame, seen.confirmLine?.frame ?? .zero]
                if frames == last { return seen }
                last = frames
            }
            try await Task.sleep(for: .milliseconds(50))
        }
        return seen
    }

    /// Laid out and committed: a drawing waits for the screen's update,
    /// which is also what places the tree's nodes.
    private func shown() async throws {
        let view: UIView = host.view
        view.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        _ = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in
            view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        try await Task.sleep(for: .milliseconds(50))
    }

    private static func identifier(of object: NSObject) -> String {
        let getter = #selector(getter: UIAccessibilityIdentification.accessibilityIdentifier)
        guard object.responds(to: getter) else { return "" }
        return object.perform(getter)?.takeUnretainedValue() as? String ?? ""
    }

    private func collect(_ any: Any, depth: Int, into found: inout [Element]) {
        guard depth < 48, let object = any as? NSObject else { return }
        if object.isAccessibilityElement {
            found.append(Element(
                label: object.accessibilityLabel ?? "", id: Self.identifier(of: object),
                frame: object.accessibilityFrame,
                enabled: !object.accessibilityTraits.contains(.notEnabled)
            ))
        }
        if let children = object.accessibilityElements {
            for child in children { collect(child, depth: depth + 1, into: &found) }
        } else if case let count = object.accessibilityElementCount(), count != NSNotFound, count > 0 {
            for index in 0..<count {
                if let child = object.accessibilityElement(at: index) {
                    collect(child, depth: depth + 1, into: &found)
                }
            }
        } else if let view = object as? UIView {
            for sub in view.subviews { collect(sub, depth: depth + 1, into: &found) }
        }
    }
}
