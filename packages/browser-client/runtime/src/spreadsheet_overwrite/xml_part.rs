use std::{borrow::Cow, collections::BTreeMap, str};

use quick_xml::{
    NsReader,
    escape::unescape,
    events::{BytesStart, Event},
    name::{LocalName, ResolveResult},
};

type ExpandedName = (String, String);
type Attributes = BTreeMap<ExpandedName, String>;
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const SPREADSHEET_NAMESPACE: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const XML_WHITESPACE: [char; 4] = [' ', '\t', '\n', '\r'];

#[derive(PartialEq, Eq)]
enum Token {
    Start(ExpandedName, Attributes),
    End,
    Text(String),
    Comment(String),
    ProcessingInstruction(String),
    Eof,
}

pub(super) fn equivalent(path: &str, original: &[u8], regenerated: &[u8]) -> bool {
    if original == regenerated {
        return true;
    }
    if !path.rsplit_once('.').is_some_and(|(_, suffix)| {
        suffix.eq_ignore_ascii_case("xml") || suffix.eq_ignore_ascii_case("rels")
    }) {
        return false;
    }
    let mut original = XmlPart::new(original);
    let mut regenerated = XmlPart::new(regenerated);
    loop {
        match (original.next(), regenerated.next()) {
            (Some(Token::Eof), Some(Token::Eof)) => return true,
            (Some(left), Some(right)) => {
                if !equivalent_tokens(left, right, &mut original, &mut regenerated) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

fn equivalent_tokens(
    original: Token,
    regenerated: Token,
    original_xml: &mut XmlPart<'_>,
    regenerated_xml: &mut XmlPart<'_>,
) -> bool {
    if original == regenerated {
        return true;
    }
    let (Token::Start(name, attributes), Token::Start(other_name, other_attributes)) =
        (original, regenerated)
    else {
        return false;
    };
    name == other_name
        && name.0 == SPREADSHEET_NAMESPACE
        && name.1 == "t"
        && equivalent_text_attributes(attributes, other_attributes)
        && equivalent_unpadded_text(original_xml, regenerated_xml)
}

fn equivalent_text_attributes(mut original: Attributes, mut regenerated: Attributes) -> bool {
    let key = (XML_NAMESPACE.to_owned(), "space".to_owned());
    [original.remove(&key), regenerated.remove(&key)]
        .iter()
        .all(|space| matches!(space.as_deref(), None | Some("default" | "preserve")))
        && original == regenerated
}

fn equivalent_unpadded_text(original: &mut XmlPart<'_>, regenerated: &mut XmlPart<'_>) -> bool {
    // xml:space can change how a reader interprets leading or trailing whitespace.
    match (original.next(), regenerated.next()) {
        (Some(Token::End), Some(Token::End)) => true,
        (Some(Token::Text(left)), Some(Token::Text(right)))
            if left == right && left == left.trim_matches(XML_WHITESPACE) =>
        {
            matches!(
                (original.next(), regenerated.next()),
                (Some(Token::End), Some(Token::End))
            )
        }
        _ => false,
    }
}

struct XmlPart<'a> {
    reader: NsReader<&'a [u8]>,
    pending: Option<Event<'a>>,
    elements: Vec<Element>,
}

struct Element {
    preserve_space: bool,
    carries_value: bool,
}

impl<'a> XmlPart<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let mut reader = NsReader::from_reader(bytes);
        reader.config_mut().expand_empty_elements = true;
        Self {
            reader,
            pending: None,
            elements: Vec::new(),
        }
    }

    fn read(&mut self) -> Option<Event<'a>> {
        self.pending
            .take()
            .or_else(|| self.reader.read_event().ok())
    }

    fn next(&mut self) -> Option<Token> {
        loop {
            let event = self.read()?;
            if matches!(event, Event::Text(_) | Event::CData(_)) {
                let text = self.read_text(event)?;
                if self.keeps_text(&text) {
                    return Some(Token::Text(text));
                }
            } else if !matches!(event, Event::Decl(_)) {
                return self.structure(event);
            }
        }
    }

    fn read_text(&mut self, mut event: Event<'a>) -> Option<String> {
        let mut text = String::new();
        loop {
            match &event {
                Event::Text(chunk) => text.push_str(&unescape(&xml_lines(chunk)?).ok()?),
                Event::CData(chunk) => text.push_str(&xml_lines(chunk)?),
                _ => {
                    self.pending = Some(event);
                    return Some(text);
                }
            }
            event = self.read()?;
        }
    }

    fn keeps_text(&self, text: &str) -> bool {
        !text.is_empty()
            && (self
                .elements
                .last()
                .is_some_and(|element| element.preserve_space || element.carries_value)
                || !text.trim_matches(XML_WHITESPACE).is_empty())
    }

    fn structure(&mut self, event: Event<'a>) -> Option<Token> {
        match event {
            Event::Start(element) => self.start(&element),
            Event::End(_) => {
                self.elements.pop()?;
                Some(Token::End)
            }
            Event::Comment(text) => Some(Token::Comment(xml_lines(&text)?.into_owned())),
            Event::PI(text) => Some(Token::ProcessingInstruction(xml_lines(&text)?.into_owned())),
            Event::Eof => Some(Token::Eof),
            _ => None,
        }
    }

    fn start(&mut self, element: &BytesStart<'_>) -> Option<Token> {
        let (namespace, local) = self.reader.resolve_element(element.name());
        let name = expanded_name(&namespace, local)?;
        let mut attributes = BTreeMap::new();
        let mut preserve_space = self
            .elements
            .last()
            .is_some_and(|parent| parent.preserve_space);
        for attribute in element.attributes() {
            let attribute = attribute.ok()?;
            if attribute.key.as_namespace_binding().is_some() {
                continue;
            }
            let (namespace, local) = self.reader.resolve_attribute(attribute.key);
            let key = expanded_name(&namespace, local)?;
            let value = attribute_value(&attribute.value)?;
            if key.0 == XML_NAMESPACE && key.1 == "space" {
                preserve_space = value != "default";
            }
            if attributes.insert(key, value).is_some() {
                return None;
            }
        }
        // The XLSX writer emits character data only in these value elements.
        // Whitespace inside them matters even without an xml:space attribute.
        let carries_value = matches!(name.1.as_str(), "t" | "v" | "f");
        self.elements.push(Element {
            preserve_space,
            carries_value,
        });
        Some(Token::Start(name, attributes))
    }
}

fn expanded_name(namespace: &ResolveResult<'_>, local: LocalName<'_>) -> Option<ExpandedName> {
    let uri = match namespace {
        ResolveResult::Unbound => String::new(),
        ResolveResult::Bound(namespace) => attribute_value(namespace.into_inner())?,
        ResolveResult::Unknown(_) => return None,
    };
    Some((uri, str::from_utf8(local.into_inner()).ok()?.to_owned()))
}

fn xml_lines(bytes: &[u8]) -> Option<Cow<'_, str>> {
    let text = str::from_utf8(bytes).ok()?;
    Some(if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(text)
    })
}

fn attribute_value(bytes: &[u8]) -> Option<String> {
    let text = xml_lines(bytes)?;
    if text.contains(['\n', '\t']) {
        return unescape(&text.replace(['\n', '\t'], " "))
            .ok()
            .map(Cow::into_owned);
    }
    unescape(&text).ok().map(Cow::into_owned)
}
