use super::{MAIN, parse_xml};
use std::fmt::Write;

#[test]
fn declarations_restore_after_empty_and_nested_elements() {
    let source = format!(
        "<root xmlns=\"{MAIN}\" xmlns:p=\"outer\"><p:a xmlns:p=\"empty\"/><p:b xmlns:p=\"inner\"><p:c/></p:b><p:d/></root>"
    );
    let xml = parse_xml(source.as_bytes()).unwrap();
    assert_eq!(xml.children[0].ns.as_str(), "empty");
    assert_eq!(xml.children[1].children[0].ns.as_str(), "inner");
    assert_eq!(xml.children[2].ns.as_str(), "outer");
    for body in ["<a xmlns:p=\"local\"/>", "<a xmlns:p=\"local\"><p:b/></a>"] {
        let error = parse_xml(format!("<root>{body}<p:c/></root>").as_bytes()).unwrap_err();
        assert_eq!(error.to_string(), "Unbound XML namespace prefix");
    }
    let mut declarations = String::new();
    for i in 0..63 {
        write!(declarations, " xmlns:p{i}=\"uri\"").unwrap();
    }
    parse_xml(format!("<root{declarations}><a xmlns:p0=\"shadow\"/><p0:b/></root>").as_bytes())
        .unwrap();
    let error =
        parse_xml(format!("<root{declarations}><a xmlns:extra=\"uri\"/></root>").as_bytes())
            .unwrap_err();
    assert_eq!(error.to_string(), "XML namespace scope exceeds profile");
}

#[test]
fn relationship_aliases_keep_sorted_overwrite_and_scope() {
    let office = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let source = format!(
        "<root xmlns:a=\"{office}\" xmlns:z=\"{office}\" xmlns:rel=\"other\"><c a:id=\"first\" rel:id=\"original\" z:other=\"last\"/><c xmlns:a=\"other\" a:id=\"local\"/><c a:id=\"restored\"/></root>"
    );
    let xml = parse_xml(source.as_bytes()).unwrap();
    assert_eq!(xml.children[0].attr("rel:id"), Some("first"));
    assert_eq!(xml.children[0].attr("a:id"), Some("first"));
    assert_eq!(xml.children[0].attr("rel:other"), Some("last"));
    assert_eq!(xml.children[1].attr("rel:id"), None);
    assert_eq!(xml.children[2].attr("rel:id"), Some("restored"));
}
