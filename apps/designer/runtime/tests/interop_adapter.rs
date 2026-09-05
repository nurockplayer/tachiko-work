use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::interop_adapter::*;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
fn simple() -> SourceWorkbook {
    import_csv(b"Name,Amount\nAda,12\n", &ImportOptions::default()).unwrap()
}

#[test]
fn decoded_attributes_reject_illegal_characters_including_namespaces() {
    let bytes = export_xlsx(&simple()).unwrap();
    for reference in ["&#0;", "&#1;", "&#xFFFE;", "&#65535;"] {
        for attribute in ["name", "xmlns:unused"] {
            let invalid = mutate(&bytes, "xl/workbook.xml", |s| {
                s.replace("<sheet ", &format!("<sheet {attribute}=\"{reference}\" "))
            });
            let error = import_xlsx(&invalid).unwrap_err();
            if reference == "&#xFFFE;" {
                assert!(error.0.contains("Invalid escaped XML attribute character"));
            }
        }
    }
    let valid = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("name=\"Imported table\"", "name=\"A&amp;B &#x1F600;\"")
    });
    let book = import_xlsx(&valid).unwrap();
    assert_eq!(book.sheets[0].name, "A&B 😀");
    assert_eq!(
        import_xlsx(&export_xlsx(&book).unwrap()).unwrap().sheets[0].name,
        "A&B 😀"
    );
}

#[test]
fn only_exact_referenced_worksheet_parts_are_structural() {
    fn add(bytes: &[u8], path: &str, content: &[u8]) -> Vec<u8> {
        let mut cursor = Cursor::new(bytes.to_vec());
        let mut writer = ZipWriter::new_append(&mut cursor).unwrap();
        writer
            .start_file(path, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
        writer.finish().unwrap();
        cursor.into_inner()
    }
    let bytes = export_xlsx(&simple()).unwrap();
    for (path, content) in [
        ("xl/worksheets/custom.bin", b"opaque".as_slice()),
        ("xl/worksheets/unreferenced.xml", b"<worksheet/>".as_slice()),
        ("xl/worksheets/Sheet1.xml", b"<worksheet/>".as_slice()),
        ("XL/workbook.xml", b"<workbook/>".as_slice()),
        (
            "xl/worksheets/_rels/unknown.xml.rels",
            b"<Relationships/>".as_slice(),
        ),
    ] {
        let book = import_xlsx(&add(&bytes, path, content)).unwrap();
        assert!(
            book.ledger
                .iter()
                .any(|f| f.code == "unknown_package_part" && f.location == path && f.blocking)
        );
        assert!(export_xlsx(&book).is_err());
    }
    let relationships = b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"link\" Type=\"hyperlink\" Target=\"https://example.invalid/\" TargetMode=\"External\"/></Relationships>";
    let path = "xl/worksheets/_rels/sheet1.xml.rels";
    let book = import_xlsx(&add(&bytes, path, relationships)).unwrap();
    assert!(
        book.ledger
            .iter()
            .any(|f| f.code == "worksheet_relationships_not_exported" && !f.blocking)
    );
    assert!(
        book.ledger
            .iter()
            .any(|f| f.code == "external_relationship")
    );
    assert!(import_xlsx(&add(&bytes, path, b"<Relationships/>")).is_err());
    let rooted = mutate(&bytes, "xl/_rels/workbook.xml.rels", |s| {
        s.replace("worksheets/sheet1.xml", "/sheet.xml")
    });
    let mut archive = ZipArchive::new(Cursor::new(rooted)).unwrap();
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = if entry.name() == "xl/worksheets/sheet1.xml" {
                "sheet.xml"
            } else {
                entry.name()
            }
            .to_owned();
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            std::io::copy(&mut entry, &mut writer).unwrap();
        }
        writer.finish().unwrap();
    }
    let book = import_xlsx(&add(
        &output.into_inner(),
        "_rels/sheet.xml.rels",
        relationships,
    ))
    .unwrap();
    assert!(!book.ledger.iter().any(|f| f.blocking));
    assert!(
        book.ledger
            .iter()
            .any(|f| f.code == "worksheet_relationships_not_exported"
                && f.location == "_rels/sheet.xml.rels")
    );
}

#[test]
fn shared_and_inline_strings_obey_the_same_rich_text_shapes() {
    let shared = include_bytes!("../../tests/fixtures/interop/reference-two-sheet.xlsx");
    let inline = export_xlsx(&simple()).unwrap();
    let rich = "<r><rPr><b/></rPr><t>La</t></r><r><t>bel</t></r><rPh sb=\"0\" eb=\"1\"><t>ignored</t></rPh>";
    let shared_rich = mutate(shared, "xl/sharedStrings.xml", |s| {
        s.replacen("<t xml:space=\"preserve\">Item</t>", rich, 1)
    });
    let inline_rich = mutate(&inline, "xl/worksheets/sheet1.xml", |s| {
        s.replacen("<t xml:space=\"preserve\">Name</t>", rich, 1)
    });
    for bytes in [&shared_rich, &inline_rich] {
        let book = import_xlsx(bytes).unwrap();
        assert_eq!(book.sheets[0].columns[0].name, "Label");
        assert!(!book.ledger.iter().any(|f| f.blocking));
    }
    for malformed in [
        "<t><r><t>nested</t></r></t>",
        "<unknown/>",
        "<r><t>one</t><t>two</t></r>",
    ] {
        let shared_bad = mutate(shared, "xl/sharedStrings.xml", |s| {
            s.replacen("<t xml:space=\"preserve\">Item</t>", malformed, 1)
        });
        assert!(import_xlsx(&shared_bad).is_err());
        let inline_bad = mutate(&inline, "xl/worksheets/sheet1.xml", |s| {
            s.replacen("<t xml:space=\"preserve\">Name</t>", malformed, 1)
        });
        assert!(
            import_xlsx(&inline_bad)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
    }
    let unknown = mutate(shared, "xl/sharedStrings.xml", |s| {
        s.replace("</sst>", "<unknown/></sst>")
    });
    assert!(import_xlsx(&unknown).is_err());
}

#[test]
fn formula_cache_cannot_bypass_cell_parent_child_shapes() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 3.0 };
    book.sheets[0].rows[0][1].formula = Some("1+2".into());
    let bytes = export_xlsx(&book).unwrap();
    for invalid in [
        "<f>1+2</f><v><f>9+9</f></v>",
        "<f>1+2<c><v>3</v></c></f><v>#VALUE!</v>",
        "<f>1+2</f><t>wrong parent</t>",
        "<f>1+2</f><is><v>wrong parent</v></is>",
        "<f>1+2</f><is><r><t>one</t><t>two</t></r></is>",
        "<f>1+2</f><is><t><r/></t></is>",
    ] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("t=\"n\"><f>", "t=\"e\"><f>")
                .replace("<f>1+2</f><v>3</v>", invalid)
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
        );
    }
    let rich = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<t xml:space=\"preserve\">Ada</t>", "<r><rPr><b/></rPr><t>A</t></r><r><t>da</t></r><rPh sb=\"0\" eb=\"1\"><t>ignored</t></rPh><phoneticPr fontId=\"0\"/>")
    });
    let imported = import_xlsx(&rich).unwrap();
    assert_eq!(
        imported.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "Ada".into()
        }
    );
    assert!(!imported.ledger.iter().any(|f| f.blocking));
}

#[test]
fn unusable_formula_caches_are_evidence_only_but_invalid_structure_stays_blocked() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 3.0 };
    book.sheets[0].rows[0][1].formula = Some("1+2".into());
    let bytes = export_xlsx(&book).unwrap();
    for cache in ["<v>not-a-number</v>", "", "<v>#VALUE!</v>"] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<v>3</v>", cache)
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(!imported.ledger.iter().any(|f| f.blocking));
        assert_eq!(
            imported.sheets[0].rows[0][1].formula.as_deref(),
            Some("1+2")
        );
        assert_eq!(imported.sheets[0].rows[0][1].value, SourceValue::Empty);
    }
    let error = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("t=\"n\"><f>", "t=\"e\"><f>")
            .replace("<v>3</v>", "<v>#VALUE!</v>")
    });
    assert!(
        !import_xlsx(&error)
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking)
    );
    for replacement in [
        "<f>SUM(1,2)</f>",
        "<f t=\"shared\"/>",
        "<f>1+2</f><unknown/>",
        "<f>1+2</f><f>4+5</f>",
        "<f>1+2</f><v>123</v>",
    ] {
        let changed = mutate(&error, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<f>1+2</f>", replacement)
        });
        assert!(
            import_xlsx(&changed)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
    }
    let scalar = mutate(&error, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<f>1+2</f>", "")
    });
    assert!(
        import_xlsx(&scalar)
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
    );
    let unknown_type = mutate(&error, "xl/worksheets/sheet1.xml", |s| {
        s.replace("t=\"e\"", "t=\"unknown\"")
    });
    assert!(
        import_xlsx(&unknown_type)
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking)
    );
    for format in ["yyyy-mm-dd", "h:mm", "[m]"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        let styled = export_xlsx(&book).unwrap();
        let invalid_cache = mutate(&styled, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<v>3</v>", "<v>invalid</v>")
        });
        assert!(
            import_xlsx(&invalid_cache)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
    }
}

#[test]
fn workbook_omissions_and_column_width_tails_are_explicit() {
    let bytes = export_xlsx(&simple()).unwrap();
    for (child, code, blocking) in [
        (
            "<workbookProtection lockStructure=\"1\"/>",
            "workbook_protection",
            false,
        ),
        ("<bookViews/>", "workbook_layout_rules", false),
        ("<futureFeature/>", "unknown_workbook_child", true),
        (
            "<sheets xmlns=\"urn:foreign\"/>",
            "unknown_workbook_child",
            true,
        ),
    ] {
        let changed = mutate(&bytes, "xl/workbook.xml", |s| {
            s.replace("</workbook>", &format!("{child}</workbook>"))
        });
        assert!(
            import_xlsx(&changed)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.code == code && f.blocking == blocking)
        );
    }
    for (min, max) in [(1, 16), (3, 16)] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(
                "<cols>",
                &format!("<cols><col min=\"{min}\" max=\"{max}\" width=\"24\"/>"),
            )
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.code == "column_width_outside_grid"
                    && f.category == FidelityCategory::LossyOnExport
                    && !f.blocking)
        );
        assert_eq!(
            imported.sheets[0].columns[0].width,
            if min == 1 { Some(24.0) } else { None }
        );
    }
    let oversized = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<cols>", "<cols><col min=\"1\" max=\"17\" width=\"24\"/>")
    });
    assert!(import_xlsx(&oversized).is_err());
    let duplicate = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("</workbook>", "<sheets/></workbook>")
    });
    assert!(import_xlsx(&duplicate).is_err());
    let unknown = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("</sheets>", "<unknown/></sheets>")
    });
    assert!(import_xlsx(&unknown).is_err());
    let invalid_epoch = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("date1904=\"0\"", "date1904=\"maybe\"")
    });
    assert!(import_xlsx(&invalid_epoch).is_err());
}

#[test]
fn multiple_column_groups_are_all_inspected_and_singleton_duplicates_rejected() {
    let bytes = export_xlsx(&simple()).unwrap();
    for (attribute, code) in [
        ("hidden=\"1\"", "hidden_column"),
        ("style=\"1\"", "inherited_column_style"),
    ] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(
                "</cols>",
                &format!("</cols><cols><col min=\"1\" max=\"2\" {attribute}/></cols>"),
            )
        });
        let book = import_xlsx(&changed).unwrap();
        assert!(book.ledger.iter().any(|f| f.code == code && f.blocking));
        assert!(export_xlsx(&book).is_err());
    }
    let width = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace(
            "</cols>",
            "</cols><cols><col min=\"2\" max=\"2\" width=\"24\"/></cols>",
        )
    });
    let book = import_xlsx(&width).unwrap();
    assert_eq!(book.sheets[0].columns[1].width, Some(24.0));
    assert!(!book.ledger.iter().any(|f| f.blocking));
    for extra in [
        "<sheetData/>",
        "<dimension ref=\"A1\"/><dimension ref=\"B2\"/>",
        "<sheetFormatPr/><sheetFormatPr zeroHeight=\"1\"/>",
    ] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("</worksheet>", &format!("{extra}</worksheet>"))
        });
        assert!(
            import_xlsx(&changed)
                .unwrap_err()
                .0
                .contains("Duplicate singleton")
        );
    }
    let unknown = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("</cols>", "</cols><cols><unknown/></cols>")
    });
    assert!(import_xlsx(&unknown).is_err());
}

#[test]
fn integral_time_only_formats_never_become_dates_or_numbers() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 0.0 };
    for format in ["h:mm", "h:mm:ss", "[m]", "[mm]"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        let bytes = export_xlsx(&book).unwrap();
        let imported = import_xlsx(&bytes).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
        );
        assert_eq!(imported.sheets[0].rows[0][1].value, SourceValue::Empty);
    }
    book.sheets[0].rows[0][1].style.number_format = None;
    let bytes = export_xlsx(&book).unwrap();
    for id in 18..=22 {
        let changed = mutate(&bytes, "xl/styles.xml", |s| {
            s.replace("<xf numFmtId=\"164\"", &format!("<xf numFmtId=\"{id}\""))
        });
        let imported = import_xlsx(&changed).unwrap();
        if id == 22 {
            assert_eq!(
                imported.sheets[0].rows[0][1].value,
                SourceValue::Date {
                    value: "1899-12-31".into()
                }
            );
        } else {
            assert!(
                imported
                    .ledger
                    .iter()
                    .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
            );
            assert_eq!(imported.sheets[0].rows[0][1].value, SourceValue::Empty);
        }
    }
}

#[test]
fn every_unmapped_worksheet_child_has_a_loss_or_blocking_inventory() {
    let bytes = export_xlsx(&simple()).unwrap();
    for (child, code, blocking) in [
        (
            "<sheetProtection sheet=\"1\"/>",
            "worksheet_protection",
            false,
        ),
        (
            "<hyperlinks><hyperlink ref=\"A2\" location=\"B2\"/></hyperlinks>",
            "worksheet_hyperlinks",
            false,
        ),
        (
            "<pageSetup orientation=\"landscape\"/>",
            "worksheet_layout_rules",
            false,
        ),
        ("<unknownSemanticFeature/>", "unknown_worksheet_child", true),
        (
            "<sheetData xmlns=\"urn:foreign\"/>",
            "unknown_worksheet_child",
            true,
        ),
    ] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("</worksheet>", &format!("{child}</worksheet>"))
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.code == code && f.blocking == blocking)
        );
        if blocking {
            assert!(export_xlsx(&imported).is_err());
        }
    }
    for (before, after, code) in [
        (
            "<row r=\"2\">",
            "<row r=\"2\" s=\"1\" customFormat=\"1\">",
            "inherited_row_style",
        ),
        (
            "<cols>",
            "<cols><col min=\"1\" max=\"2\" style=\"1\"/>",
            "inherited_column_style",
        ),
    ] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(before, after)
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(imported.ledger.iter().any(|f| f.code == code && f.blocking));
        assert!(export_xlsx(&imported).is_err());
    }
    for (attribute, blocking) in [("ht=\"40\"", false), ("unknownMode=\"1\"", true)] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<row r=\"2\">", &format!("<row r=\"2\" {attribute}>"))
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.code == "unmapped_grid_attribute" && f.blocking == blocking)
        );
    }
}

#[test]
fn cdata_obeys_xml_characters_and_root_boundaries() {
    let bytes = export_xlsx(&simple()).unwrap();
    for text in ["\u{0}", "\u{1}", "\u{fffe}", "\u{ffff}"] {
        let invalid = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(
                "<t xml:space=\"preserve\">Ada</t>",
                &format!("<t><![CDATA[{text}]]></t>"),
            )
        });
        assert_ne!(invalid, bytes);
        assert!(import_xlsx(&invalid).is_err());
    }
    for text in ["", " ", "outside"] {
        for before in [true, false] {
            let invalid = mutate(&bytes, "xl/workbook.xml", |s| {
                if before {
                    format!("<![CDATA[{text}]]>{s}")
                } else {
                    format!("{s}<![CDATA[{text}]]>")
                }
            });
            assert!(
                import_xlsx(&invalid)
                    .unwrap_err()
                    .0
                    .contains("CDATA outside XML root")
            );
        }
    }
    let valid = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace(
            "<t xml:space=\"preserve\">Ada</t>",
            "<t><![CDATA[Ada & <literal>]]></t>",
        )
    });
    assert_eq!(
        import_xlsx(&valid).unwrap().sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "Ada & <literal>".into()
        }
    );
}

#[test]
fn hidden_source_content_is_blocked_with_explicit_visibility_inventory() {
    let bytes = export_xlsx(&simple()).unwrap();
    for state in ["hidden", "veryHidden"] {
        let hidden = mutate(&bytes, "xl/workbook.xml", |s| {
            s.replace("<sheet ", &format!("<sheet state=\"{state}\" "))
        });
        let original = hidden.clone();
        let book = import_xlsx(&hidden).unwrap();
        assert!(book.ledger.iter().any(|f| f.code == "hidden_sheet"
            && f.blocking
            && f.category == FidelityCategory::UnsupportedSafeDisabled));
        assert!(export_xlsx(&book).is_err());
        assert_eq!(hidden, original);
    }
    for (xml, code) in [
        ("<sheetData><row r=\"1\" hidden=\"true\">", "hidden_row"),
        (
            "<cols><col min=\"1\" max=\"2\" hidden=\"1\"/></cols><sheetData><row r=\"1\">",
            "hidden_column",
        ),
        (
            "<sheetFormatPr zeroHeight=\"true\"/><sheetData><row r=\"1\">",
            "hidden_default_rows",
        ),
    ] {
        let hidden = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<cols></cols>", "")
                .replace("<sheetData><row r=\"1\">", xml)
        });
        assert_ne!(hidden, bytes);
        let book = import_xlsx(&hidden).unwrap();
        assert!(book.ledger.iter().any(|f| f.code == code && f.blocking));
        assert!(export_xlsx(&book).is_err());
    }
    let unknown = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("<sheet ", "<sheet state=\"unknown\" ")
    });
    assert!(import_xlsx(&unknown).is_err());
    let unknown = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<row ", "<row hidden=\"unknown\" ")
    });
    assert!(import_xlsx(&unknown).is_err());
    let visible = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("<sheet ", "<sheet state=\"visible\" ")
    });
    let visible = mutate(&visible, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<row ", "<row hidden=\"false\" ")
    });
    assert!(
        !import_xlsx(&visible)
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking)
    );
}
fn mutate(bytes: &[u8], name: &str, f: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        entries.push((file.name().to_owned(), s));
    }
    let target = entries.iter_mut().find(|(path, _)| path == name).unwrap();
    target.1 = f(target.1.clone());
    let mut result = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut result);
        for (path, contents) in entries {
            writer
                .start_file(
                    path,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    result.into_inner()
}
#[test]
fn csv_quotes_blank_positions_and_roundtrip() {
    let input = b"Name,Value,Note\r\n00123,,\"one\ntwo,three\"\r\n";
    let book = import_csv(input, &ImportOptions::default()).unwrap();
    assert_eq!(
        book.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "00123".into()
        }
    );
    assert_eq!(book.sheets[0].rows[0][1].value, SourceValue::Empty);
    let output = export_csv(&book.sheets[0]).unwrap();
    let again = import_csv(&output, &ImportOptions::default()).unwrap();
    assert_eq!(book.sheets, again.sheets);
}
#[test]
fn typed_multiple_sheet_writer_roundtrip() {
    let mut book = import_csv(b"Name,Amount\nAda,12\n", &ImportOptions::default()).unwrap();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 12.0 };
    let mut second = book.sheets[0].clone();
    second.name = "Second".into();
    second.rows[0][1].formula = Some("'Imported table'!B2*2".into());
    second.rows[0][1].value = SourceValue::Number { value: 24.0 };
    book.sheets.push(second);
    let bytes = export_xlsx(&book).unwrap();
    let again = import_xlsx(&bytes).unwrap();
    assert_eq!(again.sheets.len(), 2);
    assert_eq!(
        again.sheets[1].rows[0][1].formula,
        Some("'Imported table'!B2*2".into())
    );
    assert!(!again.ledger.iter().any(|f| f.blocking));
}
#[test]
fn shared_selfclosing_and_unsupported_functions_are_blocking() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 777.0 };
    book.sheets[0].rows[0][1].formula = Some("MIN(1,2)".into());
    let bytes = export_xlsx(&book).unwrap();
    let bytes = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("MIN(1,2)", "SUM(1,2)")
    });
    let imported = import_xlsx(&bytes).unwrap();
    assert!(
        imported
            .ledger
            .iter()
            .any(|f| f.blocking && f.code == "unsupported_formula_function")
    );
    let bytes = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<f>SUM(1,2)</f>", "<f t=\"shared\" si=\"0\"/>")
    });
    let imported = import_xlsx(&bytes).unwrap();
    assert!(
        imported
            .ledger
            .iter()
            .any(|f| f.blocking && f.code == "shared_array_dynamic_formula")
    );
    assert_eq!(imported.sheets[0].rows[0][1].value, SourceValue::Empty);
}
#[test]
fn serial_dates_reject_fictitious_day_and_time_without_truncation() {
    let mut book = simple();
    book.sheets[0].rows[0][1].style.number_format = Some("yyyy-mm-dd".into());
    for serial in [60.0, 46270.5] {
        book.sheets[0].rows[0][1].value = SourceValue::Number { value: serial };
        let imported = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
        );
    }
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 46270.0 };
    let bytes = export_xlsx(&book).unwrap();
    assert_eq!(
        import_xlsx(&bytes).unwrap().sheets[0].rows[0][1].value,
        SourceValue::Date {
            value: "2026-09-05".into()
        }
    );
    let bytes = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("date1904=\"0\"", "date1904=\"1\"")
    });
    let bytes = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<v>46270</v>", "<v>44808</v>")
    });
    assert_eq!(
        import_xlsx(&bytes).unwrap().sheets[0].rows[0][1].value,
        SourceValue::Date {
            value: "2026-09-05".into()
        }
    );
}
#[test]
fn boolean_constants_are_converted_with_evidence() {
    let mut book = simple();
    book.sheets[0].rows[0][1].formula = Some("TRUE()".into());
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 1.0 };
    let imported = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
    assert_eq!(
        imported.sheets[0].rows[0][1].value,
        SourceValue::Boolean { value: true }
    );
    assert!(imported.sheets[0].rows[0][1].formula.is_none());
    assert!(
        imported
            .ledger
            .iter()
            .any(|f| f.code == "boolean_constant_formula" && !f.blocking)
    );
}
#[test]
fn dtd_invalid_namespace_and_extreme_coordinate_fail_before_materialization() {
    let bytes = export_xlsx(&simple()).unwrap();
    for changed in [
        mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            format!("<!DOCTYPE worksheet [<!ENTITY a 'x'>]>{s}")
        }),
        mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("r=\"B2\"", "r=\"ZZZZZZZZ99999999\"")
        }),
        mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
                "urn:wrong",
            )
        }),
    ] {
        assert!(import_xlsx(&changed).is_err());
    }
}
#[test]
fn sparse_source_rows_preserve_formula_coordinates() {
    let bytes = export_xlsx(&simple()).unwrap();
    let bytes = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace("<row r=\"2\">", "<row r=\"3\">")
            .replace("r=\"A2\"", "r=\"A3\"")
            .replace("r=\"B2\"", "r=\"B3\"")
    });
    let imported = import_xlsx(&bytes).unwrap();
    assert_eq!(imported.sheets[0].rows.len(), 2);
    assert!(
        imported.sheets[0].rows[0]
            .iter()
            .all(|c| c.value == SourceValue::Empty)
    );
    assert_eq!(
        imported.sheets[0].rows[1][0].value,
        SourceValue::Text {
            value: "Ada".into()
        }
    );
}
#[test]
fn checked_in_profile_limits_match_adapter() {
    let profile: serde_json::Value =
        serde_json::from_str(include_str!("../../interop-profile.json")).unwrap();
    let l = &profile["limits"];
    for (key, value) in [
        ("source_bytes", MAX_SOURCE_BYTES),
        ("expanded_bytes", MAX_EXPANDED_BYTES),
        ("zip_entries", MAX_ZIP_ENTRIES),
        ("sheets", MAX_SHEETS),
        ("columns_per_sheet", MAX_COLUMNS),
        ("data_rows_per_sheet", MAX_DATA_ROWS),
        ("formulas", MAX_FORMULAS),
    ] {
        assert_eq!(l[key], value);
    }
}
#[test]
fn real_reference_and_prefixed_workbooks_preserve_typed_data_and_formulas() {
    for bytes in [
        include_bytes!("../../tests/fixtures/interop/reference-two-sheet.xlsx").as_slice(),
        include_bytes!("../../tests/fixtures/interop/ordinary-two-sheet.xlsx").as_slice(),
    ] {
        let book = import_xlsx(bytes).unwrap();
        assert!(!book.ledger.iter().any(|f| f.blocking), "{:?}", book.ledger);
        assert_eq!(book.sheets.len(), 2);
        assert_eq!(
            book.sheets[0].rows[0][0].value,
            SourceValue::Text {
                value: "00123".into()
            }
        );
        assert_eq!(
            book.sheets[0].rows[0][2].value,
            SourceValue::Boolean { value: true }
        );
        assert_eq!(
            book.sheets[0].rows[0][3].value,
            SourceValue::Date {
                value: "2026-09-05".into()
            }
        );
        assert_eq!(
            book.sheets[0].rows[0][4].value,
            SourceValue::Number { value: 0.15 }
        );
        assert!(
            book.sheets[0].rows[0][4]
                .style
                .number_format
                .as_ref()
                .unwrap()
                .contains('%')
        );
        assert_eq!(
            book.sheets[0].rows[0][8].value,
            SourceValue::Number { value: 240.0 }
        );
        assert!(
            book.sheets[0].rows[0][8]
                .formula
                .as_ref()
                .unwrap()
                .contains("Rates")
        );
        let written = export_xlsx(&book).unwrap();
        let again = import_xlsx(&written).unwrap();
        for (left, right) in book.sheets.iter().zip(&again.sheets) {
            assert_eq!(left.name, right.name);
            for (a, b) in left.rows.iter().flatten().zip(right.rows.iter().flatten()) {
                assert_eq!(a.value, b.value);
                assert_eq!(a.formula, b.formula);
            }
        }
    }
}
#[test]
fn checked_in_hostile_inventory_is_complete_and_semantic_findings_block() {
    let cases: [(&[u8], &[&str]); 5] = [
        (
            include_bytes!("../../tests/fixtures/interop/hostile/01-shared-selfclosing.xlsx"),
            &["shared_array_dynamic_formula"],
        ),
        (
            include_bytes!("../../tests/fixtures/interop/hostile/02-external-dde.xlsx"),
            &["external_or_dde_formula", "external_relationship"],
        ),
        (
            include_bytes!("../../tests/fixtures/interop/hostile/03-disabled-parts.xlsx"),
            &[
                "macro_disabled",
                "activex_disabled",
                "ole_disabled",
                "pivot_unsupported",
                "chart_unsupported",
            ],
        ),
        (
            include_bytes!("../../tests/fixtures/interop/hostile/04-table-validation-names.xlsx"),
            &[
                "table_rules_unsupported",
                "validation_rules",
                "filter_rules",
                "defined_names",
            ],
        ),
        (
            include_bytes!("../../tests/fixtures/interop/hostile/05-date-boundaries.xlsx"),
            &["scalar_mapping_rejected"],
        ),
    ];
    for (i, (bytes, codes)) in cases.into_iter().enumerate() {
        let book = import_xlsx(bytes).unwrap();
        for code in codes {
            assert!(
                book.ledger.iter().any(|f| &f.code == code),
                "missing {code}"
            );
        }
        if [0, 1, 4].contains(&i) {
            assert!(book.ledger.iter().any(|f| f.blocking));
            assert!(export_xlsx(&book).is_err());
        }
    }
    for bytes in [
        include_bytes!("../../tests/fixtures/interop/hostile/06-oversize-address.xlsx").as_slice(),
        include_bytes!("../../tests/fixtures/interop/hostile/07-duplicate-zip-entry.xlsx")
            .as_slice(),
    ] {
        assert!(import_xlsx(bytes).is_err());
    }
}
#[test]
fn expanded_limit_is_checked_before_xml_materialization() {
    let bytes = export_xlsx(&simple()).unwrap();
    let large = mutate(&bytes, "xl/worksheets/sheet1.xml", |_| {
        format!("<x>{}</x>", "a".repeat(MAX_EXPANDED_BYTES + 1))
    });
    assert!(large.len() < MAX_SOURCE_BYTES);
    assert!(import_xlsx(&large).unwrap_err().0.contains("Expanded ZIP"));
}
#[test]
fn zip_path_entry_count_and_source_bounds_fail_closed() {
    for (path, count) in [("../unsafe.xml", 1), ("part.xml", MAX_ZIP_ENTRIES + 1)] {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut bytes);
            for i in 0..count {
                let name = if count == 1 {
                    path.to_owned()
                } else {
                    format!("{i}.xml")
                };
                writer
                    .start_file(name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(b"<x/>").unwrap();
            }
            writer.finish().unwrap();
        }
        assert!(import_xlsx(&bytes.into_inner()).is_err());
    }
    assert!(import_xlsx(&vec![0; MAX_SOURCE_BYTES + 1]).is_err());
}
#[test]
fn unknown_parts_are_blocking_and_xml_namespace_aliases_cannot_hide_duplicate_attributes() {
    let book = import_xlsx(include_bytes!(
        "../../tests/fixtures/interop/hostile/08-expanded-limit.xlsx"
    ))
    .unwrap();
    assert!(
        book.ledger
            .iter()
            .any(|f| f.code == "unknown_package_part" && f.blocking)
    );
    let bytes = export_xlsx(&simple()).unwrap();
    let bytes = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("<sheets>","<sheets xmlns:alias=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">").replace("r:id=\"rId1\"","r:id=\"rId1\" alias:id=\"other\"")
    });
    assert!(
        import_xlsx(&bytes)
            .unwrap_err()
            .0
            .contains("Duplicate expanded")
    );
}

#[test]
fn csv_does_not_activate_literal_text_as_external_formulas() {
    for text in ["=1+1", "+1", "-1", "@SUM(A1)", "\t=1", "\r=1", "  =1"] {
        let mut book = simple();
        book.sheets[0].rows[0][0].value = SourceValue::Text { value: text.into() };
        assert!(
            export_csv(&book.sheets[0])
                .unwrap_err()
                .0
                .contains("literal Text")
        );
        let reimport = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
        assert_eq!(
            reimport.sheets[0].rows[0][0].value,
            book.sheets[0].rows[0][0].value
        );
        book.sheets[0].rows[0][0].value = SourceValue::Text {
            value: "safe".into(),
        };
        book.sheets[0].columns[0].name = text.into();
        assert!(export_csv(&book.sheets[0]).is_err());
    }
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: -1.0 };
    assert!(
        String::from_utf8(export_csv(&book.sheets[0]).unwrap())
            .unwrap()
            .contains("Ada,-1")
    );
}

#[test]
fn hostile_fixture_manifest_matches_archive_entry_sizes() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/interop/hostile");
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/interop/hostile/inventory.json"
    ))
    .unwrap();
    assert_eq!(manifest["source_file"], "../reference-two-sheet.xlsx");
    let fixtures = manifest["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 8);
    let mut paths = vec![root.join(manifest["source_file"].as_str().unwrap())];
    paths.extend(
        fixtures
            .iter()
            .map(|f| root.join(f["file"].as_str().unwrap())),
    );
    for (index, fixture) in fixtures.iter().enumerate() {
        let bytes = std::fs::read(&paths[index + 1]).unwrap();
        assert_eq!(
            bytes.len() as u64,
            fixture["compressed_bytes"].as_u64().unwrap()
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        // zip collapses duplicate names. Fixture 07 contains two identical workbook.xml entries
        // (1417 bytes each); account for its hidden duplicate without changing archive admission.
        let duplicate = fixture["file"] == "07-duplicate-zip-entry.xlsx";
        let duplicate_count = u64::from(duplicate);
        assert_eq!(
            archive.len() as u64 + duplicate_count,
            fixture["entry_count"].as_u64().unwrap()
        );
        let expanded: u64 = (0..archive.len())
            .map(|entry| archive.by_index(entry).unwrap().size())
            .sum();
        if duplicate {
            assert_eq!(archive.by_name("xl/workbook.xml").unwrap().size(), 1417);
        }
        assert_eq!(
            expanded + duplicate_count * 1417,
            fixture["expanded_bytes"].as_u64().unwrap()
        );
        if fixture["file"] == "08-expanded-limit.xlsx" {
            assert!(expanded < MAX_EXPANDED_BYTES as u64);
            let part = archive.by_name("xl/synthetic-expanded.xml").unwrap();
            assert_eq!(part.size(), 524_311);
            assert_eq!(part.compressed_size(), 549);
            assert_eq!(
                fixture["expected_inventory"][0]["shipped_expanded_limit_bytes"],
                MAX_EXPANDED_BYTES,
            );
        }
    }
}
