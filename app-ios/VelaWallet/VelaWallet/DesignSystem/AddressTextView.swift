//
//  AddressTextView.swift
//  VelaWallet
//
//  An address (or a hash, or its short form) drawn so that no line break ever
//  falls inside it at a place nobody chose. r13: a full member address in the
//  multisig sheets wrapped wherever the line ran out — on Android its last
//  five digits stood alone on a line; SwiftUI's `Text` breaks a long hex
//  "word" the same way.
//
//  The rule is the desktop's (`app-desktop/.../ui/address.rs`) and Android's
//  (`VelaAddressText`): a full address is two unbreakable halves
//  (`AddressText.halves`) side by side where the width holds them, else the
//  second under the first; a short form is one unbreakable line; anything else
//  (a name, a link, calldata) wraps as text does. The type role is handed in
//  (`typeRole`/`monoRole` are `Text`-only) and drawn on every piece; colour and
//  selection come from the caller's modifiers.
//

import SwiftUI

struct AddressTextView: View {
    /// The face each piece is drawn in — a `Text`'s own role modifiers,
    /// applied to every piece (they are `Text`-only).
    enum Face {
        case type(TypeRole)
        case mono(MonoTypeRole)
    }

    let text: String
    let face: Face
    var alignment: HorizontalAlignment = .leading

    init(_ text: String, role: TypeRole, alignment: HorizontalAlignment = .leading) {
        self.text = text
        self.face = .type(role)
        self.alignment = alignment
    }

    init(_ text: String, mono: MonoTypeRole, alignment: HorizontalAlignment = .leading) {
        self.text = text
        self.face = .mono(mono)
        self.alignment = alignment
    }

    @ViewBuilder
    private func piece(_ part: String) -> some View {
        switch face {
        case .type(let role): Text(verbatim: part).typeRole(role)
        case .mono(let role): Text(verbatim: part).monoRole(role)
        }
    }

    var body: some View {
        if let (head, tail) = AddressText.halves(text) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 0) {
                    piece(head)
                    piece(tail)
                }
                VStack(alignment: alignment, spacing: 0) {
                    piece(head)
                    piece(tail)
                }
            }
            .lineLimit(1)
            // One address to VoiceOver and to a UI test, not two halves.
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: text))
        } else if AddressText.isShortForm(text) {
            piece(text)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
        } else {
            piece(text)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
