//! Small helpers over roxmltree mirroring the ElementTree calls of the prototype.
use roxmltree::Node;

pub type N<'a, 'i> = Node<'a, 'i>;

pub fn tag<'a>(n: N<'a, '_>) -> &'a str {
    n.tag_name().name()
}

/// Element children, in order.
pub fn children<'a, 'i>(n: N<'a, 'i>) -> impl Iterator<Item = N<'a, 'i>> {
    n.children().filter(|c| c.is_element())
}

/// First direct child with this tag (`el.find("Tag")`).
pub fn child<'a, 'i>(n: N<'a, 'i>, name: &str) -> Option<N<'a, 'i>> {
    children(n).find(|c| tag(*c) == name)
}

/// `el.find("A/B/C")`: a path of direct children.
pub fn path<'a, 'i>(n: N<'a, 'i>, p: &str) -> Option<N<'a, 'i>> {
    let mut cur = n;
    for part in p.split('/') {
        cur = child(cur, part)?;
    }
    Some(cur)
}

/// Every element under `n`, `n` included, in document order (`el.iter()`).
pub fn iter<'a, 'i>(n: N<'a, 'i>) -> impl Iterator<Item = N<'a, 'i>> {
    n.descendants().filter(|c| c.is_element())
}

/// `el.iter("Tag")`.
pub fn iter_tag<'a, 'i>(n: N<'a, 'i>, name: &'static str) -> impl Iterator<Item = N<'a, 'i>> {
    iter(n).filter(move |c| tag(*c) == name)
}

/// `el.find(".//Tag")`: the first descendant with this tag (not `n` itself).
pub fn find_desc<'a, 'i>(n: N<'a, 'i>, name: &str) -> Option<N<'a, 'i>> {
    n.descendants().skip(1).find(|c| c.is_element() && tag(*c) == name)
}

/// `el.find(".//A/B")`: the first descendant A that has a child B, then that B.
pub fn find_desc_path<'a, 'i>(n: N<'a, 'i>, first: &str, then: &str) -> Option<N<'a, 'i>> {
    n.descendants().skip(1).filter(|c| c.is_element() && tag(*c) == first).find_map(|a| child(a, then))
}

pub fn attr<'a>(n: N<'a, '_>, name: &str) -> Option<&'a str> {
    n.attribute(name)
}

/// `_val(el, tag)`: the Value attribute of a direct child (or of a child path).
pub fn val<'a>(n: Option<N<'a, '_>>, p: &str) -> Option<&'a str> {
    path(n?, p)?.attribute("Value")
}

/// `a or b` for optional strings.
pub fn or<'a>(a: Option<&'a str>, b: Option<&'a str>) -> Option<&'a str> {
    match a {
        Some(s) if !s.is_empty() => Some(s),
        _ => b,
    }
}

/// ElementTree's `.text`: the text before the first child element.
pub fn text<'a>(n: N<'a, '_>) -> Option<&'a str> {
    n.first_child().filter(|c| c.is_text()).and_then(|c| c.text())
}

/// Python's `round(x, 2)` (correctly rounded, ties to even on the exact value).
pub fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// Python 3.12+'s `sum()` of floats: Neumaier compensated summation, so totals (and the
/// percentages and rounded values made from them) come out exactly as in the prototype.
pub fn py_sum(xs: impl IntoIterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in xs {
        let t = s + x;
        if s.abs() >= x.abs() {
            c += (s - t) + x;
        } else {
            c += (x - t) + s;
        }
        s = t;
    }
    if c != 0.0 && c.is_finite() { s + c } else { s }
}
