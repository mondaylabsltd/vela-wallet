//
//  ContactFormSheet.swift
//  VelaWallet
//
//  C7 / C8 — the add and edit form, as one sheet in two states.
//
//  Drawn from spec 018's vocabulary and no new words: a raised sheet, two
//  labelled fields, the core's one error line, Save and Cancel. The same
//  component serves both, because "new" and "editing" differ by exactly two
//  facts — the title, and whether the address can still be changed.
//
//  **The address is locked when editing.** In the address book an address IS
//  the identity of the person; typing a different one does not rename them, it
//  names somebody else. Every client enforces that the same way.
//
//  Typing is echoed locally by the caller and round-trips through the core,
//  which validates and gates Save — this component never decides either.
//

import SwiftUI

struct ContactFormSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ContactFormModel
    /// Live bindings when the form is real; the gallery passes none and the
    /// fields render exactly as drawn.
    var nameText: Binding<String>?
    var addressText: Binding<String>?
    var onSave: () -> Void = {}
    var onCancel: () -> Void = {}

    @State private var contentHeight: CGFloat = 0
    /// Fallbacks, so the drawn sheet shows its fixture values without needing
    /// a caller to own state for a picture.
    @State private var drawnName = ""
    @State private var drawnAddress = ""

    private var name: Binding<String> { nameText ?? $drawnName }
    private var address: Binding<String> { addressText ?? $drawnAddress }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            Capsule()
                .fill(theme.borderStrong)
                .frame(width: WalletGeometry.sheetHandleWidth, height: WalletGeometry.sheetHandleHeight)
                .frame(maxWidth: .infinity)
                .padding(.top, Tokens.Space.s8)

            Text(verbatim: model.title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .padding(.top, Tokens.Space.s20)

            NameField(
                label: model.nameLabel,
                placeholder: model.namePlaceholder,
                helper: "",
                tooLongText: "",
                text: name,
                tooLong: false
            )
            .padding(.top, Tokens.Space.s20)

            Group {
                if model.addressLocked {
                    // Not a disabled field: a locked address is a FACT about
                    // this contact, and the detail page's own address block is
                    // how the app already states one.
                    VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                        Text(verbatim: model.addressLabel)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                        Text(verbatim: model.address)
                            .monoRole(Typography.monoAddressDetail.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(Tokens.Space.s12)
                            .background(
                                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                                    .fill(theme.bgSunken)
                            )
                    }
                } else {
                    FlowMonoInput(
                        value: address,
                        label: model.addressLabel,
                        placeholder: model.addressPlaceholder,
                        error: model.error,
                        well: theme.bgSunken
                    )
                }
            }
            .padding(.top, Tokens.Space.s16)

            VelaButton(title: model.save, kind: .primary, enabled: model.saveEnabled, action: onSave)
                .padding(.top, Tokens.Space.s20)

            Button(action: onCancel) {
                Text(verbatim: model.cancel)
                    .typeRole(Typography.button.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: Tokens.Control.lg)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(.top, Tokens.Space.s4)
            .padding(.bottom, Tokens.Space.s24)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .frame(maxWidth: .infinity, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
        .onGeometryChange(for: CGFloat.self) { proxy in
            proxy.size.height
        } action: { height in
            contentHeight = height
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onAppear {
            drawnName = model.name
            drawnAddress = model.address
        }
        .presentationDetents([.height(max(contentHeight, 1))])
        .presentationDragIndicator(.hidden)
        .presentationCornerRadius(Tokens.Radius.r20)
        .presentationBackground(theme.bgRaised)
    }
}

#Preview("Contact form · add") {
    ContactFormSheet(model: ContactsFixtures.contactForm(loc: ContactsPreviewData.loc, edit: false))
        .background(Tokens.dark.bgRaised.color)
        .themed(.dark)
        .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("Contact form · edit") {
    ContactFormSheet(model: ContactsFixtures.contactForm(loc: ContactsPreviewData.loc, edit: true))
        .background(Tokens.dark.bgRaised.color)
        .themed(.dark)
        .environment(\.lucideIconProvider, .previewSafe)
}
