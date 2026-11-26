use super::{CellStyle, MAIN, c_unknown, cell_value, ignorable_formula_cache, parse_xml};

fn cell_xml(kind: &str, carrier: &str, formula: &str) -> String {
    format!("<c xmlns=\"{MAIN}\"{kind}>{formula}{carrier}</c>")
}

#[test]
fn mismatched_carriers_cannot_reach_defaults_or_formula_cache_recovery() {
    for (kind, carrier) in [
        ("", "<is><t>value</t></is>"),
        (" t=\"n\"", "<is><t>value</t></is>"),
        (" t=\"str\"", "<is><t>value</t></is>"),
        (" t=\"b\"", "<is><t>1</t></is>"),
        (" t=\"s\"", "<is><t>0</t></is>"),
        (" t=\"e\"", "<is><t>#VALUE!</t></is>"),
        (" t=\"d\"", "<is><t>2026-10-10</t></is>"),
        (" t=\"inlineStr\"", "<v>123</v>"),
    ] {
        for formula in ["", "<f>1+2</f>"] {
            let xml = cell_xml(kind, carrier, formula);
            let cell = parse_xml(xml.as_bytes()).unwrap();
            assert!(c_unknown(&cell), "{kind} {carrier} {formula}");
            assert!(cell_value(&cell, &[], &CellStyle::default(), false).is_err());
            assert!(!ignorable_formula_cache(&cell, &CellStyle::default()));
        }
    }
}

#[test]
fn compatible_and_absent_carriers_keep_formula_cache_eligibility() {
    for (kind, carrier) in [
        ("", ""),
        (" t=\"n\"", "<v/>"),
        (" t=\"n\"", "<v>3</v>"),
        (" t=\"inlineStr\"", ""),
        (" t=\"inlineStr\"", "<is><t>3</t></is>"),
        (" t=\"e\"", "<v>#VALUE!</v>"),
        (" t=\"str\"", "<v>cache</v>"),
    ] {
        let xml = cell_xml(kind, carrier, "<f>1+2</f>");
        let cell = parse_xml(xml.as_bytes()).unwrap();
        assert!(!c_unknown(&cell), "{kind} {carrier}");
        assert!(ignorable_formula_cache(&cell, &CellStyle::default()));
    }
}
