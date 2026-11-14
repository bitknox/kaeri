// Go to page's field on iPad. With a hardware keyboard attached, a SwiftUI field asked to take
// the keys goes through the keyboard's focus system, which a popover that just showed turns
// down until a key is pressed, so G left the field without the keys. A text field of UIKit's
// own takes them as it reaches its window.

#if os(iOS)
import SwiftUI
import UIKit

struct GoToPageField: UIViewRepresentable {
    @Binding var text: String
    let placeholder: String
    /// Return: go to the page typed.
    let submit: () -> Void
    /// Esc: close the prompt.
    let close: () -> Void

    func makeUIView(context: Context) -> Field {
        let field = Field()
        field.borderStyle = .roundedRect
        field.keyboardType = .numberPad
        field.font = .preferredFont(forTextStyle: .body)
        field.adjustsFontForContentSizeCategory = true
        field.delegate = context.coordinator
        field.addTarget(context.coordinator, action: #selector(Coordinator.changed(_:)), for: .editingChanged)
        field.setContentHuggingPriority(.required, for: .vertical)
        update(field, context.coordinator)
        return field
    }

    func updateUIView(_ field: Field, context: Context) {
        update(field, context.coordinator)
    }

    private func update(_ field: Field, _ coordinator: Coordinator) {
        coordinator.field = self
        field.close = close
        field.placeholder = placeholder
        if field.text != text { field.text = text }
    }

    func makeCoordinator() -> Coordinator { Coordinator(field: self) }

    final class Coordinator: NSObject, UITextFieldDelegate {
        var field: GoToPageField

        init(field: GoToPageField) { self.field = field }

        @objc func changed(_ sender: UITextField) { field.text = sender.text ?? "" }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            field.submit()
            return false
        }
    }

    final class Field: UITextField {
        var close: () -> Void = {}

        override func didMoveToWindow() {
            super.didMoveToWindow()
            takeKeyboard { true }
        }

        override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let escapes = presses.filter(Self.isEscape)
            if !escapes.isEmpty { close() }
            let rest = presses.subtracting(escapes)
            if !rest.isEmpty { super.pressesBegan(rest, with: event) }
        }

        override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let rest = presses.filter { !Self.isEscape($0) }
            if !rest.isEmpty { super.pressesEnded(rest, with: event) }
        }

        private static func isEscape(_ press: UIPress) -> Bool {
            guard let key = press.key else { return false }
            return key.keyCode == .keyboardEscape && key.modifierFlags.isDisjoint(with: [.command, .alternate, .control, .shift])
        }
    }
}
#endif
