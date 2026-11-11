//! Stylesheet parser for the grammar used by `ui/style.css`.
use super::select::{parse_selector_list, SelectorList};

#[derive(Clone, Debug)]
pub struct Decl {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub selectors: SelectorList,
    pub decls: Vec<Decl>,
    /// Inside `@media (prefers-reduced-motion: reduce)`.
    pub reduced_motion: bool,
}

#[derive(Clone, Debug)]
pub struct Keyframes {
    pub name: String,
    /// `(offset 0..=1, declarations)` sorted by offset.
    pub frames: Vec<(f32, Vec<Decl>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub rules: Vec<Rule>,
    pub keyframes: Vec<Keyframes>,
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut r = s;
    while let Some(i) = r.find("/*") {
        out.push_str(&r[..i]);
        r = match r[i + 2..].find("*/") {
            Some(e) => &r[i + 2 + e + 2..],
            None => "",
        };
    }
    out.push_str(r);
    out
}

/// Index of the `}` matching the `{` at `open`.
fn block_end(s: &str, open: usize) -> usize {
    let mut depth = 0;
    let mut quote = None;
    for (i, c) in s[open..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '{') => depth += 1,
            (None, '}') => {
                depth -= 1;
                if depth == 0 {
                    return open + i;
                }
            }
            _ => {}
        }
    }
    s.len()
}

pub fn parse_decls(s: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for d in super::value::split_top(s, ';') {
        let Some((n, v)) = d.split_once(':') else { continue };
        let name = n.trim().to_string();
        let mut value = v.trim().to_string();
        let mut important = false;
        if let Some(x) = value.strip_suffix("!important") {
            value = x.trim().to_string();
            important = true;
        }
        if name.is_empty() || value.is_empty() {
            continue;
        }
        out.push(Decl { name: if name.starts_with("--") { name } else { name.to_ascii_lowercase() }, value, important });
    }
    out
}

pub fn parse_sheet(src: &str) -> Sheet {
    let src = strip_comments(src);
    let mut sheet = Sheet::default();
    parse_block(&src, &mut sheet, false);
    sheet
}

fn parse_block(s: &str, sheet: &mut Sheet, reduced: bool) {
    let mut i = 0;
    while let Some(o) = s[i..].find('{').map(|o| o + i) {
        let head = s[i..o].trim();
        let end = block_end(s, o);
        let body = &s[o + 1..end.min(s.len())];
        if let Some(name) = head.strip_prefix("@keyframes") {
            let mut frames = Vec::new();
            let mut j = 0;
            while let Some(fo) = body[j..].find('{').map(|x| x + j) {
                let sel = body[j..fo].trim();
                let fe = block_end(body, fo);
                let decls = parse_decls(&body[fo + 1..fe]);
                for p in sel.split(',') {
                    let off = match p.trim() {
                        "from" => 0.0,
                        "to" => 1.0,
                        x => x.trim_end_matches('%').trim().parse::<f32>().unwrap_or(0.0) / 100.0,
                    };
                    frames.push((off, decls.clone()));
                }
                j = fe + 1;
            }
            frames.sort_by(|a, b| a.0.total_cmp(&b.0));
            sheet.keyframes.push(Keyframes { name: name.trim().to_string(), frames });
        } else if head.starts_with("@media") {
            if head.contains("prefers-reduced-motion") {
                parse_block(body, sheet, true);
            }
        } else if let Some(selectors) = parse_selector_list(head) {
            sheet.rules.push(Rule { selectors, decls: parse_decls(body), reduced_motion: reduced });
        } else {
            crate::conf::log(&format!("css: unsupported selector {head}"));
        }
        i = end + 1;
        if i >= s.len() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_whole_sheet() {
        let s = super::parse_sheet(include_str!("../../ui/style.css"));
        assert!(s.rules.iter().any(|r| r.reduced_motion && r.decls.iter().all(|d| d.important)));
        let pulse = s.keyframes.iter().find(|k| k.name == "pulse").unwrap();
        assert_eq!(pulse.frames.len(), 3);
    }
}
