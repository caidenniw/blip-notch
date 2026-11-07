//! Tolerant HTML parser for `ui/index.html` and the fixed `innerHTML` snippets in `ui/app.js`.
use super::{Dom, NodeId};

const VOID: &[&str] = &["img", "input", "br", "meta", "link", "hr", "source"];
/// Tags whose children `parse_document` drops (`<head>` keeps only its `<title>` text).
const SKIP: &[&str] = &["script", "style", "head"];

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let ent = end.map(|e| &rest[1..e]);
        let rep = match ent {
            Some("amp") => Some('&'),
            Some("lt") => Some('<'),
            Some("gt") => Some('>'),
            Some("quot") => Some('"'),
            Some("apos") => Some('\''),
            Some("nbsp") => Some('\u{a0}'),
            Some(e) if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            Some(e) if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match rep {
            Some(c) => {
                out.push(c);
                rest = &rest[end.unwrap() + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct P<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> P<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }
    fn skip_ws(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start().len();
    }
    fn name(&mut self) -> &'a str {
        let r = self.rest();
        let n = r
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/' || c == '=')
            .unwrap_or(r.len());
        self.i += n;
        &r[..n]
    }
}

/// Parses `src` as children of `parent`.
pub fn parse_fragment(dom: &mut Dom, parent: NodeId, src: &str, _svg: bool) {
    let mut p = P { s: src, i: 0 };
    parse_into(dom, &mut p, parent, false);
}

pub fn parse_document(src: &str) -> Dom {
    let mut dom = Dom::new();
    // Take <title> text, then parse the <body> element's attributes and contents.
    if let (Some(a), Some(b)) = (src.find("<title>"), src.find("</title>")) {
        dom.title = decode_entities(&src[a + 7..b]);
    }
    let start = src.find("<body").expect("index.html has <body>");
    let mut p = P { s: src, i: start + 5 };
    let body = dom.body;
    parse_attrs(&mut dom, &mut p, body);
    parse_into(&mut dom, &mut p, body, true);
    dom.mutations.clear();
    dom
}

fn parse_attrs(dom: &mut Dom, p: &mut P, el: NodeId) -> bool {
    loop {
        p.skip_ws();
        if p.rest().starts_with("/>") {
            p.i += 2;
            return true;
        }
        if p.rest().starts_with('>') || p.eof() {
            p.i += 1;
            return false;
        }
        let name = p.name().to_ascii_lowercase();
        if name.is_empty() {
            p.i += 1;
            continue;
        }
        p.skip_ws();
        let mut value = String::new();
        if p.rest().starts_with('=') {
            p.i += 1;
            p.skip_ws();
            let r = p.rest();
            if let Some(q) = r.chars().next().filter(|&c| c == '"' || c == '\'') {
                let end = r[1..].find(q).map(|e| e + 1).unwrap_or(r.len());
                value = decode_entities(&r[1..end]);
                p.i += (end + 1).min(r.len());
            } else {
                let end = r.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(r.len());
                value = decode_entities(&r[..end]);
                p.i += end;
            }
        }
        dom.set_attr(el, &name, &value);
    }
}

/// Parses until `</tag>` of `parent` (or EOF). `doc` = drop skipped elements.
fn parse_into(dom: &mut Dom, p: &mut P, parent: NodeId, doc: bool) {
    let mut text = String::new();
    let flush = |dom: &mut Dom, text: &mut String| {
        if !text.is_empty() {
            // Whitespace-only runs between tags carry no rendering in this UI (all such gaps
            // sit between block/flex items); drop them so they don't become anonymous boxes.
            if !text.trim().is_empty() {
                let t = dom.create_text(&decode_entities(text));
                dom.append_child(parent, t);
            }
            text.clear();
        }
    };
    while !p.eof() {
        let r = p.rest();
        if !r.starts_with('<') {
            let n = r.find('<').unwrap_or(r.len());
            text.push_str(&r[..n]);
            p.i += n;
            continue;
        }
        if r.starts_with("<!--") {
            let e = r.find("-->").map(|e| e + 3).unwrap_or(r.len());
            p.i += e;
            continue;
        }
        if r.starts_with("<!") {
            let e = r.find('>').map(|e| e + 1).unwrap_or(r.len());
            p.i += e;
            continue;
        }
        if r.starts_with("</") {
            flush(dom, &mut text);
            let e = r.find('>').map(|e| e + 1).unwrap_or(r.len());
            p.i += e;
            return;
        }
        flush(dom, &mut text);
        p.i += 1;
        let tag = p.name().to_ascii_lowercase();
        if doc && SKIP.contains(&tag.as_str()) {
            let close = format!("</{tag}>");
            let e = p.rest().find(&close).map(|e| e + close.len()).unwrap_or(p.rest().len());
            p.i += e;
            continue;
        }
        let el = dom.create_element(&tag);
        dom.append_child(parent, el);
        let self_closed = parse_attrs(dom, p, el);
        if !self_closed && !VOID.contains(&tag.as_str()) {
            parse_into(dom, p, el, doc);
        }
    }
    flush(dom, &mut text);
}

#[cfg(test)]
mod tests {
    #[test]
    fn entities() {
        assert_eq!(super::decode_entities("a &amp; b &lt;&#8212;&#x41;&zz;"), "a & b <\u{2014}A&zz;");
    }
}
