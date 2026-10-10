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
}

/// One editable line: a UITextView that sends every change to the engine at once (it never holds
/// text the engine does not have), turns Return into a new line and Backspace at the start into a
/// merge with the line above. It takes the keyboard when the engine says the caret is in it.
struct LineEditor: UIViewRepresentable {
    let text: String
    /// While not editing, the text is drawn formatted (marks applied, references as these names).
    let refs: [String: String]
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
        let attrs: [NSAttributedString.Key: Any] = [.font: Fonts.ui(size, w), .foregroundColor: color]
        v.typingAttributes = attrs
        v.plain = NSAttributedString(string: text, attributes: attrs)
        // Never type over the line being composed (marked text) or edited: the engine already has it.
        if v.markedTextRange == nil {
            if v.isFirstResponder {
                if v.text != text || v.lastAttrs != attrs.description {
                    let sel = v.selectedRange
                    v.attributedText = v.plain
                    v.selectedRange = NSRange(location: min(sel.location, text.utf16.count), length: 0)
                }
            } else {
                let shown = NSMutableAttributedString(attributedString: Format.styled(text, refs: refs, size: size, weight: w, color: color))
                if strike { shown.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: NSRange(location: 0, length: shown.length)) }
                if !(v.attributedText?.isEqual(to: shown) ?? false) { v.attributedText = shown }
            }
            v.lastAttrs = attrs.description
        }
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
            // Switch from the formatted text to the stored one (marks and reference tokens).
            if let lv = v as? LineTextView, let plain = lv.plain {
                let same = v.text == p.text
                let sel = v.selectedRange
                v.attributedText = plain
                v.selectedRange = same ? sel : NSRange(location: v.text.utf16.count, length: 0)
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
    /// The stored text with plain attributes (what editing shows).
    var plain: NSAttributedString?
    override func deleteBackward() {
        if selectedRange.location == 0 && selectedRange.length == 0 && markedTextRange == nil {
            backspaceAtStart()
            return
        }
        super.deleteBackward()
    }
}
