//
//  HostedScreen.swift
//  VelaWalletTests
//
//  Any real screen, hosted in a window of the app's own scene
//  (`HostedWindow`), with what it draws read back two ways:
//
//  - the tree an assistive client gets — every word, its identifier and
//    where it stands — once two reads in a row agree;
//  - the pixels the window drew, for what the tree cannot say: whether a
//    thing that is THERE is also SEEN (content behind the status bar, a
//    control drawn over a line of text).
//
//  One live host per value, its root swapped under it (`show`), so a view's
//  own state carries from step to step as it does on a phone. A hosted
//  screen is heavy on the one main actor every suite shares: a suite hosts
//  few of them.
//

import Foundation
import Observation
import SwiftUI
import UIKit
@testable import VelaWallet

@MainActor
final class HostedScreen {

    /// One node of the tree: something an assistive client reads.
    struct Element: Equatable {
        let label: String
        let id: String
        let frame: CGRect
        let enabled: Bool
    }

    struct Seen {
        let tree: [Element]

        var words: [String] { tree.map(\.label).filter { !$0.isEmpty } }
        func element(_ id: String) -> Element? { tree.first { $0.id == id } }
        func elements(_ id: String) -> [Element] { tree.filter { $0.id == id } }
        func labelled(_ label: String) -> Element? { tree.first { $0.label == label } }
        func containing(_ text: String) -> Element? { tree.first { $0.label.contains(text) } }
        func says(_ text: String) -> Bool { tree.contains { $0.label.contains(text) } }
    }

    @MainActor
    @Observable
    final class Box {
        var root: AnyView
        init(_ root: AnyView) { self.root = root }
    }

    private struct Host: View {
        let box: Box
        var body: some View { box.root }
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
    let window: UIWindow
    let size: CGSize

    /// The iPhone 17's screen in points, and the SE's (3rd generation).
    static let phone = CGSize(width: 390, height: 844)
    static let se = CGSize(width: 375, height: 667)

    init<Content: View>(_ root: Content, size: CGSize = HostedScreen.phone) {
        _ = Self.automation
        self.size = size
        box = Box(AnyView(root))
        host = UIHostingController(rootView: Host(box: box))
        window = HostedWindow.show(host, size: size)
    }

    func close() { window.isHidden = true }

    /// The top safe area of the hosted window — the status bar's room.
    var safeTop: CGFloat { window.safeAreaInsets.top }
    var safeBottom: CGFloat { window.safeAreaInsets.bottom }

    /// Swap the root under the live host and read it once it has settled.
    func show<Content: View>(_ root: Content, until ready: (Seen) -> Bool = { _ in true }) async throws -> Seen {
        box.root = AnyView(root)
        return try await read(until: ready)
    }

    /// The tree, once `ready` holds of it and two reads in a row agree on
    /// every frame (40 tries at most): beside a thousand other tests on one
    /// main actor, one read can come before the tree has followed the last
    /// commit.
    func read(until ready: (Seen) -> Bool = { _ in true }) async throws -> Seen {
        var last: [Element]?
        var seen = Seen(tree: [])
        for _ in 0..<40 {
            try await drawn()
            var tree: [Element] = []
            collect(host.view as Any, depth: 0, into: &tree)
            seen = Seen(tree: tree)
            if !tree.isEmpty, ready(seen) {
                if tree == last { return seen }
                last = tree
            } else {
                last = nil
            }
            try await Task.sleep(for: .milliseconds(50))
        }
        return seen
    }

    /// Laid out and committed: a drawing waits for the screen's update,
    /// which is also what places the tree's nodes.
    private func drawn() async throws {
        let view: UIView = host.view
        view.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        _ = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in
            view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        try await Task.sleep(for: .milliseconds(50))
    }

    // MARK: - A sheet over the screen

    /// What a presented sheet shows: its own frame in the window, and the
    /// tree of what is on it.
    struct Sheet {
        /// The sheet's frame in the window — its top edge is where it stands.
        let frame: CGRect
        let seen: Seen
        /// The lowest point a thing on the sheet can be and still be reached:
        /// the window's bottom, less the home indicator's room.
        let floor: CGFloat
    }

    /// The sheet this screen presented, once it has stopped moving (two reads
    /// agree on its frame and its tree) — `nil` if none came up.
    func sheet(until ready: (Seen) -> Bool = { _ in true }) async throws -> Sheet? {
        var last: (CGRect, [Element])?
        for _ in 0..<60 {
            try await Task.sleep(for: .milliseconds(120))
            guard let presented = host.presentedViewController, let view = presented.view,
                  view.window != nil
            else { continue }
            view.layoutIfNeeded()
            _ = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
            }
            let frame = view.convert(view.bounds, to: window)
            var tree: [Element] = []
            collect(view as Any, depth: 0, into: &tree)
            let seen = Seen(tree: tree)
            if !tree.isEmpty, ready(seen), let last, last.0 == frame, last.1 == tree {
                return Sheet(frame: frame, seen: seen, floor: window.bounds.height - window.safeAreaInsets.bottom)
            }
            last = (frame, tree)
        }
        return nil
    }

    // MARK: - Scrolling

    /// Every scroll view under the host, outermost first.
    var scrollViews: [UIScrollView] {
        var found: [UIScrollView] = []
        func walk(_ view: UIView) {
            if let scroll = view as? UIScrollView { found.append(scroll) }
            view.subviews.forEach(walk)
        }
        walk(host.view)
        return found
    }

    /// The page's own scroll: the first full-width vertical scroll view.
    var pageScroll: UIScrollView? {
        scrollViews.first {
            $0.bounds.width > size.width / 2 && $0.bounds.height > size.height / 3
                && $0.contentSize.width <= $0.bounds.width + 1
        }
    }

    /// Whether the page has more content than room — a person can scroll it.
    var scrolls: Bool {
        guard let scroll = pageScroll else { return false }
        return scroll.contentSize.height > scroll.bounds.height + 1
    }

    /// Move the page's content `points` up from rest — past its end too, as
    /// a finger's rubber band does on a page shorter than its room.
    func scroll(by points: CGFloat) async throws {
        guard let scroll = pageScroll else { return }
        let rest = -scroll.adjustedContentInset.top
        scroll.setContentOffset(CGPoint(x: 0, y: rest + max(0, points)), animated: false)
        try await drawn()
    }

    // MARK: - Pixels

    /// The window as it is drawn, one point = one pixel, RGBA.
    struct Picture {
        let width: Int
        let height: Int
        let bytes: [UInt8]

        func pixel(_ x: Int, _ y: Int) -> (r: Int, g: Int, b: Int) {
            let at = (y * width + x) * 4
            return (Int(bytes[at]), Int(bytes[at + 1]), Int(bytes[at + 2]))
        }

        /// How many pixels of `rect` differ from `colour` by more than
        /// `tolerance` in any channel.
        func strangers(in rect: CGRect, to colour: (r: Int, g: Int, b: Int), tolerance: Int = 6) -> Int {
            var count = 0
            let xs = max(0, Int(rect.minX))..<min(width, Int(rect.maxX))
            let ys = max(0, Int(rect.minY))..<min(height, Int(rect.maxY))
            for y in ys {
                for x in xs {
                    let p = pixel(x, y)
                    if abs(p.r - colour.r) > tolerance || abs(p.g - colour.g) > tolerance
                        || abs(p.b - colour.b) > tolerance { count += 1 }
                }
            }
            return count
        }
    }

    /// Where the strangers are, for a failure's message.
    static func where_(_ picture: Picture, in rect: CGRect, to colour: (r: Int, g: Int, b: Int)) -> String {
        var minX = Int.max, maxX = -1, minY = Int.max, maxY = -1
        var sample = ""
        for y in max(0, Int(rect.minY))..<min(picture.height, Int(rect.maxY)) {
            for x in max(0, Int(rect.minX))..<min(picture.width, Int(rect.maxX)) {
                let p = picture.pixel(x, y)
                if abs(p.r - colour.r) > 6 || abs(p.g - colour.g) > 6 || abs(p.b - colour.b) > 6 {
                    minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y)
                    if sample.isEmpty { sample = "first (\(x),\(y)) = \(p) vs \(colour)" }
                }
            }
        }
        return maxX < 0 ? "none" : "x \(minX)…\(maxX), y \(minY)…\(maxY); \(sample)"
    }

    /// Draw the whole window — everything in it, the way the screen shows it
    /// (the system's own status bar is not the app's and is not in it).
    /// `settled`: the caller has just waited for the screen (`read`,
    /// `scroll`), and one wait is enough.
    func picture(settled: Bool = false) async throws -> Picture {
        if !settled { try await drawn() }
        let width = Int(size.width), height = Int(size.height)
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        let space = CGColorSpaceCreateDeviceRGB()
        bytes.withUnsafeMutableBytes { buffer in
            guard let context = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: space,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return }
            // UIKit's coordinates: origin at the top left.
            context.translateBy(x: 0, y: CGFloat(height))
            context.scaleBy(x: 1, y: -1)
            UIGraphicsPushContext(context)
            window.drawHierarchy(in: CGRect(origin: .zero, size: size), afterScreenUpdates: true)
            UIGraphicsPopContext()
        }
        return Picture(width: width, height: height, bytes: bytes)
    }

    /// The window as a picture on disk, for a person to LOOK at — only when
    /// the run asks for it (`VELA_TEST_SHOTS=<folder>` in the test host's
    /// environment; a simulator writes to the Mac's own disk). Nothing is
    /// asserted on it and nothing is written otherwise.
    func save(_ name: String) async throws {
        guard let folder = ProcessInfo.processInfo.environment["VELA_TEST_SHOTS"], !folder.isEmpty else { return }
        try await drawn()
        let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        try? image.pngData()?.write(to: URL(fileURLWithPath: folder).appendingPathComponent("\(name).png"))
    }

    // MARK: - The tree

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
