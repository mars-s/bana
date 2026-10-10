import SwiftUI
import UIKit

/// Carets: the engine counts Unicode scalars (Rust chars); UIKit counts UTF-16 units.
extension String {
    func scalarOffset(utf16 n: Int) -> Int {
        String(utf16.prefix(max(0, n)))?.unicodeScalars.count ?? unicodeScalars.count
    }

    func utf16Offset(scalars n: Int) -> Int {
        var units = 0
        for (i, s) in unicodeScalars.enumerated() {
            if i >= n { break }
            units += s.utf16.count
        }
        return units
    }
}

/// What an outline line can ask for besides typing.
struct LineActions {
    var changed: (String, Int) -> Void
    var began: (Int) -> Void
    var returned: (Int) -> Void
    var backspaceAtStart: () -> Void = {}
    var ended: () -> Void = {}
    var accessory: [AccessoryItem] = []
}

struct AccessoryItem {
    let id: String
    let symbol: String
    let label: String
    let action: () -> Void
}

/// One editable line: a UITextView that sends every change to the engine at once (it never holds
/// text the engine does not have), turns Return into a new line and Backspace at the start into a
/// merge with the line above. It takes the keyboard when the engine says the caret is in it.
struct LineEditor: UIViewRepresentable {
    let text: String
    /// Shown while not editing (inline references as names); nil: same as `text`.
    let display: String?
    let focused: Bool
    let caret: Int
    var size: CGFloat = Phone.body
    var weight: CGFloat? = nil
    var color: UIColor = .label
    var strike = false
    var identifier = "line"
    let actions: LineActions

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> LineTextView {
        let v = LineTextView()
        v.isScrollEnabled = false
        v.backgroundColor = .clear
        v.textContainerInset = .zero
        v.textContainer.lineFragmentPadding = 0
        v.delegate = context.coordinator
        v.autocorrectionType = .default
        v.returnKeyType = .default
        v.accessibilityIdentifier = identifier
        v.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        v.setContentHuggingPriority(.defaultHigh, for: .vertical)
        return v
    }

    func updateUIView(_ v: LineTextView, context: Context) {
        let c = context.coordinator
        c.parent = self
        v.backspaceAtStart = actions.backspaceAtStart
        let w = weight ?? Fonts.textWeight(v.traitCollection)
        var attrs: [NSAttributedString.Key: Any] = [.font: Fonts.ui(size, w), .foregroundColor: color]
        if strike && !v.isFirstResponder {
            attrs[.strikethroughStyle] = NSUnderlineStyle.single.rawValue
        }
        v.typingAttributes = attrs
        let want = v.isFirstResponder ? text : (display ?? text)
        // Never type over the line being composed (marked text) or edited: the engine already has it.
        if v.markedTextRange == nil, v.text != want || v.lastAttrs != attrs.description {
            let sel = v.selectedRange
            v.attributedText = NSAttributedString(string: want, attributes: attrs)
            v.lastAttrs = attrs.description
            if v.isFirstResponder { v.selectedRange = NSRange(location: min(sel.location, want.utf16.count), length: 0) }
        }
        v.setAccessory(actions.accessory)
        if focused && !v.isFirstResponder {
            let at = text.utf16Offset(scalars: caret)
            DispatchQueue.main.async {
                guard v.window != nil, !v.isFirstResponder else { return }
                c.placing = true
                if v.text != text { v.text = text }
                _ = v.becomeFirstResponder()
                v.selectedRange = NSRange(location: min(at, v.text.utf16.count), length: 0)
                c.placing = false
            }
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: LineTextView, context: Context) -> CGSize? {
        let width = proposal.width ?? 300
        let fit = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
        return CGSize(width: width, height: max(ceil(fit.height), ceil(Fonts.ui(size, 400).lineHeight)))
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: LineEditor?
        var placing = false

        func textViewDidBeginEditing(_ v: UITextView) {
            guard let p = parent else { return }
            // Switch from the shown text (names for references) to the stored one.
            if v.text != p.text {
                v.text = p.text
                v.selectedRange = NSRange(location: v.text.utf16.count, length: 0)
            }
            LineTextView.current = v as? LineTextView
            if !placing && !p.focused {
                p.actions.began(p.text.scalarOffset(utf16: v.selectedRange.location))
            }
        }

        func textViewDidEndEditing(_ v: UITextView) {
            guard let p = parent else { return }
            // Ended for good (keyboard down), not handed to the next line.
            DispatchQueue.main.async {
                if let cur = LineTextView.current, cur.isFirstResponder { return }
                p.actions.ended()
            }
        }

        func textView(_ v: UITextView, shouldChangeTextIn range: NSRange, replacementText s: String) -> Bool {
            guard let p = parent else { return true }
            if s == "\n" {
                p.actions.returned(v.text.scalarOffset(utf16: range.location))
                return false
            }
            if s.contains("\n") {
                // ponytail: a pasted multi-line text stays one line; paste as nodes when asked for.
                v.replace(range, with: s.replacingOccurrences(of: "\r\n", with: " ").replacingOccurrences(of: "\n", with: " "))
                return false
            }
            return true
        }

        func textViewDidChange(_ v: UITextView) {
            guard let p = parent else { return }
            p.actions.changed(v.text, v.text.scalarOffset(utf16: v.selectedRange.location))
        }
    }
}

private extension UITextView {
    func replace(_ range: NSRange, with s: String) {
        guard let start = position(from: beginningOfDocument, offset: range.location),
              let end = position(from: start, offset: range.length),
              let r = textRange(from: start, to: end)
        else { return }
        replace(r, withText: s)
    }
}

final class LineTextView: UITextView {
    static weak var current: LineTextView?
    var backspaceAtStart: () -> Void = {}
    var lastAttrs = ""
    private var accessoryIds: [String] = []
    private var accessoryActions: [() -> Void] = []

    override func deleteBackward() {
        if selectedRange.location == 0 && selectedRange.length == 0 && markedTextRange == nil {
            backspaceAtStart()
            return
        }
        super.deleteBackward()
    }

    /// The keyboard bar: outline moves the phone has no keys for (Tab, Shift+Tab, ...).
    func setAccessory(_ items: [AccessoryItem]) {
        accessoryActions = items.map(\.action)
        let ids = items.map(\.id)
        guard ids != accessoryIds else { return }
        accessoryIds = ids
        guard !items.isEmpty else {
            inputAccessoryView = nil
            return
        }
        let bar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
        var buttons: [UIBarButtonItem] = []
        for (i, it) in items.enumerated() {
            let b = UIBarButtonItem(image: UIImage(systemName: it.symbol), style: .plain, target: self, action: #selector(tapAccessory(_:)))
            b.tag = i
            b.accessibilityIdentifier = "kb.\(it.id)"
            b.accessibilityLabel = it.label
            buttons.append(b)
            buttons.append(UIBarButtonItem(barButtonSystemItem: .flexibleSpace, target: nil, action: nil))
        }
        buttons.removeLast()
        bar.items = buttons
        bar.sizeToFit()
        inputAccessoryView = bar
        if isFirstResponder { reloadInputViews() }
    }

    @objc private func tapAccessory(_ b: UIBarButtonItem) {
        if accessoryActions.indices.contains(b.tag) { accessoryActions[b.tag]() }
    }
}
