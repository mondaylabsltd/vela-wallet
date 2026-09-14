//
//  ContactsSearchField.swift
//  VelaWallet
//
//  SearchField (spec 018 vocabulary #6, mock C1): sunken full-width well,
//  leading search glyph, 搜索名字、ENS 或地址 placeholder, trailing clear
//  affordance once a query is present (C1f). Filtering happens in the
//  fixture layer — this component never filters (FR-005).
//

import SwiftUI

struct ContactsSearchField: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ContactsSearchModel
    var onClear: () -> Void = {}
    /// What is being typed. Absent in the gallery, where the field is a
    /// picture of a query; present everywhere a person can search — and until
    /// spec 054 it was absent EVERYWHERE, so the field could not be typed in.
    var text: Binding<String>?

    private var hasQuery: Bool { !(model.query ?? "").isEmpty }

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            LucideIcon(.search, size: LucideIconSize.checkmark)
                .foregroundStyle(theme.fgSubtle)
            if let text {
                TextField(
                    "",
                    text: text,
                    prompt: Text(verbatim: model.placeholder).foregroundStyle(theme.fgSubtle)
                )
                .font(Typography.body.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.search)
                .accessibilityLabel(Text(verbatim: model.placeholder))
            } else {
                Text(verbatim: hasQuery ? (model.query ?? "") : model.placeholder)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(hasQuery ? theme.fgBase : theme.fgSubtle)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s8)
            if hasQuery {
                Button(action: onClear) {
                    LucideIcon(.close, size: LucideIconSize.checkmark)
                        .foregroundStyle(theme.fgMuted)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.clearLabel)
            }
        }
        .padding(.horizontal, Tokens.Space.s12)
        .frame(minHeight: ContactsGeometry.searchFieldHeight)
        .background {
            RoundedRectangle(cornerRadius: ContactsGeometry.searchFieldRadius)
                .fill(theme.bgRaised)
        }
    }
}

#Preview("Search field dark") {
    VStack(spacing: Tokens.Space.s16) {
        ContactsSearchField(model: ContactsSearchModel(
            placeholder: "搜索名字、ENS 或地址", query: nil, clearLabel: "取消"
        ))
        ContactsSearchField(model: ContactsSearchModel(
            placeholder: "搜索名字、ENS 或地址", query: "Ali", clearLabel: "取消"
        ))
    }
    .padding(Tokens.Layout.screenPaddingX)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
    .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("Search field light") {
    ContactsSearchField(model: ContactsSearchModel(
        placeholder: "Search name, ENS, or address", query: nil, clearLabel: "Cancel"
    ))
    .padding(Tokens.Layout.screenPaddingX)
    .themed(.light)
    .environment(\.lucideIconProvider, .previewSafe)
}
