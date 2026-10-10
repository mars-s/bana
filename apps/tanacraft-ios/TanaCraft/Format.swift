import SwiftUI
import UIKit

/// A line's stored text drawn as people read it: inline marks (`**bold**`, `*italic*`,
/// `__underline__`, `==highlight==`, `` `code` ``) applied and hidden, inline references
/// (`[[id:42]]`) shown as names in link colour, and web addresses in link colour. Mirrors
/// `crates/core/src/format.rs`: a mark with no partner later in the text is shown as typed.
enum Format {
    private static let marks = ["**", "__", "==", "`", "*"]

    static func styled(_ text: String, refs: [String: String], size: CGFloat, weight: CGFloat, color: UIColor) -> NSAttributedString {
        let out = NSMutableAttributedString()
        var active = Set<String>()
        var i = text.startIndex
        var run = ""

        func attrs() -> [NSAttributedString.Key: Any] {
            var a: [NSAttributedString.Key: Any] = [.foregroundColor: color]
            var w = weight
            if active.contains("**") { w = T.fwBold }
            var font = Fonts.ui(size, w)
            if active.contains("*"), let d = font.fontDescriptor.withSymbolicTraits(.traitItalic) {
                font = UIFont(descriptor: d, size: size)
            }
            if active.contains("`") {
                font = .monospacedSystemFont(ofSize: size * T.fsCode / T.fsBody, weight: .regular)
                a[.foregroundColor] = UIColor(T.cCodeText)
                a[.backgroundColor] = UIColor(T.cCodeBg)
            }
            if active.contains("__") { a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            if active.contains("==") { a[.backgroundColor] = UIColor(T.cHighlight) }
            a[.font] = font
            return a
        }
        func flush() {
            guard !run.isEmpty else { return }
            out.append(link(run, attrs()))
            run = ""
        }

        while i < text.endIndex {
            let rest = text[i...]
            if rest.hasPrefix("[[id:"), let close = rest.range(of: "]]") {
                let id = String(rest[rest.index(rest.startIndex, offsetBy: 5)..<close.lowerBound])
                if let name = refs[id] {
                    flush()
                    var a = attrs()
                    a[.foregroundColor] = UIColor(T.cLink)
                    out.append(NSAttributedString(string: name, attributes: a))
                    i = close.upperBound
                    continue
                }
            }
            if let m = marks.first(where: { rest.hasPrefix($0) }), active.contains("`") ? m == "`" : true {
                let after = rest[rest.index(rest.startIndex, offsetBy: m.count)...]
                if active.contains(m) || after.contains(m) {
                    flush()
                    if active.contains(m) { active.remove(m) } else { active.insert(m) }
                    i = after.startIndex
                    continue
                }
            }
            run.append(text[i])
            i = text.index(after: i)
        }
        flush()
        return out
    }

    /// Web addresses inside a run take the link colour.
    private static func link(_ s: String, _ a: [NSAttributedString.Key: Any]) -> NSAttributedString {
        let out = NSMutableAttributedString(string: s, attributes: a)
        guard s.contains("http") else { return out }
        let ns = s as NSString
        for word in s.split(separator: " ") where word.hasPrefix("http://") || word.hasPrefix("https://") {
            let r = ns.range(of: String(word))
            if r.location != NSNotFound { out.addAttribute(.foregroundColor, value: UIColor(T.cLink), range: r) }
        }
        return out
    }
}
