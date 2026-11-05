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
//  (a name, a link, calldata) wraps as text does. Font and colour come from the
//  caller's modifiers, as they would for a `Text`.
//

import SwiftUI

struct AddressTextView: View {
    let text: String
    var alignment: HorizontalAlignment = .leading

    init(_ text: String, alignment: HorizontalAlignment = .leading) {
        self.text = text
        self.alignment = alignment
    }

    var body: some View {
        if let (head, tail) = AddressText.halves(text) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 0) {
                    Text(verbatim: head)
                    Text(verbatim: tail)
                }
                VStack(alignment: alignment, spacing: 0) {
                    Text(verbatim: head)
                    Text(verbatim: tail)
                }
            }
            .lineLimit(1)
            // One address to VoiceOver and to a UI test, not two halves.
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: text))
        } else if AddressText.isShortForm(text) {
            Text(verbatim: text)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
        } else {
            Text(verbatim: text)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
