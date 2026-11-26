use super::{Result, decode_xstring, fail};
use std::{borrow::Cow, collections::BTreeMap, rc::Rc};

// Namespace-aware bounded XML tree. No DTD or entity expansion is accepted.
#[derive(Clone, Debug)]
pub(super) struct Xml<'src> {
    pub(super) name: Rc<String>,
    pub(super) ns: Rc<String>,
    pub(super) attrs: Vec<(Cow<'src, str>, Cow<'src, str>)>,
    pub(super) namespace_declarations: Vec<String>,
    pub(super) children: Vec<Xml<'src>>,
    pub(super) text: Cow<'src, str>,
    pub(super) plain_attribute_namespaces: bool,
    pub(super) plain_lexical_values: bool,
}
impl<'src> Xml<'src> {
    pub(super) fn push_text(&mut self, raw: &[u8], text: Cow<'src, str>) {
        self.plain_lexical_values &=
            !matches!(self.name.as_str(), "t" | "v") || !raw.contains(&b'\r');
        if self.text.is_empty() {
            self.text = text;
        } else {
            self.text.to_mut().push_str(&text);
        }
    }

    pub(super) fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_ref())
    }
    pub(super) fn child(&self, name: &str) -> Option<&Xml<'src>> {
        self.children
            .iter()
            .find(|x| x.name.as_str() == name && x.ns == self.ns)
    }
    pub(super) fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Xml<'src>> {
        self.children
            .iter()
            .filter(move |x| x.name.as_str() == name && x.ns == self.ns)
    }
    pub(super) fn all<'a>(&'a self, name: &str, out: &mut Vec<&'a Xml<'src>>) {
        if self.name.as_str() == name {
            out.push(self);
        }
        for c in &self.children {
            c.all(name, out);
        }
    }
    pub(super) fn content(&self) -> Result<String> {
        if self.name.as_str() == "t" {
            return decode_xstring(&self.text);
        }
        let mut parts = self
            .children
            .iter()
            .filter(|c| c.ns == self.ns && matches!(c.name.as_str(), "t" | "r"));
        let Some(first) = parts.next() else {
            return Ok(String::new());
        };
        let mut text = first.content()?;
        for part in parts {
            text.push_str(&part.content()?);
        }
        Ok(text)
    }
}
pub(super) fn restore_xml_bindings(
    bindings: &mut BTreeMap<String, Rc<String>>,
    changes: &mut Vec<(String, Option<Rc<String>>)>,
    scope_start: usize,
) {
    for (prefix, previous) in changes.drain(scope_start..).rev() {
        if let Some(previous) = previous {
            bindings.insert(prefix, previous);
        } else {
            bindings.remove(&prefix);
        }
    }
}
pub(super) fn attach_xml<'src>(
    node: Xml<'src>,
    stack: &mut [(Xml<'src>, usize)],
    root: &mut Option<Xml<'src>>,
) -> Result<()> {
    if let Some((parent, _)) = stack.last_mut() {
        // Cell value/formula and inline-text nodes usually have one or two
        // children. Vec's default first allocation holds four full Xml nodes.
        if parent.children.is_empty() {
            parent.children.reserve_exact(1);
        }
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return fail("Multiple XML roots");
    }
    Ok(())
}
