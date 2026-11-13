//! An editable XML tree with the same model and the same output as Python's ElementTree,
//! which wrote every repaired copy so far: element text, and the "tail" text that follows
//! each element. Comments and processing instructions are dropped on reading (as ET does);
//! a set written back without changes is byte-identical to ET's.
use std::io::Read;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Element {
    pub tag: String,
    /// In document order.
    pub attrs: Vec<(String, String)>,
    /// Text before the first child.
    pub text: Option<String>,
    /// Text after this element's end tag, inside its parent.
    pub tail: Option<String>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn new(tag: &str) -> Element {
        Element { tag: tag.into(), ..Default::default() }
    }

    pub fn get(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, k: &str, v: impl Into<String>) {
        let v = v.into();
        match self.attrs.iter_mut().find(|(a, _)| a == k) {
            Some(slot) => slot.1 = v,
            None => self.attrs.push((k.into(), v)),
        }
    }

    /// First direct child with this tag.
    pub fn child(&self, tag: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.tag == tag)
    }

    pub fn child_mut(&mut self, tag: &str) -> Option<&mut Element> {
        self.children.iter_mut().find(|c| c.tag == tag)
    }

    /// ET's `find("A/B/C")`: a path of direct children; a leading ".//" searches the whole
    /// subtree (document order) for the first step.
    pub fn find(&self, path: &str) -> Option<&Element> {
        if let Some(rest) = path.strip_prefix(".//") {
            let (first, more) = rest.split_once('/').map_or((rest, None), |(a, b)| (a, Some(b)));
            return self.descendants().skip(1).filter(|e| e.tag == first).find_map(|e| match more {
                Some(m) => e.find(m),
                None => Some(e),
            });
        }
        path.split('/').try_fold(self, |e, step| e.child(step))
    }

    pub fn find_mut(&mut self, path: &str) -> Option<&mut Element> {
        path.split('/').try_fold(self, |e, step| e.child_mut(step))
    }

    /// ET's `findall("A/B")`: every element at that path of direct children, in order.
    pub fn find_all(&self, path: &str) -> Vec<&Element> {
        let mut cur = vec![self];
        for step in path.split('/') {
            cur = cur.into_iter().flat_map(|e| e.children.iter().filter(move |c| c.tag == step)).collect();
        }
        cur
    }

    /// The element at these child indices.
    pub fn at_mut(&mut self, path: &[usize]) -> &mut Element {
        path.iter().fold(self, |e, i| &mut e.children[*i])
    }

    pub fn at(&self, path: &[usize]) -> &Element {
        path.iter().fold(self, |e, i| &e.children[*i])
    }

    /// Sets the `Value` of the element at `path` (direct children); false when there is none.
    pub fn set_value(&mut self, path: &str, v: impl Into<String>) -> bool {
        match self.find_mut(path) {
            Some(e) => {
                e.set("Value", v);
                true
            }
            None => false,
        }
    }

    /// The `Value` attribute of the element at `path`.
    pub fn value(&self, path: &str) -> Option<&str> {
        self.find(path).and_then(|e| e.get("Value"))
    }

    /// This element and everything under it, in document order (ET's `iter()`).
    pub fn descendants(&self) -> impl Iterator<Item = &Element> {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let e = stack.pop()?;
            stack.extend(e.children.iter().rev());
            Some(e)
        })
    }

    /// Visits every element mutably, in document order, parents before children.
    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut Element)) {
        f(self);
        for c in &mut self.children {
            c.walk_mut(f);
        }
    }

    /// Removes, anywhere below, the children for which `drop(parent, child)` is true. Returns
    /// how many were removed. A removed element's tail goes with it (as in ET).
    pub fn remove_where(&mut self, drop: &mut impl FnMut(&Element, &Element) -> bool) -> usize {
        let mut n = 0;
        let keep: Vec<bool> = self.children.iter().map(|c| !drop(self, c)).collect();
        if keep.iter().any(|k| !k) {
            let mut i = 0;
            self.children.retain(|_| {
                let k = keep[i];
                i += 1;
                k
            });
            n += keep.iter().filter(|k| !**k).count();
        }
        for c in &mut self.children {
            n += c.remove_where(drop);
        }
        n
    }
}

/// Parses a document. Character references and entities are decoded, as by expat.
pub fn parse(text: &str) -> Result<Element, String> {
    let opt = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX };
    let doc = roxmltree::Document::parse_with_options(text, opt).map_err(|e| e.to_string())?;
    Ok(build(doc.root_element()))
}

fn build(n: roxmltree::Node) -> Element {
    let mut e = Element {
        tag: n.tag_name().name().to_string(),
        attrs: n.attributes().map(|a| (a.name().to_string(), a.value().to_string())).collect(),
        ..Default::default()
    };
    // text pieces separated only by comments / PIs are one piece, as in ET
    let mut pending: Option<String> = None;
    for c in n.children() {
        if c.is_element() {
            let t = pending.take();
            match e.children.last_mut() {
                Some(prev) => prev.tail = t,
                None => e.text = t,
            }
            e.children.push(build(c));
        } else if c.is_text() {
            pending.get_or_insert_with(String::new).push_str(c.text().unwrap_or(""));
        }
    }
    let t = pending.take();
    match e.children.last_mut() {
        Some(prev) => prev.tail = t,
        None => e.text = t,
    }
    e
}

fn escape_text(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#09;"),
            c => out.push(c),
        }
    }
}

/// ET's `tostring(elem, encoding="utf-8")`: no declaration, " />" for empty elements, the
/// tail of `e` itself included.
pub fn serialize(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.tag);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if e.text.as_deref().is_some_and(|t| !t.is_empty()) || !e.children.is_empty() {
        out.push('>');
        if let Some(t) = &e.text {
            escape_text(t, out);
        }
        for c in &e.children {
            serialize(c, out);
        }
        out.push_str("</");
        out.push_str(&e.tag);
        out.push('>');
    } else {
        out.push_str(" />");
    }
    if let Some(t) = &e.tail {
        escape_text(t, out);
    }
}

/// A set as read: its tree, and whether Live wrote it with Windows line endings.
pub struct Set {
    pub root: Element,
    pub crlf: bool,
}

/// Reads a gzipped .als (or a plain XML file).
pub fn read_set(path: &std::path::Path) -> Result<Set, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let xml = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut s = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..]).read_to_end(&mut s).map_err(|e| e.to_string())?;
        s
    } else {
        raw
    };
    let crlf = xml[..xml.len().min(200)].windows(2).any(|w| w == b"\r\n");
    let text = String::from_utf8(xml).map_err(|e| e.to_string())?;
    Ok(Set { root: parse(&text)?, crlf })
}

/// The bytes of a set as the prototype wrote them (before gzip): its declaration, the tree,
/// Live's line endings kept (newlines inside attributes are escaped, so only layout changes).
pub fn set_bytes(set: &Set) -> Vec<u8> {
    let mut s = String::with_capacity(1 << 20);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    serialize(&set.root, &mut s);
    if set.crlf { s.replace('\n', "\r\n").into_bytes() } else { s.into_bytes() }
}

/// Writes a set gzipped as a NEW file: through a temporary file (a crash never leaves half a
/// set) given the name `out` only if no file has it (crate::safe: never overwrites).
pub fn write_set(set: &Set, out: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    if out.exists() {
        return Err(i18n::tr!("repair.existsNoOverwrite", path = out.display()));
    }
    let (tmp, f) = crate::safe::new_temp(out).map_err(|e| e.to_string())?;
    let mut gz = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(9));
    let written = gz.write_all(&set_bytes(set)).and_then(|_| gz.finish()).and_then(|mut w| w.flush());
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp); // our own temporary file
        return Err(e.to_string());
    }
    crate::safe::publish(&tmp, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(s: &str) -> String {
        let mut o = String::new();
        serialize(&parse(s).unwrap(), &mut o);
        o
    }

    #[test]
    fn like_elementtree() {
        assert_eq!(
            round(r#"<A x="1"><B/>  <C Value="a&amp;b&#10;c">t&lt;</C><!-- c -->x</A>"#),
            r#"<A x="1"><B />  <C Value="a&amp;b&#10;c">t&lt;</C>x</A>"#
        );
        // a literal tab in an attribute is a space once parsed (XML rule, expat too); &#9; stays a tab
        assert_eq!(round("<A>\n\t<B Value=\"&quot;é\t&#9;\" />\n</A>"), "<A>\n\t<B Value=\"&quot;é &#09;\" />\n</A>");
        assert_eq!(round("<A>a<!--x-->b<B/>c<?p?>d</A>"), "<A>ab<B />cd</A>");
    }

    #[test]
    fn find_and_edit() {
        let mut e = parse(r#"<R><L><D><P Value="1"/></D></L><Q><P Value="2"/></Q></R>"#).unwrap();
        assert_eq!(e.value("L/D/P"), Some("1"));
        assert_eq!(e.find(".//P").and_then(|p| p.get("Value")), Some("1"));
        assert_eq!(e.find(".//Q/P").and_then(|p| p.get("Value")), Some("2"));
        e.find_mut("L/D/P").unwrap().set("Value", "9");
        assert_eq!(e.remove_where(&mut |_, c| c.tag == "Q"), 1);
        let mut o = String::new();
        serialize(&e, &mut o);
        assert_eq!(o, r#"<R><L><D><P Value="9" /></D></L></R>"#);
    }
}
