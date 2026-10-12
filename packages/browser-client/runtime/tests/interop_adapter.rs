use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::interop_adapter::{
    FidelityCategory, ImportOptions, MAX_COLUMNS, MAX_DATA_ROWS, MAX_EXPANDED_BYTES, MAX_FORMULAS,
    MAX_SHEETS, MAX_SOURCE_BYTES, MAX_ZIP_ENTRIES, SourceValue, SourceWorkbook, export_csv,
    export_xlsx, import_csv, import_xlsx,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
fn simple() -> SourceWorkbook {
    import_csv(b"Name,Amount\nAda,12\n", &ImportOptions::default()).unwrap()
}

fn capacity_book() -> SourceWorkbook {
    let source = format!("a,b,c\n{}", "one,two,three\n".repeat(65));
    import_csv(source.as_bytes(), &ImportOptions::default()).unwrap()
}

#[test]
fn capacity_xlsx_worksheet_compression_is_independent_of_metadata() {
    let mut book = capacity_book();
    book.sheets[0].rows[0][0].value = SourceValue::Text {
        value: "CR\r\nLF & <".into(),
    };
    let first = export_xlsx(&book).unwrap();
    book.sheets[0].name = "metadata\n\t\r".into();
    for (column, name) in book.sheets[0]
        .columns
        .iter_mut()
        .zip(["a & <", "b\r\n", "c\t"])
    {
        column.name = name.into();
    }
    let second = export_xlsx(&book).unwrap();
    let worksheet = |bytes: &[u8]| {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entry = archive.by_name("xl/worksheets/sheet1.xml").unwrap();
        let size = entry.compressed_size();
        let mut contents = String::new();
        entry.read_to_string(&mut contents).unwrap();
        (size, contents)
    };
    assert_eq!(worksheet(&first), worksheet(&second));
    let mut archive = ZipArchive::new(Cursor::new(&second)).unwrap();
    for path in ["xl/workbook.xml", "xl/sharedStrings.xml"] {
        assert_eq!(
            archive.by_name(path).unwrap().compression(),
            zip::CompressionMethod::Stored
        );
    }
    let parsed = import_xlsx(&second).unwrap();
    assert_eq!(parsed.sheets[0].name, book.sheets[0].name);
    assert_eq!(parsed.sheets[0].columns, book.sheets[0].columns);
    assert_eq!(
        parsed.sheets[0].rows[0][0].value,
        book.sheets[0].rows[0][0].value
    );
}

#[test]
fn capacity_source_cannot_hide_header_style_or_out_of_grid_width() {
    let bytes = export_xlsx(&capacity_book()).unwrap();
    let styled = mutate(&bytes, "xl/styles.xml", |xml| {
        xml.replace("<fonts count=\"1\">", "<fonts count=\"2\">")
            .replace("</fonts>", "<font><b/></font></fonts>")
            .replace("<cellXfs count=\"1\">", "<cellXfs count=\"2\">")
            .replace("</cellXfs>", "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs>")
    });
    let styled = mutate(&styled, "xl/worksheets/sheet1.xml", |xml| {
        xml.replace("r=\"A1\" s=\"0\"", "r=\"A1\" s=\"1\"")
    });
    assert!(
        import_xlsx(&styled)
            .unwrap_err()
            .0
            .contains("default source headers")
    );
    let width = mutate(&bytes, "xl/worksheets/sheet1.xml", |xml| {
        xml.replace(
            "<sheetData>",
            "<cols><col min=\"4\" max=\"4\" width=\"12\"/></cols><sheetData>",
        )
    });
    assert!(import_xlsx(&width).unwrap_err().0.contains("column widths"));
    // Existing small generic source-only losses keep their original behavior.
    let small = export_xlsx(&simple()).unwrap();
    let width = mutate(&small, "xl/worksheets/sheet1.xml", |xml| {
        xml.replace(
            "<sheetData>",
            "<cols><col min=\"4\" max=\"4\" width=\"12\"/></cols><sheetData>",
        )
    });
    let inspected = import_xlsx(&width).unwrap();
    assert!(
        inspected
            .ledger
            .iter()
            .any(|finding| finding.code == "column_width_outside_grid" && !finding.blocking)
    );
}

#[test]
fn existing_64_row_text_source_keeps_generic_styles_and_long_headers() {
    let source = format!(
        "{},b,c\n{}",
        "generic-label".repeat(8),
        "one,two,three\n".repeat(64)
    );
    let mut book = import_csv(source.as_bytes(), &ImportOptions::default()).unwrap();
    book.sheets[0].rows[0][0].style.bold = true;
    book.sheets[0].columns[1].width = Some(12.0);
    let parsed = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
    assert_eq!(parsed.sheets[0].rows.len(), 64);
    assert_eq!(parsed.sheets[0].columns, book.sheets[0].columns);
    assert!(parsed.sheets[0].rows[0][0].style.bold);
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
}

fn capacity_source_admits(bytes: &[u8]) -> bool {
    let Ok(book) = import_xlsx(bytes) else {
        return false;
    };
    tachiko_designer_runtime::import_workbook(
        &book,
        &tachiko_designer_runtime::ImportSelection {
            column_types: vec![vec![tachiko_designer_runtime::ImportFieldType::Text; 3]],
            extra_columns: vec![vec![]],
        },
        "00000000-0000-4000-8000-000000000000",
    )
    .is_ok()
}

fn plain_source_losses() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("xl/styles.xml", "<font>", "<font><i/>"),
        ("xl/styles.xml", "name val=\"Arial\"", "name val=\"Other\""),
        ("xl/styles.xml", "<font>", "<font><color theme=\"1\"/>"),
        (
            "xl/styles.xml",
            "<xf numFmtId",
            "<xf unknown=\"1\" numFmtId",
        ),
        (
            "xl/sharedStrings.xml",
            "<t xml:space=\"preserve\">a</t>",
            "<r><t>a</t></r>",
        ),
        (
            "xl/worksheets/sheet1.xml",
            "<c r=\"A2\"",
            "<c unknown=\"1\" r=\"A2\"",
        ),
        ("xl/workbook.xml", "<sheets>", "<sheets unknown=\"1\">"),
        (
            "[Content_Types].xml",
            "spreadsheetml.sheet.main+xml",
            "spreadsheetml.styles+xml",
        ),
        ("_rels/.rels", "<Relationships ", "<sst "),
        ("xl/_rels/workbook.xml.rels", "/worksheet\"", "/styles\""),
        (
            "xl/worksheets/sheet1.xml",
            "<sheetData>",
            "<sheetData>\u{a0}",
        ),
        ("xl/workbook.xml", "sheetId=\"1\"", "sheetId=\"0\""),
        ("xl/sharedStrings.xml", ">a</t>", ">a\rb</t>"),
        ("xl/sharedStrings.xml", ">a</t>", "><![CDATA[a\r\nb]]></t>"),
        ("xl/worksheets/sheet1.xml", "<v>0</v>", "<v>0\r</v>"),
        (
            "xl/workbook.xml",
            "name=\"Imported table\"",
            "name=\"a\tb\"",
        ),
        (
            "xl/workbook.xml",
            "name=\"Imported table\"",
            "name=\"a\nb\"",
        ),
        (
            "xl/workbook.xml",
            "name=\"Imported table\"",
            "name=\"a\rb\"",
        ),
        (
            "xl/workbook.xml",
            "<sheets>",
            "<sheets xmlns:xml=\"urn:wrong\">",
        ),
        (
            "xl/workbook.xml",
            "<sheets>",
            "<sheets xmlns:other=\"http://www.w3.org/XML/1998/namespace\">",
        ),
        (
            "xl/workbook.xml",
            "<sheets>",
            "<sheets xmlns:xmlns=\"urn:wrong\">",
        ),
    ]
}

#[test]
fn capacity_requires_complete_plain_source_at_small_and_large_sizes() {
    for rows in [8, 8406] {
        let source = format!("a,b,c\n{}", "one,two,three\n".repeat(rows));
        let mut book = import_csv(source.as_bytes(), &ImportOptions::default()).unwrap();
        if rows == 8 {
            for cell in book.sheets[0].rows.iter_mut().flatten().take(16) {
                cell.value = SourceValue::Text {
                    value: "v".repeat(4096),
                };
            }
        }
        let bytes = export_xlsx(&book).unwrap();
        assert!(capacity_source_admits(&bytes), "default {rows}");
        let aliased = mutate(&bytes, "xl/styles.xml", |xml| {
            xml.replace("<styleSheet ", "<q:styleSheet xmlns:q=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" ")
                .replace("</styleSheet>", "</q:styleSheet>")
                .replace("numFmtId=\"0\" fontId=\"0\"", "fontId=\"0\" numFmtId=\"0\"")
        });
        assert!(
            capacity_source_admits(&aliased),
            "style prefix/order {rows}"
        );
        let wrong_root = mutate(&bytes, "[Content_Types].xml", |_| {
            "<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"0\" uniqueCount=\"0\"/>".into()
        });
        assert!(
            !capacity_source_admits(&wrong_root),
            "valid wrong root {rows}"
        );
        for (path, from, to) in plain_source_losses() {
            let changed = mutate(&bytes, path, |xml| {
                assert!(xml.contains(from), "fixture lacks {from}");
                xml.replace(from, to)
            });
            assert!(!capacity_source_admits(&changed), "{rows}: {path}: {to}");
        }

        for prefix in [
            "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>",
            "<?xml version=\"1.1\"?>",
            "<?xml version=\"1.0\" unknown=\"1\"?>",
            " <?xml version=\"1.0\"?>",
            "<?XML version=\"1.0\"?>",
            "<!-- bad -- comment -->",
            "<!-- harmless -->",
            "<?app harmless?>",
            "\u{a0}",
            "&#32;",
        ] {
            let changed = mutate(&bytes, "xl/sharedStrings.xml", |xml| {
                format!("{prefix}{}", xml.replace(">a</t>", ">é</t>"))
            });
            assert!(
                !capacity_source_admits(&changed),
                "document {rows}: {prefix}"
            );
        }
        let declared = mutate(&bytes, "xl/sharedStrings.xml", |xml| {
            format!("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\r\n{xml}\r\n")
        });
        assert!(
            capacity_source_admits(&declared),
            "UTF-8 declaration {rows}"
        );
        for text in ["_x0041_", "_x000G_"] {
            let changed = mutate(&bytes, "xl/sharedStrings.xml", |xml| {
                xml.replace("count=\"3\"", "count=\"4\"")
                    .replace("uniqueCount=\"3\"", "uniqueCount=\"4\"")
                    .replace("</sst>", &format!("<si><t>{text}</t></si></sst>"))
            });
            assert_eq!(
                capacity_source_admits(&changed),
                text == "_x000G_",
                "unused shared string {rows}: {text}"
            );
        }
        let mut zip = ZipWriter::new_append(Cursor::new(bytes.clone())).unwrap();
        zip.start_file("docProps/custom.xml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"<Properties/>").unwrap();
        assert!(!capacity_source_admits(&zip.finish().unwrap().into_inner()));
        let references = mutate(&bytes, "xl/sharedStrings.xml", |xml| {
            xml.replace(">a</t>", ">a&#13;b</t>")
        });
        let references = mutate(&references, "xl/workbook.xml", |xml| {
            xml.replace("name=\"Imported table\"", "name=\"a&#9;b&#10;c&#13;\"")
                .replace("><", ">\r\n<")
        });
        assert!(capacity_source_admits(&references), "references {rows}");
        let generic = import_csv(b"a,b,c\none,two,three\n", &ImportOptions::default()).unwrap();
        let generic = export_xlsx(&generic).unwrap();
        let generic = mutate(&generic, "xl/styles.xml", |xml| {
            xml.replace("<font>", "<font><i/>")
        });
        let generic = import_xlsx(&generic).unwrap();
        assert!(generic.ledger.iter().any(|f| f.code == "style_profile"));
    }
}

#[test]
fn admitted_workbooks_pass_the_shared_output_style_predicate() {
    let mut book = simple();
    for color in ["a1B2c3", "FFA1B2C3"] {
        book.sheets[0].rows[0][1].style.fill = Some(color.into());
        let bytes = export_xlsx(&book).unwrap();
        let bytes = if color.len() == 6 {
            mutate(&bytes, "xl/styles.xml", |s| {
                s.replace(&format!("FF{color}"), color)
            })
        } else {
            bytes
        };
        let imported = import_xlsx(&bytes).unwrap();
        assert!(!imported.ledger.iter().any(|f| f.blocking));
        assert_eq!(
            imported.sheets[0].rows[0][1].style.fill.as_deref(),
            Some(color)
        );
        let reopened = import_xlsx(&export_xlsx(&imported).unwrap()).unwrap();
        let argb = if color.len() == 6 {
            format!("FF{color}")
        } else {
            color.into()
        };
        assert_eq!(
            reopened.sheets[0].rows[0][1].style.fill.as_deref(),
            Some(argb.as_str())
        );
        assert_eq!(
            reopened.sheets[0].rows[0][1].value,
            imported.sheets[0].rows[0][1].value
        );
    }
    let bytes = export_xlsx(&book).unwrap();
    for bad in ["FFFFGGGG", "1234567"] {
        let invalid = mutate(&bytes, "xl/styles.xml", |s| s.replace("FFA1B2C3", bad));
        let inspected = import_xlsx(&invalid).unwrap();
        assert!(inspected.ledger.iter().any(|f| f.blocking
            && f.code == "output_profile_rejected"
            && f.message.contains("RGB")));
        assert!(export_xlsx(&inspected).is_err());
        // Discarded header presentation remains a disclosed source-only loss.
        let header_only = mutate(&invalid, "xl/worksheets/sheet1.xml", |s| {
            s.replace("r=\"B2\" s=\"1\"", "r=\"B2\" s=\"0\"")
                .replace("r=\"B1\" s=\"0\"", "r=\"B1\" s=\"1\"")
        });
        let inspected = import_xlsx(&header_only).unwrap();
        assert!(!inspected.ledger.iter().any(|f| f.blocking));
        assert!(
            inspected
                .ledger
                .iter()
                .any(|f| f.code == "header_style_not_preserved")
        );
        assert!(export_xlsx(&inspected).is_ok());
        let already_blocked = mutate(&invalid, "xl/workbook.xml", |s| {
            s.replace("<sheet ", "<sheet state=\"hidden\" ")
        });
        let inspected = import_xlsx(&already_blocked).unwrap();
        assert!(
            inspected
                .ledger
                .iter()
                .any(|f| f.code == "hidden_sheet" && f.blocking)
        );
        assert!(
            !inspected
                .ledger
                .iter()
                .any(|f| f.code == "output_profile_rejected")
        );
    }
    // A separate output-style rule uses the same predicate at both boundaries.
    book.sheets[0].rows[0][1].style.number_format = Some("0;yyyy-mm-dd".into());
    assert!(export_xlsx(&book).is_err());
    assert!(
        import_xlsx(&source_with_format(&book))
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking)
    );
}

#[test]
fn formula_export_requires_numeric_presentation_for_every_cache_kind() {
    let mut book = simple();
    book.sheets[0].rows[0][1].formula = Some("1+2".into());
    for format in ["yyyy-mm-dd", "yyyy-mm-dd hh:mm:ss", "h:mm", "[m]"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        for cache in [
            SourceValue::Empty,
            SourceValue::Number { value: 3.0 },
            SourceValue::Text {
                value: "cached".into(),
            },
            SourceValue::Boolean { value: true },
        ] {
            book.sheets[0].rows[0][1].value = cache;
            assert!(
                export_xlsx(&book)
                    .unwrap_err()
                    .0
                    .contains("uniform numeric number format")
            );
        }
    }
    book.sheets[0].rows[0][1].style.number_format = Some("0.00".into());
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 3.0 };
    assert!(export_xlsx(&book).is_ok());
}

fn source_with_format(book: &SourceWorkbook) -> Vec<u8> {
    let mut legal = book.clone();
    let pattern = legal.sheets[0].rows[0][1]
        .style
        .number_format
        .replace("0.00".into())
        .unwrap();
    let bytes = export_xlsx(&legal).unwrap();
    let escaped = pattern
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    mutate(&bytes, "xl/styles.xml", |s| {
        s.replace("formatCode=\"0.00\"", &format!("formatCode=\"{escaped}\""))
    })
}

#[test]
fn numeric_format_sections_are_checked_without_guessing_the_active_section() {
    let mut book = simple();
    for pattern in [
        "0;[Red]-0;yyyy-mm-dd",
        "[>0]0;[<=0]yyyy-mm-dd",
        "0;h:mm",
        "yyyy-mm-dd;0",
        "0;0;0;0;0",
        "0\"unfinished",
        "0[Red",
        "0]",
        "0\\",
    ] {
        book.sheets[0].rows[0][1].style.number_format = Some(pattern.into());
        for value in [1.0, -1.0, 0.0] {
            book.sheets[0].rows[0][1].value = SourceValue::Number { value };
            assert!(export_xlsx(&book).is_err());
            let imported = import_xlsx(&source_with_format(&book)).unwrap();
            assert!(
                imported
                    .ledger
                    .iter()
                    .any(|f| f.blocking && f.code == "scalar_mapping_rejected"),
                "{pattern} / {value}"
            );
        }
    }
    for pattern in [
        "0;[Red]-0;0;yyyy-mm-dd",
        "0\";yyyy-mm-dd\"",
        "0\\;0",
        "0_;0",
        "0*;0",
        "0_d",
        "0*m",
    ] {
        book.sheets[0].rows[0][1].style.number_format = Some(pattern.into());
        book.sheets[0].rows[0][1].value = SourceValue::Number { value: -1234.5 };
        let imported = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
        assert!(!imported.ledger.iter().any(|f| f.blocking), "{pattern}");
        assert_eq!(
            imported.sheets[0].rows[0][1].value,
            SourceValue::Number { value: -1234.5 }
        );
    }
}

#[test]
fn mixed_formats_cannot_bypass_used_cell_or_formula_cache_admission() {
    let mut book = simple();
    book.sheets[0].rows[0][1].style.number_format = Some("0;[Red]-0;yyyy-mm-dd".into());
    book.sheets[0].rows[0][1].formula = Some("1+2".into());
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 3.0 };
    assert!(export_xlsx(&book).is_err());
    let bytes = source_with_format(&book);
    for cache in ["<v>3</v>", "<v>invalid</v>", ""] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<v>3</v>", cache)
        });
        assert!(
            import_xlsx(&changed)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
    }
    for (kind, cache) in [("str", "cached text"), ("b", "1"), ("e", "#VALUE!")] {
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace("t=\"n\"><f>", &format!("t=\"{kind}\"><f>"))
                .replace("<v>3</v>", &format!("<v>{cache}</v>"))
        });
        assert!(
            import_xlsx(&changed)
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
    }
    book.sheets[0].rows[0][1].formula = None;
    for value in [
        SourceValue::Empty,
        SourceValue::Text {
            value: "literal".into(),
        },
        SourceValue::Date {
            value: "2026-09-05".into(),
        },
    ] {
        book.sheets[0].rows[0][1].value = value;
        assert!(
            import_xlsx(&source_with_format(&book))
                .unwrap()
                .ledger
                .iter()
                .any(|f| f.blocking)
        );
        assert!(export_xlsx(&book).is_err());
    }
    book.sheets[0].rows[0][1].style.number_format = Some("yyyy-mm-dd [h]".into());
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 46270.0 };
    assert!(
        import_xlsx(&source_with_format(&book))
            .unwrap()
            .ledger
            .iter()
            .any(|f| f.blocking)
    );
}

#[test]
fn worksheet_names_share_import_export_boundaries() {
    let mut book = simple();
    let bytes = export_xlsx(&book).unwrap();
    let invalid = [
        String::new(),
        "名".repeat(32),
        "Bad[".into(),
        "Bad]".into(),
        "Bad:".into(),
        "Bad*".into(),
        "Bad?".into(),
        "Bad/".into(),
        "Bad\\".into(),
        "Bad\u{fffe}".into(),
    ];
    for name in invalid {
        book.sheets[0].name = name.clone();
        assert!(export_xlsx(&book).is_err());
        let changed = mutate(&bytes, "xl/workbook.xml", |s| {
            s.replace("name=\"Imported table\"", &format!("name=\"{name}\""))
        });
        assert!(import_xlsx(&changed).is_err());
    }
    for name in ["名".repeat(31), "😀".repeat(31), "A".into()] {
        book.sheets[0].name = name.clone();
        let imported = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
        assert_eq!(imported.sheets[0].name, name);
        assert_eq!(
            import_xlsx(&export_xlsx(&imported).unwrap())
                .unwrap()
                .sheets[0]
                .name,
            name
        );
    }
    book.sheets[0].name = "Ä".into();
    let mut second = book.sheets[0].clone();
    second.name = "Other".into();
    book.sheets.push(second);
    let bytes = export_xlsx(&book).unwrap();
    let duplicate = mutate(&bytes, "xl/workbook.xml", |s| {
        s.replace("name=\"Other\"", "name=\"ä\"")
    });
    assert!(import_xlsx(&duplicate).is_err());
    book.sheets[1].name = "ä".into();
    assert!(export_xlsx(&book).is_err());
}

#[test]
fn accounting_number_formats_preserve_defined_patterns_without_inventing_currency() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: -1234.5 };
    let bytes = export_xlsx(&book).unwrap();
    // Microsoft OpenXML NumberingFormat's All Languages implied formats.
    for (id, pattern) in [
        (37, "#,##0 ;(#,##0)"),
        (38, "#,##0 ;[Red](#,##0)"),
        (39, "#,##0.00;(#,##0.00)"),
        (40, "#,##0.00;[Red](#,##0.00)"),
    ] {
        let input = mutate(&bytes, "xl/styles.xml", |s| {
            s.replace("<xf numFmtId=\"164\"", &format!("<xf numFmtId=\"{id}\""))
        });
        let imported = import_xlsx(&input).unwrap();
        assert!(!imported.ledger.iter().any(|f| f.blocking));
        let cell = &imported.sheets[0].rows[0][1];
        assert_eq!(cell.value, SourceValue::Number { value: -1234.5 });
        assert_eq!(cell.style.number_format.as_deref(), Some(pattern));
        let reopened = import_xlsx(&export_xlsx(&imported).unwrap()).unwrap();
        assert_eq!(reopened.sheets[0].rows[0][1], *cell);
    }
    // These IDs have no locale-independent implied pattern: only explicit
    // formatCode establishes presentation. No USD inference is permitted.
    for id in [5, 6, 7, 8, 41, 42, 43, 44] {
        let undefined = mutate(&bytes, "xl/styles.xml", |s| {
            s.replace("<xf numFmtId=\"164\"", &format!("<xf numFmtId=\"{id}\""))
        });
        let rejected = import_xlsx(&undefined).unwrap();
        assert!(
            rejected
                .ledger
                .iter()
                .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
        );
        assert!(export_xlsx(&rejected).is_err());
        let pattern = "[$¥-411]#,##0.000;[Red](#,##0.000)";
        let defined = mutate(&bytes, "xl/styles.xml", |s| {
            s.replace("numFmtId=\"164\"", &format!("numFmtId=\"{id}\""))
                .replace(
                    "formatCode=\"General\"",
                    &format!("formatCode=\"{pattern}\""),
                )
        });
        let imported = import_xlsx(&defined).unwrap();
        assert!(!imported.ledger.iter().any(|f| f.blocking));
        let cell = &imported.sheets[0].rows[0][1];
        assert_eq!(cell.value, SourceValue::Number { value: -1234.5 });
        assert_eq!(cell.style.number_format.as_deref(), Some(pattern));
        let reopened = import_xlsx(&export_xlsx(&imported).unwrap()).unwrap();
        assert_eq!(reopened.sheets[0].rows[0][1], *cell);
    }
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

// #503: replace the complete cell, checking that the fixture actually reached A2.
fn scalar_carrier_fixture(cell: &str) -> Vec<u8> {
    let source = import_csv(b"Value\nresident\n", &ImportOptions::default()).unwrap();
    mutate(
        &export_xlsx(&source).unwrap(),
        "xl/worksheets/sheet1.xml",
        |xml| {
            let marker = "<c r=\"A2\"";
            assert_eq!(xml.matches(marker).count(), 1);
            let start = xml.find(marker).unwrap();
            let end = start + xml[start..].find("</c>").unwrap() + "</c>".len();
            format!("{}{cell}{}", &xml[..start], &xml[end..])
        },
    )
}

#[test]
fn incompatible_scalar_carriers_are_blocking_before_empty_defaults() {
    for cell in [
        "<c r=\"A2\" t=\"n\"><is><t>do-not-drop</t></is></c>",
        "<c r=\"A2\"><is><t>do-not-drop</t></is></c>",
        "<c r=\"A2\" t=\"inlineStr\"><v>123</v></c>",
        "<c r=\"A2\" t=\"str\"><is><t>do-not-drop</t></is></c>",
    ] {
        let inspected = import_xlsx(&scalar_carrier_fixture(cell)).unwrap();
        assert!(
            inspected
                .ledger
                .iter()
                .any(|finding| { finding.blocking && finding.code == "scalar_mapping_rejected" }),
            "incompatible carrier was silently accepted: {cell}"
        );
        assert!(
            tachiko_designer_runtime::import_workbook(
                &inspected,
                &tachiko_designer_runtime::ImportSelection {
                    column_types: vec![vec![tachiko_designer_runtime::ImportFieldType::Text]],
                    extra_columns: vec![vec![]],
                },
                "00000000-0000-4000-8000-000000000000",
            )
            .is_err()
        );
    }
}

#[test]
fn compatible_scalar_carriers_and_numeric_blanks_keep_their_values() {
    for (cell, expected) in [
        (
            "<c r=\"A2\" t=\"inlineStr\"><is><t>do-not-drop</t></is></c>",
            SourceValue::Text {
                value: "do-not-drop".into(),
            },
        ),
        (
            "<c r=\"A2\" t=\"n\"><v>123</v></c>",
            SourceValue::Number { value: 123.0 },
        ),
        (
            "<c r=\"A2\"><v>123</v></c>",
            SourceValue::Number { value: 123.0 },
        ),
        ("<c r=\"A2\" t=\"n\"/>", SourceValue::Empty),
        ("<c r=\"A2\"/>", SourceValue::Empty),
        ("<c r=\"A2\" t=\"n\"><v/></c>", SourceValue::Empty),
    ] {
        let inspected = import_xlsx(&scalar_carrier_fixture(cell)).unwrap();
        assert!(
            !inspected.ledger.iter().any(|finding| finding.blocking),
            "{cell}"
        );
        assert_eq!(inspected.sheets[0].rows[0][0].value, expected, "{cell}");
    }
}

#[test]
fn formula_cache_cannot_hide_incompatible_scalar_carriers() {
    for cell in [
        "<c r=\"A2\" t=\"n\"><f>1+2</f><is><t>3</t></is></c>",
        "<c r=\"A2\"><f>1+2</f><is><t>3</t></is></c>",
        "<c r=\"A2\" t=\"inlineStr\"><f>1+2</f><v>3</v></c>",
        "<c r=\"A2\" t=\"e\"><f>1+2</f><is><t>#VALUE!</t></is></c>",
    ] {
        let inspected = import_xlsx(&scalar_carrier_fixture(cell)).unwrap();
        assert!(
            inspected
                .ledger
                .iter()
                .any(|finding| { finding.blocking && finding.code == "scalar_mapping_rejected" }),
            "formula cache weakened structural refusal: {cell}"
        );
    }
}

#[test]
fn duplicate_or_mixed_scalar_carriers_remain_blocking() {
    for cell in [
        "<c r=\"A2\" t=\"n\"><v>1</v><v>2</v></c>",
        "<c r=\"A2\" t=\"inlineStr\"><is><t>a</t></is><is><t>b</t></is></c>",
        "<c r=\"A2\" t=\"inlineStr\"><is><t>a</t></is><v>2</v></c>",
    ] {
        let inspected = import_xlsx(&scalar_carrier_fixture(cell)).unwrap();
        assert!(
            inspected
                .ledger
                .iter()
                .any(|finding| { finding.blocking && finding.code == "scalar_mapping_rejected" }),
            "{cell}"
        );
    }
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
        assert!(export_xlsx(&book).is_err());
        let styled = source_with_format(&book);
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
                "<sheetData>",
                &format!("<cols><col min=\"{min}\" max=\"{max}\" width=\"24\"/></cols><sheetData>"),
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
        s.replace(
            "<sheetData>",
            "<cols><col min=\"1\" max=\"17\" width=\"24\"/></cols><sheetData>",
        )
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
                "<sheetData>",
                &format!(
                    "<cols><col min=\"1\" max=\"1\" width=\"24\"/></cols><cols><col min=\"1\" max=\"2\" {attribute}/></cols><sheetData>"
                ),
            )
        });
        let book = import_xlsx(&changed).unwrap();
        assert!(book.ledger.iter().any(|f| f.code == code && f.blocking));
        assert!(export_xlsx(&book).is_err());
    }
    let width = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace(
            "<sheetData>",
            "<cols><col min=\"1\" max=\"1\" width=\"24\"/></cols><cols><col min=\"2\" max=\"2\" width=\"24\"/></cols><sheetData>",
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
        s.replace(
            "<sheetData>",
            "<cols><col min=\"1\" max=\"1\" width=\"24\"/></cols><cols><unknown/></cols><sheetData>",
        )
    });
    assert!(import_xlsx(&unknown).is_err());
}

#[test]
fn integral_time_only_formats_never_become_dates_or_numbers() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 0.0 };
    for format in ["h:mm", "h:mm:ss", "[m]", "[mm]"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        let bytes = source_with_format(&book);
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
    {
        let (before, after, code) = (
            "<row r=\"2\">",
            "<row r=\"2\" s=\"1\" customFormat=\"1\">",
            "inherited_row_style",
        );
        let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
            s.replace(before, after)
        });
        let imported = import_xlsx(&changed).unwrap();
        assert!(imported.ledger.iter().any(|f| f.code == code && f.blocking));
        assert!(export_xlsx(&imported).is_err());
    }
    let changed = mutate(&bytes, "xl/worksheets/sheet1.xml", |s| {
        s.replace(
            "<sheetData>",
            "<cols><col min=\"1\" max=\"2\" style=\"1\"/></cols><sheetData>",
        )
    });
    let imported = import_xlsx(&changed).unwrap();
    assert!(
        imported
            .ledger
            .iter()
            .any(|f| f.code == "inherited_column_style" && f.blocking)
    );
    assert!(export_xlsx(&imported).is_err());
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
        let imported = import_xlsx(&source_with_format(&book)).unwrap();
        assert!(
            imported
                .ledger
                .iter()
                .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
        );
    }
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 46270.0 };
    let bytes = source_with_format(&book);
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

#[test]
fn csv_admission_checks_all_retained_text_against_the_shared_output_profile() {
    for illegal in ['\0', '\u{b}', '\u{fffe}', '\u{ffff}'] {
        for source in [
            format!("Na{illegal}me\nAda\n"),
            format!("Name\nA{illegal}da\n"),
        ] {
            let book = import_csv(source.as_bytes(), &ImportOptions::default()).unwrap();
            assert!(
                book.ledger
                    .iter()
                    .any(|f| f.blocking && f.code == "output_profile_rejected")
            );
            assert!(export_xlsx(&book).is_err());
        }
    }
    let input = "\"姓名\t😀\n列\r名\"\n\"文字\t😀\n列\r值\"\n";
    let book = import_csv(input.as_bytes(), &ImportOptions::default()).unwrap();
    assert!(!book.ledger.iter().any(|f| f.blocking));
    let reopened = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
    assert_eq!(reopened.sheets[0].columns, book.sheets[0].columns);
    assert_eq!(
        reopened.sheets[0].rows[0][0].value,
        book.sheets[0].rows[0][0].value
    );
}

#[test]
fn checkbox_property_bag_inventory_covers_the_entire_construct() {
    let bytes = include_bytes!("../../tests/fixtures/interop/ordinary-two-sheet.xlsx");
    let path = "xl/featurePropertyBag/featurePropertyBag.xml";
    let original = import_xlsx(bytes).unwrap();
    assert!(!original.ledger.iter().any(|f| f.blocking));
    assert!(
        original
            .ledger
            .iter()
            .any(|f| f.code == "checkbox_presentation")
    );
    let mutations = [
        (
            "</xfpb:FeaturePropertyBags>",
            "<xfpb:unknown/></xfpb:FeaturePropertyBags>",
        ),
        (
            "type=\"Checkbox\" />",
            "type=\"Checkbox\"><xfpb:unknown/></xfpb:bag>",
        ),
        ("type=\"Checkbox\"", "type=\"Checkbox\" unknown=\"1\""),
        (
            "<xfpb:FeaturePropertyBags ",
            "<xfpb:FeaturePropertyBags unknown=\"1\" ",
        ),
        (
            "type=\"Checkbox\" />",
            "type=\"Checkbox\">hidden semantics</xfpb:bag>",
        ),
        (
            "</xfpb:FeaturePropertyBags>",
            "hidden semantics</xfpb:FeaturePropertyBags>",
        ),
        (
            "<xfpb:bag type=\"Checkbox\"",
            "<xfpb:bag xmlns:xfpb=\"urn:unknown\" type=\"Checkbox\"",
        ),
        (
            "<xfpb:FeaturePropertyBags ",
            "<xfpb:FeaturePropertyBags xmlns:unknown=\"urn:unknown\" ",
        ),
        (">0</xfpb:bagId>", ">3</xfpb:bagId>"),
        (">1</xfpb:bagId>", ">0</xfpb:bagId>"),
        (">2</xfpb:bagId>", ">99</xfpb:bagId>"),
        ("k=\"CellControl\"", "k=\"DifferentControl\""),
        ("extRef=\"XFComplementsMapperExtRef\"", "extRef=\"unknown\""),
        (
            "<xfpb:bagId>2</xfpb:bagId>",
            "<xfpb:bagId>2<xfpb:unknown/></xfpb:bagId>",
        ),
        ("<xfpb:bag type=\"Checkbox\" />", ""),
    ];
    for (from, to) in mutations {
        let changed = mutate(bytes, path, |xml| {
            assert!(xml.contains(from), "fixture lacks {from}");
            xml.replace(from, to)
        });
        let book = import_xlsx(&changed).unwrap();
        assert!(
            book.ledger.iter().any(|f| f.blocking && f.location == path),
            "{from}: {:?}",
            book.ledger
        );
        assert!(
            !book
                .ledger
                .iter()
                .any(|f| f.code == "checkbox_presentation"),
            "{from}"
        );
        assert!(export_xlsx(&book).is_err());
    }
}

#[test]
fn numeric_scalar_export_cannot_change_type_through_its_format() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 46000.0 };
    for format in ["yyyy-mm-dd", "h:mm", "[m]"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        assert!(
            export_xlsx(&book)
                .unwrap_err()
                .0
                .contains("Number requires")
        );
    }
    for format in ["0.00", "0%", "[$$-409]#,##0.00"] {
        book.sheets[0].rows[0][1].style.number_format = Some(format.into());
        let reopened = import_xlsx(&export_xlsx(&book).unwrap()).unwrap();
        assert!(!reopened.ledger.iter().any(|f| f.blocking));
        assert_eq!(
            reopened.sheets[0].rows[0][1].value,
            book.sheets[0].rows[0][1].value
        );
    }
}

#[test]
fn standalone_month_tokens_keep_calendar_serials_as_dates() {
    let mut book = simple();
    book.sheets[0].rows[0][1].value = SourceValue::Number { value: 46270.0 };
    for pattern in ["m", "mm", "mmm", "mmmm", "mmmmm", "[$-409]mmmm"] {
        book.sheets[0].rows[0][1].style.number_format = Some(pattern.into());
        let imported = import_xlsx(&source_with_format(&book)).unwrap();
        assert!(
            !imported.ledger.iter().any(|f| f.blocking),
            "{pattern}: {:?}",
            imported.ledger
        );
        assert_eq!(
            imported.sheets[0].rows[0][1].value,
            SourceValue::Date {
                value: "2026-09-05".into()
            }
        );
        let reopened = import_xlsx(&export_xlsx(&imported).unwrap()).unwrap();
        assert_eq!(
            reopened.sheets[0].rows[0][1].value,
            imported.sheets[0].rows[0][1].value
        );
    }
}

#[test]
fn all_language_builtin_formats_match_explicit_patterns_and_scalar_boundaries() {
    // Complete finite "All Languages" table in Microsoft's NumberingFormat
    // reference. Locale-dependent IDs are covered separately and remain blocked
    // unless the source declares an explicit pattern.
    let formats = [
        (0, "General"),
        (1, "0"),
        (2, "0.00"),
        (3, "#,##0"),
        (4, "#,##0.00"),
        (9, "0%"),
        (10, "0.00%"),
        (11, "0.00E+00"),
        (12, "# ?/?"),
        (13, "# ??/??"),
        (14, "mm-dd-yy"),
        (15, "d-mmm-yy"),
        (16, "d-mmm"),
        (17, "mmm-yy"),
        (18, "h:mm AM/PM"),
        (19, "h:mm:ss AM/PM"),
        (20, "h:mm"),
        (21, "h:mm:ss"),
        (22, "m/d/yy h:mm"),
        (37, "#,##0 ;(#,##0)"),
        (38, "#,##0 ;[Red](#,##0)"),
        (39, "#,##0.00;(#,##0.00)"),
        (40, "#,##0.00;[Red](#,##0.00)"),
        (45, "mm:ss"),
        (46, "[h]:mm:ss"),
        (47, "mmss.0"),
        (48, "##0.0E+0"),
        (49, "@"),
    ];
    assert_eq!(formats.len(), 28);
    for (id, pattern) in formats {
        let date = matches!(id, 14..=17 | 22);
        let time = matches!(id, 18..=21 | 45..=47);
        // Integral and fractional values exercise numeric formatting without
        // turning the finite built-in table into a new Time representation.
        for (value, invalid_date) in [(46270.0, false), (46270.5, true), (60.0, true)] {
            let mut source = simple();
            source.sheets[0].rows[0][1].value = SourceValue::Number { value };
            let bytes = export_xlsx(&source).unwrap();
            let builtin = mutate(&bytes, "xl/styles.xml", |xml| {
                xml.replace("<xf numFmtId=\"164\"", &format!("<xf numFmtId=\"{id}\""))
            });
            source.sheets[0].rows[0][1].style.number_format = Some(pattern.into());
            let explicit = source_with_format(&source);
            let builtin = import_xlsx(&builtin).unwrap();
            let explicit = import_xlsx(&explicit).unwrap();
            let blocked = time || (date && invalid_date);
            for book in [&builtin, &explicit] {
                assert_eq!(
                    book.ledger.iter().any(|f| f.blocking),
                    blocked,
                    "ID {id}, {pattern}, {value}: {:?}",
                    book.ledger
                );
                if blocked {
                    assert!(
                        book.ledger
                            .iter()
                            .any(|f| f.blocking && f.code == "scalar_mapping_rejected")
                    );
                    assert!(export_xlsx(book).is_err());
                } else {
                    let expected = if date {
                        SourceValue::Date {
                            value: "2026-09-05".into(),
                        }
                    } else {
                        SourceValue::Number { value }
                    };
                    assert_eq!(book.sheets[0].rows[0][1].value, expected, "ID {id}");
                    let reopened = import_xlsx(&export_xlsx(book).unwrap()).unwrap();
                    assert!(!reopened.ledger.iter().any(|f| f.blocking));
                    assert_eq!(reopened.sheets[0].rows[0][1].value, expected);
                    assert_eq!(
                        reopened.sheets[0].rows[0][1].style.number_format.as_deref(),
                        Some(pattern)
                    );
                }
            }
            assert_eq!(
                builtin.sheets[0].rows[0][1].value,
                explicit.sheets[0].rows[0][1].value
            );
            assert_eq!(
                builtin.sheets[0].rows[0][1].style.number_format.as_deref(),
                if id == 0 { None } else { Some(pattern) }
            );
            assert_eq!(
                explicit.sheets[0].rows[0][1].style.number_format.as_deref(),
                Some(pattern)
            );
        }
    }
}
