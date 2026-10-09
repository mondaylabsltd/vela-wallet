//
//  OnboardingGalleryScreen.swift
//  VelaWallet
//
//  Dev-only state gallery, rewritten to the v2 state set (spec 019 T136).
//
//  The important property is not the list but the RENDERER: every entry goes
//  through `screenFor` and the same five screens production uses, so a fixture
//  cannot look right here and wrong in the app. The 014 gallery drove a separate
//  presentation type, which meant it could — and is the whole reason those types
//  are gone.
//
//  Compiled out of Release entirely; in Debug it is reachable only with
//  VELA_GALLERY=1 (SIMCTL_CHILD_VELA_GALLERY=1 on the simulator, house style).
//

#if DEBUG

import SwiftUI

/// The gallery gate — env-read once, static (house style: LaunchAnimation /
/// ThemeOverride switches).
enum GalleryMode {
    static let isEnabled = ProcessInfo.processInfo.environment["VELA_GALLERY"] == "1"
}

struct OnboardingGalleryScreen: View {
    let loc: Loc
    @State private var scheme: ColorScheme = ThemeOverride.launchScheme ?? .dark
    /// VELA_GALLERY_FIXTURE=<code> preselects a fixture at launch so a
    /// screenshot loop can walk states without GUI scripting (dev-only).
    @State private var selected: StateFixture? = ProcessInfo.processInfo
        .environment["VELA_GALLERY_FIXTURE"]
        .flatMap(FlowFixtures.byCode)

    private var groups: [(String, [StateFixture])] {
        // `Dictionary(grouping:)` has no order, and a gallery whose sections
        // shuffle between launches is one nobody can navigate by memory.
        let names = ["Create", "Failures"]
        return names.map { name in (name, FlowFixtures.all.filter { $0.group == name }) }
    }

    var body: some View {
        NavigationStack {
            List {
                ForEach(groups, id: \.0) { name, fixtures in
                    Section(name) {
                        ForEach(fixtures) { fixture in
                            Button {
                                selected = fixture
                            } label: {
                                // The fixture's own name is data, not UI copy —
                                // it never translates.
                                Text(verbatim: fixture.code)
                            }
                        }
                    }
                }
            }
            .navigationTitle(Text(verbatim: "Flow Gallery"))
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        scheme = scheme == .dark ? .light : .dark
                    } label: {
                        Image(systemName: scheme == .dark ? "sun.max" : "moon")
                    }
                    .accessibilityLabel(Text(verbatim: "Toggle theme"))
                }
            }
        }
        .themed(scheme)
        .preferredColorScheme(scheme)
        .fullScreenCover(item: flowBinding) { fixture in
            if case .flow(let view) = fixture.fixture {
                GalleryFlowHost(loc: loc, view: view) { selected = nil }
                    .themed(scheme)
                    .preferredColorScheme(scheme)
            }
        }
        .sheet(item: sheetBinding) { fixture in
            if case .sheet(let kind, let confirmable) = fixture.fixture {
                FlowSheet(loc: loc, kind: kind, confirmable: confirmable) { _ in selected = nil }
                    .themed(scheme)
            }
        }
    }

    /// A flow step covers the list entirely, as it does in production — it IS a
    /// full screen there, and showing it in a sheet would be a picture of a
    /// layout the app never draws. The two presentations therefore need two
    /// bindings, each nil unless its own kind is selected.
    private var flowBinding: Binding<StateFixture?> {
        Binding(
            get: { if case .flow = selected?.fixture { selected } else { nil } },
            set: { if $0 == nil { selected = nil } }
        )
    }

    private var sheetBinding: Binding<StateFixture?> {
        Binding(
            get: { if case .sheet = selected?.fixture { selected } else { nil } },
            set: { if $0 == nil { selected = nil } }
        )
    }
}

/// One flow step, rendered by the production screens with every control inert.
///
/// A fixture has no core behind it, so a button that appeared to work would be
/// lying. The only live control is the way out.
private struct GalleryFlowHost: View {
    let loc: Loc
    let view: CreateView
    let onClose: () -> Void

    @State private var name: String = ""

    private var screen: FlowScreen { screenFor(view) }

    private var statusText: String? {
        guard let status = view.status, progressFor(status) == nil else { return nil }
        return loc.t(statusKeyToI18n(status))
    }

    var body: some View {
        FlowShell(
            backLabel: loc.t(I18nKeys.Flow.back),
            canGoBack: true,
            onBack: onClose
        ) {
            switch screen {
            case .loading:
                Color.clear
            case .name:
                NameScreen(
                    loc: loc,
                    view: view,
                    statusText: statusText,
                    name: $name,
                    onToggleAck: { _ in },
                    onSubmit: {},
                    onStartOver: {},
                    onLink: { _ in }
                )
            case .keys:
                KeysScreen(
                    loc: loc,
                    view: view,
                    onAddKey: { _ in },
                    onConfirmKey: { _ in },
                    onRemoveKey: { _ in },
                    onFinish: {},
                    // The picker opens on its fixture pages; a pick goes
                    // nowhere, as every control here does.
                    // The self-hosted page's build asks to be trusted, so the
                    // list and the chosen page show the answer beside it.
                    pageChoices: SigningPagePickerModel.choices(
                        pages: SigningPageFixtures.pages, selected: view.signingPage, loc: loc,
                        line: { SigningPageFixtures.askingLine($0) },
                        asksTrust: { SigningPageFixtures.asksTrust($0) }
                    ),
                    onChoosePage: { _ in }
                )
            case .progress:
                ProgressScreen(
                    loc: loc,
                    position: progressFor(view.status) ?? ProgressPosition(activeTask: 0, percent: 33),
                    keyCount: view.keys.count
                )
            case .retry:
                RetryScreen(
                    loc: loc,
                    detail: view.syncErrorDetail,
                    busy: false,
                    onRetry: {},
                    onStartOver: {},
                    onEditEndpoint: {}
                )
            case .done:
                DoneScreen(
                    loc: loc,
                    address: view.address ?? "",
                    walletName: view.keys.first?.name ?? view.name,
                    keys: view.keys,
                    onEnter: onClose
                )
            }
        }
        .onAppear { name = view.name }
    }
}

#endif
