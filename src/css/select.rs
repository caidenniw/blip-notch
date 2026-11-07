//! Selectors: compound + descendant combinator + optional pseudo-element.
use crate::dom::{self, Dom, NodeId};

#[derive(Clone, Debug, PartialEq)]
pub enum Pseudo {
    Hover,
    Active,
    Focus,
    FocusVisible,
    FocusWithin,
    Empty,
    FirstChild,
    LastChild,
    NthChild(usize),
    Root,
    Not(SelectorList),
    Where(SelectorList),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PseudoEl {
    Before,
    After,
    Placeholder,
    Scrollbar,
    ScrollbarThumb,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Compound {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// `[name]` or `[name="value"]`.
    pub attrs: Vec<(String, Option<String>)>,
    pub pseudos: Vec<Pseudo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    /// Left to right; each step is a descendant of the previous.
    pub parts: Vec<Compound>,
    pub pe: Option<PseudoEl>,
    pub spec: (u16, u16, u16),
}

pub type SelectorList = Vec<Selector>;

pub fn parse_selector_list(s: &str) -> Option<SelectorList> {
    crate::css::value::split_top(s, ',').into_iter().map(parse_selector).collect()
}

fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    if s.is_empty() || s.contains(['>', '+', '~']) && !s.contains('(') {
        return None;
    }
    let mut parts = Vec::new();
    let mut pe = None;
    // Split on whitespace outside brackets/parens.
    let mut tokens = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' | ']' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    tokens.push(&s[st..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(st) = start {
        tokens.push(&s[st..]);
    }
    let n = tokens.len();
    for (i, t) in tokens.into_iter().enumerate() {
        let (c, p) = parse_compound(t)?;
        if p.is_some() && i + 1 != n {
            return None;
        }
        pe = pe.or(p);
        parts.push(c);
    }
    let mut spec = (0, 0, 0);
    for c in &parts {
        let s = compound_spec(c);
        spec = (spec.0 + s.0, spec.1 + s.1, spec.2 + s.2);
    }
    if pe.is_some() {
        spec.2 += 1;
    }
    Some(Selector { parts, pe, spec })
}

fn compound_spec(c: &Compound) -> (u16, u16, u16) {
    let mut s = (c.id.is_some() as u16, (c.classes.len() + c.attrs.len()) as u16, c.tag.is_some() as u16);
    for p in &c.pseudos {
        match p {
            Pseudo::Where(_) => {}
            Pseudo::Not(l) => {
                let m = l.iter().map(|x| x.spec).max().unwrap_or((0, 0, 0));
                s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
            }
            _ => s.1 += 1,
        }
    }
    s
}

fn ident_end(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_')).unwrap_or(s.len())
}

fn parse_compound(t: &str) -> Option<(Compound, Option<PseudoEl>)> {
    let mut c = Compound::default();
    let mut pe = None;
    let mut r = t;
    if r.starts_with('*') {
        r = &r[1..];
    } else if r.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
        let e = ident_end(r);
        c.tag = Some(r[..e].to_ascii_lowercase());
        r = &r[e..];
    }
    while !r.is_empty() {
        if let Some(x) = r.strip_prefix('#') {
            let e = ident_end(x);
            c.id = Some(x[..e].to_string());
            r = &x[e..];
        } else if let Some(x) = r.strip_prefix('.') {
            let e = ident_end(x);
            c.classes.push(x[..e].to_string());
            r = &x[e..];
        } else if let Some(x) = r.strip_prefix('[') {
            let e = x.find(']')?;
            let inner = &x[..e];
            match inner.split_once('=') {
                Some((n, v)) => c.attrs.push((n.trim().to_string(), Some(v.trim().trim_matches(['"', '\'']).to_string()))),
                None => c.attrs.push((inner.trim().to_string(), None)),
            }
            r = &x[e + 1..];
        } else if let Some(x) = r.strip_prefix("::") {
            let e = ident_end(x);
            pe = Some(match &x[..e] {
                "before" => PseudoEl::Before,
                "after" => PseudoEl::After,
                "placeholder" => PseudoEl::Placeholder,
                "-webkit-scrollbar" => PseudoEl::Scrollbar,
                "-webkit-scrollbar-thumb" => PseudoEl::ScrollbarThumb,
                _ => return None,
            });
            r = &x[e..];
        } else if let Some(x) = r.strip_prefix(':') {
            let e = ident_end(x);
            let name = &x[..e];
            let mut rest = &x[e..];
            let mut arg = None;
            if rest.starts_with('(') {
                let mut depth = 0;
                let mut end = 0;
                for (i, ch) in rest.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                arg = Some(&rest[1..end]);
                rest = &rest[end + 1..];
            }
            c.pseudos.push(match (name, arg) {
                ("hover", None) => Pseudo::Hover,
                ("active", None) => Pseudo::Active,
                ("focus", None) => Pseudo::Focus,
                ("focus-visible", None) => Pseudo::FocusVisible,
                ("focus-within", None) => Pseudo::FocusWithin,
                ("empty", None) => Pseudo::Empty,
                ("first-child", None) => Pseudo::FirstChild,
                ("last-child", None) => Pseudo::LastChild,
                ("root", None) => Pseudo::Root,
                ("nth-child", Some(a)) => Pseudo::NthChild(a.trim().parse().ok()?),
                ("not", Some(a)) => Pseudo::Not(parse_selector_list(a)?),
                ("where", Some(a)) => Pseudo::Where(parse_selector_list(a)?),
                // `:before`/`:after` legacy single-colon form.
                ("before", None) => {
                    pe = Some(PseudoEl::Before);
                    r = rest;
                    continue;
                }
                ("after", None) => {
                    pe = Some(PseudoEl::After);
                    r = rest;
                    continue;
                }
                _ => return None,
            });
            r = rest;
        } else {
            return None;
        }
    }
    Some((c, pe))
}

pub fn matches_compound(d: &Dom, n: NodeId, c: &Compound) -> bool {
    let Some(e) = d.elem(n) else { return false };
    if let Some(t) = &c.tag {
        if *t != e.tag {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if d.attr(n, "id") != Some(id.as_str()) {
            return false;
        }
    }
    if !c.classes.iter().all(|k| e.classes.iter().any(|x| x == k)) {
        return false;
    }
    for (name, v) in &c.attrs {
        match (d.attr(n, name), v) {
            (None, _) => return false,
            (Some(a), Some(v)) if a != v => return false,
            _ => {}
        }
    }
    let st = d.state(n);
    c.pseudos.iter().all(|p| match p {
        Pseudo::Hover => st & dom::HOVER != 0,
        Pseudo::Active => st & dom::ACTIVE != 0,
        Pseudo::Focus => st & dom::FOCUS != 0,
        Pseudo::FocusVisible => st & dom::FOCUS_VISIBLE != 0,
        Pseudo::FocusWithin => st & (dom::FOCUS | dom::FOCUS_WITHIN) != 0,
        Pseudo::Empty => d.children(n).iter().all(|&k| d.text(k).is_some_and(str::is_empty)),
        Pseudo::FirstChild => sibling_index(d, n).0 == 1,
        Pseudo::LastChild => {
            let (i, total) = sibling_index(d, n);
            i == total
        }
        Pseudo::NthChild(k) => sibling_index(d, n).0 == *k,
        Pseudo::Root => n == d.html,
        Pseudo::Not(l) => !matches_list(d, n, l),
        Pseudo::Where(l) => matches_list(d, n, l),
    })
}

/// (1-based index among element siblings, element sibling count).
fn sibling_index(d: &Dom, n: NodeId) -> (usize, usize) {
    let Some(p) = d.parent(n) else { return (1, 1) };
    let mut idx = 0;
    let mut total = 0;
    for &k in d.children(p) {
        if d.is_element(k) {
            total += 1;
            if k == n {
                idx = total;
            }
        }
    }
    (idx, total)
}

/// Matches ignoring the pseudo-element part.
pub fn matches_selector(d: &Dom, n: NodeId, s: &Selector) -> bool {
    let Some((last, rest)) = s.parts.split_last() else { return false };
    if !matches_compound(d, n, last) {
        return false;
    }
    let mut cur = n;
    'outer: for c in rest.iter().rev() {
        while let Some(p) = d.parent(cur) {
            cur = p;
            if matches_compound(d, cur, c) {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// Element-matching (selectors with a pseudo-element never match the element itself).
pub fn matches_list(d: &Dom, n: NodeId, l: &SelectorList) -> bool {
    l.iter().any(|s| s.pe.is_none() && matches_selector(d, n, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specificity() {
        let s = parse_selector_list("body:not([data-state=\"idle\"]) .eyes").unwrap();
        assert_eq!(s[0].spec, (0, 2, 1));
        let s = parse_selector_list(".eyes.mini").unwrap();
        assert_eq!(s[0].spec, (0, 2, 0));
        let s = parse_selector_list("body:where(:not([data-state=\"open\"]):not([data-state=\"hud\"])) .row").unwrap();
        assert_eq!(s[0].spec, (0, 1, 1));
        let s = parse_selector_list("#island::before").unwrap();
        assert_eq!((s[0].spec, s[0].pe), ((1, 0, 1), Some(PseudoEl::Before)));
    }

    #[test]
    fn matching() {
        let d = Dom::from_index();
        assert_eq!(d.query_str(d.html, "#mascot .eye:first-child", true).len(), 1);
        assert_eq!(d.query_str(d.html, ".bars i:nth-child(3)", true).len(), 1);
        assert_eq!(d.query_str(d.html, "#media button[data-cmd]", true).len(), 3);
        assert_eq!(d.query_str(d.html, ".mini-art:not([src])", true).len(), 1);
        assert_eq!(d.query_str(d.html, "#detail:empty", true).len(), 1);
        assert_eq!(d.query_str(d.html, "body[data-state=\"idle\"] .row", true).len(), 1);
    }
}
