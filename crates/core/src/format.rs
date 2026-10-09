//! Inline formatting inside a node's text, stored as light marks (our own format, close to
//! Markdown): `**bold**`, `*italic*`, `__underline__`, `==highlight==`, `` `code` ``. A mark with
//! no partner later in the text is shown as typed.
//!
//! Inline references are `[[id:42]]` (shown as the target's name); `http(s)://` words are links.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Highlight,
    Code,
}

impl Mark {
    pub fn token(self) -> &'static str {
        match self {
            Mark::Bold => "**",
            Mark::Italic => "*",
            Mark::Underline => "__",
            Mark::Highlight => "==",
            Mark::Code => "`",
        }
    }

    pub fn parse(s: &str) -> Option<Mark> {
        match s {
            "bold" => Some(Mark::Bold),
            "italic" => Some(Mark::Italic),
            "underline" => Some(Mark::Underline),
            "highlight" => Some(Mark::Highlight),
            "code" => Some(Mark::Code),
            _ => None,
        }
    }
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub highlight: bool,
    pub code: bool,
    /// An inline reference to this node (`text` is empty; show the target's name).
    pub node: Option<u64>,
    /// A link (`text` is the URL).
    pub url: Option<String>,
}

/// The inline reference token for node `id`.
pub fn ref_token(id: u64) -> String {
    format!("[[id:{id}]]")
}

/// An inline reference at byte `i`: (node id, token length).
fn ref_at(s: &str, i: usize) -> Option<(u64, usize)> {
    let rest = s.get(i..)?.strip_prefix("[[id:")?;
    let end = rest.find("]]")?;
    let id = rest.get(..end)?.parse().ok()?;
    Some((id, 5 + end + 2))
}

/// Node ids referenced inline in `s`.
pub fn refs(s: &str) -> Vec<u64> {
    spans(s).into_iter().filter_map(|sp| sp.node).collect()
}

/// Marks in matching order: longer tokens first so `**` is not read as two `*`.
const ORDER: [Mark; 5] = [Mark::Bold, Mark::Underline, Mark::Highlight, Mark::Code, Mark::Italic];

fn mark_at(s: &str, i: usize) -> Option<Mark> {
    let rest = s.get(i..)?;
    ORDER.into_iter().find(|m| rest.starts_with(m.token()))
}

/// Splits text into styled spans.
pub fn spans(s: &str) -> Vec<Span> {
    raw_spans(s).into_iter().filter(|(_, marker)| !marker).map(|(sp, _)| sp).collect()
}

/// Like [`spans`], but also returns the marks and reference tokens themselves (flagged `true`), so
/// the pieces join back into `s` exactly (the editor styles raw text with them).
pub fn raw_spans(s: &str) -> Vec<(Span, bool)> {
    let mut out: Vec<(Span, bool)> = Vec::new();
    let mut cur = Span::default();
    let mut i = 0;
    let flush = |out: &mut Vec<(Span, bool)>, cur: &mut Span| {
        if !cur.text.is_empty() {
            out.push((cur.clone(), false));
            cur.text.clear();
        }
    };
    while i < s.len() {
        if !cur.code
            && let Some((id, len)) = ref_at(s, i)
        {
            flush(&mut out, &mut cur);
            out.push((Span { text: String::new(), node: Some(id), ..cur.clone() }, false));
            out.push((Span { text: s.get(i..i + len).unwrap_or("").to_string(), node: Some(id), ..Span::default() }, true));
            i += len;
            continue;
        }
        let at_word = i == 0 || s.get(..i).and_then(|b| b.chars().last()).is_some_and(char::is_whitespace);
        if !cur.code && at_word && s.get(i..).is_some_and(|r| r.starts_with("http://") || r.starts_with("https://")) {
            let rest = s.get(i..).unwrap_or("");
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let url = rest.get(..len).unwrap_or("").to_string();
            flush(&mut out, &mut cur);
            out.push((Span { text: url.clone(), url: Some(url), ..cur.clone() }, false));
            i += len;
            continue;
        }
        let mark = if cur.code { s.get(i..).filter(|r| r.starts_with('`')).map(|_| Mark::Code) } else { mark_at(s, i) };
        if let Some(m) = mark {
            let tok = m.token();
            let open = is_open(&cur, m);
            let closes_later = s.get(i + tok.len()..).is_some_and(|r| r.contains(tok));
            if open || closes_later {
                flush(&mut out, &mut cur);
                out.push((Span { text: tok.to_string(), ..Span::default() }, true));
                set(&mut cur, m, !open);
                i += tok.len();
                continue;
            }
        }
        let Some(c) = s.get(i..).and_then(|r| r.chars().next()) else { break };
        cur.text.push(c);
        i += c.len_utf8();
    }
    flush(&mut out, &mut cur);
    out
}

fn is_open(s: &Span, m: Mark) -> bool {
    match m {
        Mark::Bold => s.bold,
        Mark::Italic => s.italic,
        Mark::Underline => s.underline,
        Mark::Highlight => s.highlight,
        Mark::Code => s.code,
    }
}

fn set(s: &mut Span, m: Mark, on: bool) {
    match m {
        Mark::Bold => s.bold = on,
        Mark::Italic => s.italic = on,
        Mark::Underline => s.underline = on,
        Mark::Highlight => s.highlight = on,
        Mark::Code => s.code = on,
    }
}

/// The raw char offset for a char offset in the shown text (marks hidden). `None` when the text
/// has inline references or links, whose shown text differs from the raw one.
pub fn shown_to_raw(raw: &str, shown: usize) -> Option<usize> {
    let pieces = raw_spans(raw);
    if pieces.iter().any(|(sp, _)| sp.node.is_some() || sp.url.is_some()) {
        return None;
    }
    let (mut seen, mut at) = (0usize, 0usize);
    for (sp, marker) in pieces {
        let n = sp.text.chars().count();
        if !marker {
            if shown <= seen + n {
                return Some(at + shown - seen);
            }
            seen += n;
        }
        at += n;
    }
    Some(at)
}

/// The text without marks (search, titles, exports that want plain text).
pub fn plain(s: &str) -> String {
    if !s.contains(['*', '_', '=', '`', '[']) {
        return s.to_string();
    }
    spans(s).into_iter().map(|sp| sp.text).collect()
}

/// Whether the text has any formatting.
pub fn is_formatted(s: &str) -> bool {
    spans(s).iter().any(|sp| sp.bold || sp.italic || sp.underline || sp.highlight || sp.code)
}

/// Toggles `mark` around chars `start..end` of the raw text (the whole text when the range is
/// empty). Returns the new text and the new selection.
pub fn toggle(s: &str, start: usize, end: usize, mark: Mark) -> (String, usize, usize) {
    let chars: Vec<char> = s.chars().collect();
    let (mut a, mut b) = (start.min(end).min(chars.len()), start.max(end).min(chars.len()));
    if a == b {
        a = 0;
        b = chars.len();
    }
    toggle_range(&chars, a, b, mark)
}

/// With a collapsed caret at `at`: an empty mark pair goes in and the caret sits inside it, so what
/// is typed next carries the format (spec 02 §10). Inside an empty pair, the pair goes away.
pub fn toggle_at(s: &str, at: usize, mark: Mark) -> (String, usize) {
    let chars: Vec<char> = s.chars().collect();
    let at = at.min(chars.len());
    let tok: Vec<char> = mark.token().chars().collect();
    let n = tok.len();
    let mut out: Vec<char> = chars[..at].to_vec();
    if at >= n && chars.get(at - n..at) == Some(&tok[..]) && chars.get(at..at + n) == Some(&tok[..]) {
        out.truncate(at - n);
        out.extend(&chars[at + n..]);
        return (out.into_iter().collect(), at - n);
    }
    out.extend(&tok);
    out.extend(&tok);
    out.extend(&chars[at..]);
    (out.into_iter().collect(), at + n)
}

fn toggle_range(chars: &[char], a: usize, b: usize, mark: Mark) -> (String, usize, usize) {
    let tok: Vec<char> = mark.token().chars().collect();
    let n = tok.len();
    let before = a >= n && chars.get(a - n..a) == Some(&tok[..]);
    let after = chars.get(b..b + n) == Some(&tok[..]);
    let inner_wrapped = b >= a + 2 * n && chars.get(a..a + n) == Some(&tok[..]) && chars.get(b - n..b) == Some(&tok[..]);
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + 2 * n);
    if before && after {
        out.extend(&chars[..a - n]);
        out.extend(&chars[a..b]);
        out.extend(&chars[b + n..]);
        return (out.into_iter().collect(), a - n, b - n);
    }
    if inner_wrapped {
        out.extend(&chars[..a]);
        out.extend(&chars[a + n..b - n]);
        out.extend(&chars[b..]);
        return (out.into_iter().collect(), a, b - 2 * n);
    }
    out.extend(&chars[..a]);
    out.extend(&tok);
    out.extend(&chars[a..b]);
    out.extend(&tok);
    out.extend(&chars[b..]);
    (out.into_iter().collect(), a + n, b + n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_spans_join_back_into_the_text() {
        let s = "a **bold** [[id:3]] https://x.org `c`";
        let joined: String = raw_spans(s).into_iter().map(|(sp, _)| sp.text).collect();
        assert_eq!(joined, s);
    }

    #[test]
    fn shown_offsets_map_to_raw_ones() {
        assert_eq!(shown_to_raw("ab **cd** e", 4), Some(6));
        assert_eq!(shown_to_raw("plain", 3), Some(3));
        assert_eq!(shown_to_raw("see [[id:4]]", 2), None);
    }

    #[test]
    fn collapsed_caret_opens_an_empty_pair() {
        let (s, c) = toggle_at("ab", 1, Mark::Bold);
        assert_eq!((s.as_str(), c), ("a****b", 3));
        assert_eq!(toggle_at(&s, c, Mark::Bold), ("ab".to_string(), 1));
    }

    #[test]
    fn spans_and_plain() {
        let s = "a **bold** and *it* ==hi== `c*o*de` __u__";
        assert_eq!(plain(s), "a bold and it hi c*o*de u");
        let sp = spans(s);
        assert!(sp.iter().any(|x| x.text == "bold" && x.bold));
        assert!(sp.iter().any(|x| x.text == "it" && x.italic));
        assert!(sp.iter().any(|x| x.text == "hi" && x.highlight));
        assert!(sp.iter().any(|x| x.text == "c*o*de" && x.code && !x.italic));
        assert!(sp.iter().any(|x| x.text == "u" && x.underline));
        // Lone marks stay literal.
        assert_eq!(plain("2 * 3 = 6"), "2 * 3 = 6");
        assert_eq!(plain("snake_case"), "snake_case");
        assert!(!is_formatted("plain text"));
        let sp = spans("see [[id:42]] and https://example.org now");
        assert_eq!(sp.iter().find_map(|x| x.node), Some(42));
        assert_eq!(sp.iter().find_map(|x| x.url.clone()).as_deref(), Some("https://example.org"));
        assert_eq!(refs("a [[id:1]] b [[id:2]]"), vec![1, 2]);
        assert_eq!(plain("a [[id:1]] b"), "a  b");
        assert_eq!(plain("[[not a ref]]"), "[[not a ref]]");
    }

    #[test]
    fn toggle_wraps_and_unwraps() {
        let (s, a, b) = toggle("make this bold", 5, 9, Mark::Bold);
        assert_eq!((s.as_str(), a, b), ("make **this** bold", 7, 11));
        let (s, a, b) = toggle(&s, a, b, Mark::Bold);
        assert_eq!((s.as_str(), a, b), ("make this bold", 5, 9));
        let (s, _, _) = toggle("whole", 2, 2, Mark::Highlight);
        assert_eq!(s, "==whole==");
        let (s, _, _) = toggle("==whole==", 0, 9, Mark::Highlight);
        assert_eq!(s, "whole");
        // Out-of-range input is clamped, never panics.
        let (s, _, _) = toggle("ab", 99, 5, Mark::Code);
        assert_eq!(s, "`ab`");
    }
}
