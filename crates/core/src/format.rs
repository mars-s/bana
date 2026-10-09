//! Inline formatting inside a node's text, stored as light marks (our own format, close to
//! Markdown): `**bold**`, `*italic*`, `__underline__`, `==highlight==`, `` `code` ``. A mark with
//! no partner later in the text is shown as typed.

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
}

/// Marks in matching order: longer tokens first so `**` is not read as two `*`.
const ORDER: [Mark; 5] = [Mark::Bold, Mark::Underline, Mark::Highlight, Mark::Code, Mark::Italic];

fn mark_at(s: &str, i: usize) -> Option<Mark> {
    let rest = s.get(i..)?;
    ORDER.into_iter().find(|m| rest.starts_with(m.token()))
}

/// Splits text into styled spans.
pub fn spans(s: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut cur = Span::default();
    let mut i = 0;
    while i < s.len() {
        let mark = if cur.code { s.get(i..).filter(|r| r.starts_with('`')).map(|_| Mark::Code) } else { mark_at(s, i) };
        if let Some(m) = mark {
            let tok = m.token();
            let open = is_open(&cur, m);
            let closes_later = s.get(i + tok.len()..).is_some_and(|r| r.contains(tok));
            if open || closes_later {
                if !cur.text.is_empty() {
                    out.push(cur.clone());
                    cur.text.clear();
                }
                set(&mut cur, m, !open);
                i += tok.len();
                continue;
            }
        }
        let Some(c) = s.get(i..).and_then(|r| r.chars().next()) else { break };
        cur.text.push(c);
        i += c.len_utf8();
    }
    if !cur.text.is_empty() {
        out.push(cur);
    }
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

/// The text without marks (search, titles, exports that want plain text).
pub fn plain(s: &str) -> String {
    if !s.contains(['*', '_', '=', '`']) {
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
