//! DOM tree for the original `ui/index.html`.
//!
//! Node ids are never reused: a freed node leaves a `None` slot, so a stale JS handle turns into a
//! no-op instead of aliasing a new node. A subtree is freed when it is detached from the document
//! (remove / textContent / innerHTML / replaceWith); nodes created by JS and not yet attached live
//! until they are attached and later detached.
pub mod html;

use crate::css::select::{self, SelectorList};

pub type NodeId = u32;

/// Element state flags used by `:hover` and friends.
pub const HOVER: u8 = 1;
pub const ACTIVE: u8 = 2;
pub const FOCUS: u8 = 4;
pub const FOCUS_VISIBLE: u8 = 8;
pub const FOCUS_WITHIN: u8 = 16;

#[derive(Clone, Debug)]
pub struct Elem {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub classes: Vec<String>,
    /// Inline style declarations in insertion order (kebab-case property, raw value).
    pub inline: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Element(Elem),
    Text(String),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: Kind,
    pub state: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutKind {
    ChildList,
    CharacterData,
    Attributes,
}

#[derive(Clone, Copy, Debug)]
pub struct Mutation {
    pub target: NodeId,
    pub kind: MutKind,
}

pub struct Dom {
    nodes: Vec<Option<Box<Node>>>,
    pub html: NodeId,
    pub body: NodeId,
    pub title: String,
    pub mutations: Vec<Mutation>,
    /// Ids freed since the last `take_freed` (JS drops its wrappers/listeners for them).
    pub freed: Vec<NodeId>,
    /// Bumped on every change that can affect style (tree, attributes, classes, inline style, state).
    pub style_gen: u64,
    /// Bumped on every change that can affect layout only (text content, form values).
    pub text_gen: u64,
    /// Form control values (`input`/`textarea` `.value`), keyed by node.
    pub values: std::collections::HashMap<NodeId, String>,
    /// Scroll offsets `(left, top)` of scroll containers.
    pub scroll: std::collections::HashMap<NodeId, (f32, f32)>,
}

impl Default for Dom {
    fn default() -> Self {
        Dom::new()
    }
}

impl Dom {
    /// Empty document with `<html><body></body></html>`.
    pub fn new() -> Dom {
        let mut d = Dom {
            nodes: Vec::new(),
            html: 0,
            body: 0,
            title: String::new(),
            mutations: Vec::new(),
            freed: Vec::new(),
            style_gen: 1,
            text_gen: 1,
            values: Default::default(),
            scroll: Default::default(),
        };
        d.html = d.create_element("html");
        d.body = d.create_element("body");
        d.append_child(d.html, d.body);
        d.mutations.clear();
        d
    }

    /// The document from `ui/index.html`.
    pub fn from_index() -> Dom {
        html::parse_document(include_str!("../../ui/index.html"))
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize).and_then(|n| n.as_deref())
    }
    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id as usize).and_then(|n| n.as_deref_mut())
    }
    pub fn exists(&self, id: NodeId) -> bool {
        self.node(id).is_some()
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn elem(&self, id: NodeId) -> Option<&Elem> {
        match &self.node(id)?.kind {
            Kind::Element(e) => Some(e),
            Kind::Text(_) => None,
        }
    }
    fn elem_mut(&mut self, id: NodeId) -> Option<&mut Elem> {
        match &mut self.node_mut(id)?.kind {
            Kind::Element(e) => Some(e),
            Kind::Text(_) => None,
        }
    }
    pub fn tag(&self, id: NodeId) -> &str {
        self.elem(id).map(|e| e.tag.as_str()).unwrap_or("")
    }
    pub fn is_element(&self, id: NodeId) -> bool {
        self.elem(id).is_some()
    }
    pub fn text(&self, id: NodeId) -> Option<&str> {
        match &self.node(id)?.kind {
            Kind::Text(s) => Some(s),
            Kind::Element(_) => None,
        }
    }
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.parent
    }
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.node(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }
    pub fn element_children(&self, id: NodeId) -> Vec<NodeId> {
        self.children(id).iter().copied().filter(|&c| self.is_element(c)).collect()
    }
    pub fn state(&self, id: NodeId) -> u8 {
        self.node(id).map(|n| n.state).unwrap_or(0)
    }
    /// True when `id` is `anc` or inside it.
    pub fn is_inclusive_descendant(&self, mut id: NodeId, anc: NodeId) -> bool {
        loop {
            if id == anc {
                return true;
            }
            match self.parent(id) {
                Some(p) => id = p,
                None => return false,
            }
        }
    }
    /// Attached to the document (reachable from `<html>`).
    pub fn connected(&self, id: NodeId) -> bool {
        self.exists(id) && self.is_inclusive_descendant(id, self.html)
    }

    fn alloc(&mut self, kind: Kind) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Some(Box::new(Node { parent: None, children: Vec::new(), kind, state: 0 })));
        id
    }
    pub fn create_element(&mut self, tag: &str) -> NodeId {
        self.alloc(Kind::Element(Elem {
            tag: tag.to_ascii_lowercase(),
            attrs: Vec::new(),
            classes: Vec::new(),
            inline: Vec::new(),
        }))
    }
    pub fn create_text(&mut self, s: &str) -> NodeId {
        self.alloc(Kind::Text(s.to_string()))
    }

    fn touch_style(&mut self) {
        self.style_gen += 1;
    }
    fn record(&mut self, target: NodeId, kind: MutKind) {
        self.mutations.push(Mutation { target, kind });
    }

    /// Unlinks `id` from its parent without freeing.
    fn unlink(&mut self, id: NodeId) {
        if let Some(p) = self.parent(id) {
            if let Some(pn) = self.node_mut(p) {
                pn.children.retain(|&c| c != id);
            }
            if let Some(n) = self.node_mut(id) {
                n.parent = None;
            }
            self.record(p, MutKind::ChildList);
            self.touch_style();
        }
    }

    fn free_subtree(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            if let Some(node) = self.nodes.get_mut(n as usize).and_then(|s| s.take()) {
                stack.extend(node.children.iter().copied());
                self.values.remove(&n);
                self.scroll.remove(&n);
                self.freed.push(n);
            }
        }
    }

    /// Detaches `id`; frees it when it was part of the document.
    pub fn remove(&mut self, id: NodeId) {
        let was = self.connected(id);
        self.unlink(id);
        if was {
            self.free_subtree(id);
        }
    }

    /// Appends (or moves) `child` to the end of `parent`.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.insert_before(parent, child, None);
    }

    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, reference: Option<NodeId>) {
        if !self.exists(parent) || !self.exists(child) || self.is_inclusive_descendant(parent, child) {
            return;
        }
        self.unlink(child);
        let pn = self.node_mut(parent).unwrap();
        let at = reference.and_then(|r| pn.children.iter().position(|&c| c == r)).unwrap_or(pn.children.len());
        pn.children.insert(at, child);
        self.node_mut(child).unwrap().parent = Some(parent);
        self.record(parent, MutKind::ChildList);
        self.touch_style();
    }

    pub fn replace_with(&mut self, old: NodeId, new: NodeId) {
        let Some(p) = self.parent(old) else { return };
        if old == new {
            return;
        }
        self.insert_before(p, new, Some(old));
        self.remove(old);
    }

    fn clear_children(&mut self, id: NodeId) {
        for c in self.children(id).to_vec() {
            self.remove(c);
        }
    }

    /// `textContent = s`.
    pub fn set_text_content(&mut self, id: NodeId, s: &str) {
        if let Some(Kind::Text(t)) = self.node_mut(id).map(|n| &mut n.kind) {
            *t = s.to_string();
            self.text_gen += 1;
            self.record(id, MutKind::CharacterData);
            return;
        }
        if !self.is_element(id) {
            return;
        }
        // Fast path: a single text child is updated in place (keeps ids stable, cheap for clocks).
        let kids = self.children(id).to_vec();
        if kids.len() == 1 && self.text(kids[0]).is_some() && !s.is_empty() {
            if let Some(Kind::Text(t)) = self.node_mut(kids[0]).map(|n| &mut n.kind) {
                if t != s {
                    *t = s.to_string();
                    self.text_gen += 1;
                    self.touch_style(); // `:empty` may not change, but layout must
                }
            }
            self.record(id, MutKind::ChildList);
            return;
        }
        self.clear_children(id);
        if !s.is_empty() {
            let t = self.create_text(s);
            self.append_child(id, t);
        }
        self.record(id, MutKind::ChildList);
    }

    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }
    fn collect_text(&self, id: NodeId, out: &mut String) {
        match self.node(id).map(|n| &n.kind) {
            Some(Kind::Text(t)) => out.push_str(t),
            Some(Kind::Element(_)) => {
                for &c in self.children(id) {
                    self.collect_text(c, out);
                }
            }
            None => {}
        }
    }

    pub fn set_inner_html(&mut self, id: NodeId, src: &str) {
        if !self.is_element(id) {
            return;
        }
        self.clear_children(id);
        let svg = self.tag(id) == "svg" || self.in_svg(id);
        html::parse_fragment(self, id, src, svg);
        self.record(id, MutKind::ChildList);
    }
    fn in_svg(&self, mut id: NodeId) -> bool {
        while let Some(p) = self.parent(id) {
            if self.tag(p) == "svg" {
                return true;
            }
            id = p;
        }
        false
    }

    // ---- attributes ----
    /// Attribute names are stored lowercase (HTML); lookups are case-insensitive.
    pub fn attr(&self, id: NodeId, name: &str) -> Option<&str> {
        let e = self.elem(id)?;
        e.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn has_attr(&self, id: NodeId, name: &str) -> bool {
        self.attr(id, name).is_some()
    }
    pub fn set_attr(&mut self, id: NodeId, name: &str, value: &str) {
        let name = name.to_ascii_lowercase();
        let Some(e) = self.elem_mut(id) else { return };
        if name == "class" {
            e.classes = value.split_ascii_whitespace().map(str::to_string).collect();
        }
        match e.attrs.iter_mut().find(|(k, _)| *k == name) {
            Some((_, v)) if v == value => return,
            Some((_, v)) => *v = value.to_string(),
            None => e.attrs.push((name, value.to_string())),
        }
        self.touch_style();
        self.record(id, MutKind::Attributes);
    }
    pub fn remove_attr(&mut self, id: NodeId, name: &str) {
        let name = &name.to_ascii_lowercase();
        let Some(e) = self.elem_mut(id) else { return };
        let before = e.attrs.len();
        e.attrs.retain(|(k, _)| k != name);
        if name == "class" {
            e.classes.clear();
        }
        if e.attrs.len() != before {
            self.touch_style();
            self.record(id, MutKind::Attributes);
        }
    }

    // ---- classes ----
    pub fn has_class(&self, id: NodeId, c: &str) -> bool {
        self.elem(id).is_some_and(|e| e.classes.iter().any(|x| x == c))
    }
    fn sync_class_attr(&mut self, id: NodeId) {
        let Some(e) = self.elem_mut(id) else { return };
        let v = e.classes.join(" ");
        match e.attrs.iter_mut().find(|(k, _)| k == "class") {
            Some((_, x)) => *x = v,
            None => e.attrs.push(("class".into(), v)),
        }
        self.touch_style();
        self.record(id, MutKind::Attributes);
    }
    pub fn add_class(&mut self, id: NodeId, c: &str) {
        if c.is_empty() || self.has_class(id, c) || !self.is_element(id) {
            return;
        }
        self.elem_mut(id).unwrap().classes.push(c.to_string());
        self.sync_class_attr(id);
    }
    pub fn remove_class(&mut self, id: NodeId, c: &str) {
        if !self.has_class(id, c) {
            return;
        }
        self.elem_mut(id).unwrap().classes.retain(|x| x != c);
        self.sync_class_attr(id);
    }
    /// `classList.toggle(c, force)`; returns whether the class is now present.
    pub fn toggle_class(&mut self, id: NodeId, c: &str, force: Option<bool>) -> bool {
        let on = force.unwrap_or(!self.has_class(id, c));
        if on {
            self.add_class(id, c)
        } else {
            self.remove_class(id, c)
        }
        on
    }

    // ---- inline style ----
    pub fn style_get(&self, id: NodeId, prop: &str) -> Option<&str> {
        self.elem(id)?.inline.iter().find(|(k, _)| k == prop).map(|(_, v)| v.as_str())
    }
    pub fn style_set(&mut self, id: NodeId, prop: &str, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return self.style_remove(id, prop);
        }
        let Some(e) = self.elem_mut(id) else { return };
        match e.inline.iter_mut().find(|(k, _)| k == prop) {
            Some((_, v)) if v == value => return,
            Some((_, v)) => *v = value.to_string(),
            None => e.inline.push((prop.to_string(), value.to_string())),
        }
        self.touch_style();
    }
    pub fn style_remove(&mut self, id: NodeId, prop: &str) {
        let Some(e) = self.elem_mut(id) else { return };
        let before = e.inline.len();
        e.inline.retain(|(k, _)| k != prop);
        if e.inline.len() != before {
            self.touch_style();
        }
    }

    // ---- form values / scroll ----
    pub fn value(&self, id: NodeId) -> &str {
        self.values.get(&id).map(String::as_str).unwrap_or("")
    }
    pub fn set_value(&mut self, id: NodeId, v: &str) {
        if self.value(id) != v {
            self.values.insert(id, v.to_string());
            self.text_gen += 1;
        }
    }
    pub fn scroll_of(&self, id: NodeId) -> (f32, f32) {
        self.scroll.get(&id).copied().unwrap_or((0.0, 0.0))
    }

    // ---- state flags ----
    pub fn set_state(&mut self, id: NodeId, flag: u8, on: bool) {
        let Some(n) = self.node_mut(id) else { return };
        let next = if on { n.state | flag } else { n.state & !flag };
        if next != n.state {
            n.state = next;
            self.touch_style();
        }
    }

    // ---- queries ----
    /// Elements under `root` (excluded) in tree order.
    pub fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(root).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if self.is_element(n) {
                out.push(n);
                stack.extend(self.children(n).iter().rev().copied());
            }
        }
        out
    }
    pub fn query(&self, root: NodeId, sel: &SelectorList, all: bool) -> Vec<NodeId> {
        let mut out = Vec::new();
        for n in self.descendants(root) {
            if select::matches_list(self, n, sel) {
                out.push(n);
                if !all {
                    break;
                }
            }
        }
        out
    }
    pub fn query_str(&self, root: NodeId, sel: &str, all: bool) -> Vec<NodeId> {
        match select::parse_selector_list(sel) {
            Some(s) => self.query(root, &s, all),
            None => Vec::new(),
        }
    }
    pub fn matches(&self, id: NodeId, sel: &SelectorList) -> bool {
        self.is_element(id) && select::matches_list(self, id, sel)
    }
    pub fn closest(&self, mut id: NodeId, sel: &SelectorList) -> Option<NodeId> {
        loop {
            if self.matches(id, sel) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
    pub fn by_id(&self, id_attr: &str) -> Option<NodeId> {
        self.descendants(self.html).into_iter().find(|&n| self.attr(n, "id") == Some(id_attr))
    }

    pub fn take_mutations(&mut self) -> Vec<Mutation> {
        std::mem::take(&mut self.mutations)
    }
    pub fn take_freed(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.freed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_structure() {
        let d = Dom::from_index();
        let clip = d.query_str(d.html, "#island .clip", false)[0];
        let kids: Vec<String> = d
            .element_children(clip)
            .iter()
            .map(|&k| d.elem(k).unwrap().classes[0].clone())
            .collect();
        assert_eq!(kids, ["ambient", "row", "hud", "open", "voice", "answer"]);
        let nav = d.by_id("pageNav").unwrap();
        assert!(d.children(nav).is_empty());
        assert_eq!(d.title, "Blip");
        assert_eq!(d.attr(d.body, "data-state"), Some("idle"));
        assert!(d.query_str(d.html, "script", true).is_empty());
    }

    #[test]
    fn inner_html_and_free() {
        let mut d = Dom::from_index();
        let msgs = d.by_id("messages").unwrap();
        let div = d.create_element("div");
        d.append_child(msgs, div);
        d.set_inner_html(div, r#"<span class="caret"></span>"#);
        let kids = d.children(div).to_vec();
        assert_eq!(kids.len(), 1);
        assert_eq!(d.tag(kids[0]), "span");
        assert!(d.has_class(kids[0], "caret"));
        d.set_text_content(msgs, "");
        assert!(!d.exists(div) && !d.exists(kids[0]));
        assert!(d.take_freed().contains(&div));
    }

    #[test]
    fn svg_inner_html_and_text() {
        let mut d = Dom::from_index();
        let ic = d.by_id("hudIcon").unwrap();
        d.set_inner_html(ic, r#"<path d="M4 9h3"/><rect x="7" y="18" width="10" height="2.2" rx="1.1"/>"#);
        let k = d.children(ic).to_vec();
        assert_eq!(k.len(), 2);
        assert_eq!(d.tag(k[1]), "rect");
        assert_eq!(d.attr(k[1], "rx"), Some("1.1"));
        let t = d.by_id("title").unwrap();
        assert_eq!(d.text_content(t), "Blip");
        d.set_text_content(t, "a &lt; b");
        assert_eq!(d.text_content(t), "a &lt; b");
    }
}
