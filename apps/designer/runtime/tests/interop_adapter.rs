use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::interop_adapter::*;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
fn simple() -> SourceWorkbook {
    import_csv(b"Name,Amount\nAda,12\n", &ImportOptions::default()).unwrap()
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
