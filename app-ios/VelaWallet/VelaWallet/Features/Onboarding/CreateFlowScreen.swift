//
//  CreateFlowScreen.swift
//  VelaWallet
//
//  The create journey, end to end.
//
//  This view holds no flow state. It renders whatever view the core last emitted
//  and sends events back — the whole mapping from `CreateView` to a screen is
//  `screenFor`, and it is the only place that decides which step is showing
//  (data-model §3).
//

import SwiftUI

struct CreateFlowScreen: View {
    let loc: Loc
    @Bindable var model: OnboardingModel
    let onExit: () -> Void
    let onLink: (ActionId) -> Void

    private var screen: FlowScreen { screenFor(model.createView) }

    private var statusText: String? {
        guard let status = model.createView?.status, progressFor(status) == nil else { return nil }
        return loc.t(statusKeyToI18n(status))
    }

    var body: some View {
        FlowShell(
            backLabel: loc.t(I18nKeys.Flow.back),
            // The one screen with no way back is the one where going back would
            // abandon work already in flight: a ceremony is running and a
            // passkey may already exist in the person's provider.
            canGoBack: screen != .progress,
            onBack: {
                // The core owns whether there is anywhere to go back TO — only
                // the key list has one. From the name screen it reports none,
                // and back then means leaving the flow, which is the host's
                // because the core has no idea what contains it.
                if model.createView?.canGoBack == true { model.goBack() } else { onExit() }
            }
        ) {
            content
        }
        .task { model.startCreate() }
        .onDisappear { model.disposeCreate() }
    }

    @ViewBuilder
    private var content: some View {
        if let view = model.createView {
            switch screen {
            case .loading:
                Text(loc.t(I18nKeys.Flow.confirmInPrompt)).typeRole(Typography.flowCaption)

            case .name:
                NameScreen(
                    loc: loc,
                    view: view,
                    statusText: statusText,
                    name: $model.name,
                    onToggleAck: model.toggleAck,
                    onSubmit: model.submit,
                    onStartOver: model.startOver,
                    onLink: onLink
                )

            case .keys:
                let pages = model.signingPages()
                KeysScreen(
                    loc: loc,
                    view: view,
                    onAddKey: model.addKey,
                    onConfirmKey: model.confirmKey,
                    onRemoveKey: model.removeKey,
                    onFinish: model.finishKeys,
                    // Spec 102: "Use a trusted signing page" — the pages this
                    // device trusts, each with its domain and this phone's
                    // check of it.
                    pageChoices: SigningPagePickerModel.choices(
                        pages: pages?.pages ?? [], selected: view.signingPage, loc: loc,
                        line: { SignerPageChecks.shared.line(for: $0) }
                    ),
                    onChoosePage: model.chooseSigningPage,
                    onAddPage: model.addSigningPage,
                    pageAddError: SigningPagesViewWire.addErrorKey(pages?.addError),
                    onPagesShown: {
                        for page in pages?.pages ?? [] { SignerPageChecks.shared.prime(page.url) }
                    }
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
                    busy: view.busy,
                    onRetry: model.retryUpload,
                    onStartOver: model.startOver,
                    onEditEndpoint: { model.endpointSheetOpen = true }
                )

            case .done:
                DoneScreen(
                    loc: loc,
                    address: view.address ?? "",
                    walletName: view.keys.first?.name ?? view.name,
                    keys: view.keys,
                    onEnter: model.enterWallet
                )
            }
        } else {
            // The core has not produced a view yet. One frame, at most —
            // constructing a core is synchronous.
            Color.clear
        }
    }
}
