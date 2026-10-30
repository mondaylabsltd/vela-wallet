//
//  VelaWidgets.swift
//  VelaWidgets
//
//  The widget extension (native parity N-I2): the balance on the Home
//  Screen and the Lock Screen, the scanner one tap from the Lock Screen,
//  and — on iOS 18 — Scan in the Control Center.
//
//  **What this extension is not allowed to be.** It holds no key, links no
//  core, makes no network request and writes nothing: it reads the one file
//  the app writes (`WidgetSnapshotStore`) and draws it. Its timeline never
//  asks to be refreshed — the app reloads it whenever the figure it showed
//  changes — so a widget never draws a figure the app did not. The store
//  binary check (`app-ios/scripts/check-ios-store-binary.sh`) reads the built
//  extension and fails when any of that stops being true.
//

import SwiftUI
import WidgetKit

@main
struct VelaWidgetsBundle: WidgetBundle {
    var body: some Widget {
        BalanceWidget()
        ScanWidget()
        if #available(iOS 18.0, *) {
            ScanControl()
        }
    }
}

// MARK: - The timeline

struct BalanceEntry: TimelineEntry {
    let date: Date
    /// `nil`: nothing to show — signed out, or never opened.
    let snapshot: WidgetSnapshot?
    /// The widget gallery's sample: drawn redacted, never a real figure.
    var sample = false
}

struct BalanceProvider: TimelineProvider {
    let store: WidgetSnapshotStore

    init(store: WidgetSnapshotStore = .shared) {
        self.store = store
    }

    func placeholder(in context: Context) -> BalanceEntry {
        BalanceEntry(date: Date(), snapshot: WidgetSamples.placeholder, sample: true)
    }

    func getSnapshot(in context: Context, completion: @escaping (BalanceEntry) -> Void) {
        // The gallery shows the person's own figure when there is one, and
        // a redacted sample when there is none — never a made-up balance.
        if let snapshot = store.read() {
            completion(BalanceEntry(date: Date(), snapshot: snapshot))
        } else {
            completion(BalanceEntry(date: Date(), snapshot: context.isPreview ? WidgetSamples.placeholder : nil,
                                    sample: context.isPreview))
        }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<BalanceEntry>) -> Void) {
        // One entry, and no refresh of its own: the app reloads the
        // timeline when it has a new figure (`WidgetPublisher`).
        completion(Timeline(entries: [BalanceEntry(date: Date(), snapshot: store.read())], policy: .never))
    }
}

/// The redacted shape the gallery draws before any figure exists.
enum WidgetSamples {
    static let placeholder = WidgetSnapshot(
        title: " ",
        currency: "",
        figure: .amount,
        integer: "$0,000",
        decimals: "00",
        spoken: "",
        partial: false,
        note: nil,
        asOf: nil,
        updatedLabel: "",
        accountName: nil,
        sendLabel: " ",
        receiveLabel: " ",
        scanLabel: " ",
        language: "en"
    )
}

// MARK: - The balance

struct BalanceWidget: Widget {
    static let kind = "app.getvela.VelaWallet.balance"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: Self.kind, provider: BalanceProvider()) { entry in
            BalanceWidgetView(entry: entry)
        }
        .configurationDisplayName(Text("widget.balance.name"))
        .description(Text("widget.balance.description"))
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryRectangular, .accessoryInline])
    }
}

struct BalanceWidgetView: View {
    @Environment(\.widgetFamily) private var family
    @Environment(\.colorScheme) private var scheme
    let entry: BalanceEntry

    var body: some View {
        face
            .redacted(reason: entry.sample ? .placeholder : [])
            .containerBackground(for: .widget) {
                WidgetPalette(scheme).page
            }
    }

    @ViewBuilder private var face: some View {
        switch family {
        case .systemMedium:
            BalanceMediumFace(snapshot: entry.snapshot, now: entry.date)
        case .accessoryRectangular:
            BalanceRectangularFace(snapshot: entry.snapshot, now: entry.date)
        case .accessoryInline:
            BalanceInlineFace(snapshot: entry.snapshot)
        default:
            BalanceSmallFace(snapshot: entry.snapshot, now: entry.date)
        }
    }
}

// MARK: - The scanner, from the Lock Screen

struct ScanWidget: Widget {
    static let kind = "app.getvela.VelaWallet.scan"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: Self.kind, provider: BalanceProvider()) { entry in
            ScanCircularFace(label: entry.snapshot?.scanLabel)
                .containerBackground(for: .widget) { Color.clear }
        }
        .configurationDisplayName(Text("widget.scan.name"))
        .description(Text("widget.scan.description"))
        .supportedFamilies([.accessoryCircular])
    }
}

// MARK: - The Control Center (iOS 18)

@available(iOS 18.0, *)
struct ScanControl: ControlWidget {
    static let kind = "app.getvela.VelaWallet.control.scan"

    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: Self.kind) {
            ControlWidgetButton(action: OpenScanIntent()) {
                Label("control.scan.title", systemImage: VelaEntry.scan.systemImage)
            }
        }
        .displayName("control.scan.title")
    }
}
