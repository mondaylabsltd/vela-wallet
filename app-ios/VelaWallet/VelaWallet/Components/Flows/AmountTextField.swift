//
//  AmountTextField.swift
//  VelaWallet
//
//  A text field for an amount of money, cleaned by the core's rule (spec 073)
//  one edit at a time, inside the edit.
//
//  Every amount field used to be a SwiftUI `TextField` cleaned in
//  `.onChange`, and that dropped keystrokes (2026-09-28, the decimal-comma
//  device test failing one run in three: "0,5" typed, "0." left). An
//  `onChange` runs after the render that saw the edit, not in the edit. Type
//  "," and "5" quickly and three things happen in this order: the field reads
//  "0,", the "5" lands and the field reads "0,5", THEN the handler for "0,"
//  runs and writes back "0." — computed from the text as it was, over the text
//  as it is. The "5" is gone. When the two keys arrive before a render instead,
//  the handler sees "0,5" at once and cannot tell fast typing from a paste, so
//  under a decimal-point preset it refuses the lot. Either way the person's key
//  does nothing and nothing on screen says so.
//
//  `textField(_:shouldChangeCharactersIn:replacementString:)` is the one
//  place that sees each edit on its own, before it is shown, and knows
//  exactly what it was: which characters, where, and whether more than one
//  arrived at once (a paste). The core cleans it there and the field shows
//  the result in the same call, so the next key always lands on clean text.
//  Nothing is written back later, so nothing can be written back over it.
//
//  What the field emits is always clean. Its owner sends it on as is.
//

import SwiftUI
import UIKit

struct AmountTextField: View {
    @Environment(\.keyboardDone) private var done
    @Binding var text: String
    let placeholder: String
    /// The role's font, already at the person's text size (`scaled`).
    let font: UIFont
    let color: Color
    var alignment: NSTextAlignment = .natural
    var identifier: String?

    var body: some View {
        AmountTextFieldBox(
            text: $text, placeholder: placeholder, font: font,
            color: UIColor(color), alignment: alignment, identifier: identifier,
            done: done
        )
        // On the text's baseline, as a SwiftUI `TextField` is, so a unit set
        // beside the figure (`HStack(alignment: .firstTextBaseline)`) sits on
        // the same line. A `UITextField` centres its line in its height.
        .alignmentGuide(.firstTextBaseline) { Self.baseline(height: $0.height, font: font) }
        .alignmentGuide(.lastTextBaseline) { Self.baseline(height: $0.height, font: font) }
    }

    static func baseline(height: CGFloat, font: UIFont) -> CGFloat {
        height / 2 + (font.ascender + font.descender) / 2
    }
}

/// The label of the Done bar the app's own keypads carry (087 F28). Set once
/// at the root, in the app's language; `nil` (the gallery, a test) is no bar.
private struct KeyboardDoneKey: EnvironmentKey {
    static let defaultValue: String? = nil
}

extension EnvironmentValues {
    var keyboardDone: String? {
        get { self[KeyboardDoneKey.self] }
        set { self[KeyboardDoneKey.self] = newValue }
    }
}

/// One edit, as the field is about to apply it: the text after it, cleaned,
/// and where the caret goes. Apart from UIKit so a test can type into it.
enum AmountEdit {
    struct Result: Equatable {
        /// What the field shows after the edit.
        let text: String
        /// The caret, in UTF-16 units — UIKit's.
        let caret: Int
    }

    /// `replacement` put over `range` of `current`. `nil`: the edit is refused
    /// (a paste with no reading as one figure) and the field keeps `current`.
    static func apply(_ replacement: String, in range: NSRange, of current: String) -> Result? {
        let before = current as NSString
        guard range.location != NSNotFound, NSMaxRange(range) <= before.length else { return nil }
        let raw = before.replacingCharacters(in: range, with: replacement)
        // More than one character at once is a paste or an autofill. The field
        // knows, so the core is told rather than left to guess from lengths.
        let pasted = (replacement as NSString).length > 1
        guard let clean = AmountText.clean(raw, previous: current, pasted: pasted) else { return nil }
        let caret = range.location + (replacement as NSString).length
        return Result(text: clean, caret: AmountText.caret(raw: raw, clean: clean, caret: caret))
    }
}

private struct AmountTextFieldBox: UIViewRepresentable {
    @Binding var text: String
    let placeholder: String
    let font: UIFont
    let color: UIColor
    let alignment: NSTextAlignment
    let identifier: String?
    let done: String?

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

    func makeUIView(context: Context) -> UITextField {
        let field = UITextField()
        field.delegate = context.coordinator
        context.coordinator.field = field
        field.keyboardType = .decimalPad
        field.borderStyle = .none
        field.adjustsFontForContentSizeCategory = false
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.addTarget(context.coordinator, action: #selector(Coordinator.edited(_:)), for: .editingChanged)
        field.text = text
        style(field)
        return field
    }

    func updateUIView(_ field: UITextField, context: Context) {
        context.coordinator.text = $text
        style(field)
        context.coordinator.carryDone(done, on: field)
        // A value that did not come from this field: Max, the ⇄ swap, a
        // cleared form. The field's own edits are already here.
        if field.text != text { field.text = text }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextField, context: Context) -> CGSize? {
        let natural = uiView.intrinsicContentSize
        return CGSize(width: proposal.width ?? natural.width, height: max(natural.height, ceil(font.lineHeight)))
    }

    private func style(_ field: UITextField) {
        field.font = font
        field.textColor = color
        field.textAlignment = alignment
        field.accessibilityIdentifier = identifier
        field.attributedPlaceholder = NSAttributedString(
            string: placeholder,
            attributes: [.font: font, .foregroundColor: UIColor.placeholderText]
        )
    }

    final class Coordinator: NSObject, UITextFieldDelegate {
        var text: Binding<String>
        weak var field: UITextField?

        init(text: Binding<String>) {
            self.text = text
        }

        /// The decimal pad has no return key, so nothing on it ever put it
        /// away (087 F28, an iPhone 11): it covered 继续 until the form was
        /// scrolled by hand. A Done bar above it, as iOS's own number fields
        /// carry, labelled in the app's language. Rebuilt only when the label
        /// changes — a new accessory on every render would flicker the bar.
        func carryDone(_ label: String?, on field: UITextField) {
            let current = (field.inputAccessoryView as? UIToolbar)?.items?.last?.title
            guard current != label else { return }
            guard let label else {
                field.inputAccessoryView = nil
                field.reloadInputViews()
                return
            }
            let bar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
            let done = UIBarButtonItem(title: label, style: .done, target: self, action: #selector(finish))
            done.accessibilityIdentifier = "keyboard.done"
            bar.items = [UIBarButtonItem(systemItem: .flexibleSpace), done]
            bar.sizeToFit()
            field.inputAccessoryView = bar
            field.reloadInputViews()
        }

        @objc private func finish() {
            field?.resignFirstResponder()
        }

        func textField(
            _ field: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String
        ) -> Bool {
            let current = field.text ?? ""
            guard let edit = AmountEdit.apply(string, in: range, of: current) else { return false }
            let raw = (current as NSString).replacingCharacters(in: range, with: string)
            // Clean already: UIKit applies it, keeping its own caret and undo,
            // and `edited` reports it.
            if edit.text == raw { return true }
            field.text = edit.text
            if let at = field.position(from: field.beginningOfDocument, offset: edit.caret) {
                field.selectedTextRange = field.textRange(from: at, to: at)
            }
            // A programmatic `text` raises no `.editingChanged`.
            report(edit.text)
            return false
        }

        @objc func edited(_ field: UITextField) {
            report(field.text ?? "")
        }

        private func report(_ value: String) {
            if text.wrappedValue != value { text.wrappedValue = value }
        }
    }
}
