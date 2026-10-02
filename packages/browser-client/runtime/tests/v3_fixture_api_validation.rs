//! Bounded existing-interface/fixture validation; no selected-v3 qualification.
#[path = "v3_acceptance_fixtures/mod.rs"]
mod fixtures;

use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
use tachiko_designer_runtime::{
    DesignerRequest, ImportFieldType, ImportSelection, NewTableColumnInput, StoredValueProjection,
    import_workbook,
};
use tachiko_storage::{CanonicalRoProjectV3, FormatError, decode_roproj_v3, encode_roproj_v3};
use tachiko_workspace_engine::{Date, FieldConstraint, SchemaId};

#[test]
fn valid_constraint_fixtures_roundtrip_in_the_existing_v3_storage_codec() {
    for (index, nondefault) in [(1, false), (2, false), (1, true), (2, true)] {
        let expected = if nondefault {
            fixtures::nondefault_constrained_v3_document(index)
        } else {
            fixtures::constrained_v3_document(index)
        };
        let tree = encode_roproj_v3(&expected).expect("v3 codec accepts valid field constraints");
        assert_eq!(decode_roproj_v3(&tree).unwrap(), expected);
        assert!(fixtures::frame_v3(&tree).starts_with(b"TWDPROJ1"));
        let transfer = if nondefault {
            fixtures::nondefault_constrained_v3_transfer(index)
        } else {
            fixtures::constrained_v3_transfer(index)
        };
        assert!(transfer.starts_with(b"TWDPROJ1"));
        if nondefault {
            let default = &expected.schemas[&SchemaId::from(fixtures::SCHEMA_ID)];
            assert!(
                default
                    .fields
                    .values()
                    .all(|field| field.constraint == FieldConstraint::None)
            );
            let extra = &expected.schemas[&SchemaId::from("zz_acceptance_unbound_schema")];
            assert_eq!(extra.fields.len(), 1);
            assert_ne!(
                extra.fields.values().next().unwrap().constraint,
                FieldConstraint::None
            );
            assert!(
                expected
                    .entities
                    .values()
                    .all(|entity| entity.schema != extra.id)
            );
            assert!(
                expected
                    .keyed_grouped_sum_definitions
                    .values()
                    .all(|definition| definition.orders.schema != extra.id
                        && definition.products.schema != extra.id)
            );
        }
        let error = fixtures::inspect_existing_api(&transfer).unwrap_err();
        assert_eq!(
            error.failure_projection("resident/0").code,
            "unsupported_project"
        );
        let mut slot = None;
        let error = fixtures::open_existing_api(
            &mut slot,
            &transfer,
            "00000000-0000-4000-8000-000000000001",
        )
        .unwrap_err();
        assert_eq!(
            error.failure_projection("resident/0").code,
            "unsupported_project"
        );
        assert!(slot.is_none());
    }
}

#[test]
fn all_definition_mutations_reach_their_storage_representation_or_semantic_gate() {
    let mut document = fixtures::constrained_v3_document(1);
    for schema in document.schemas.values_mut() {
        for field in schema.fields.values_mut() {
            field.constraint = FieldConstraint::None;
        }
    }
    let tree = encode_roproj_v3(&document).unwrap();
    let definition = tree
        .files()
        .iter()
        .find(|file| file.path() == "definitions.json")
        .unwrap();
    let vectors =
        fixtures::malformed_definition_payloads(std::str::from_utf8(definition.bytes()).unwrap());
    assert_eq!(vectors.len(), 8);
    for (label, body) in vectors {
        // Every vector remains syntactically JSON; refusal must concern a
        // representation member or semantic binding, not a broken JSON edit.
        let _: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let files = tree
            .files()
            .iter()
            .map(|file| {
                (
                    file.path().to_owned(),
                    if file.path() == "definitions.json" {
                        body.clone()
                    } else {
                        file.bytes().to_vec()
                    },
                )
            })
            .collect();
        let error = CanonicalRoProjectV3::try_from_files(files)
            .and_then(|tree| decode_roproj_v3(&tree))
            .expect_err("invalid definition vector must reach its intended storage gate");
        match error {
            FormatError::InvalidRoProjectRepresentation { ref message } => {
                let expected = match label {
                    "empty definition id" => "must not be empty",
                    "omitted definition id" => "missing field `id`",
                    "missing schema reference"
                    | "missing field reference"
                    | "wrong bound field type" => "invalid keyed grouped-sum definition",
                    "missing binding member" => "missing field `quantity_field`",
                    "duplicate definition record" => "duplicate keyed grouped-sum definition",
                    "unknown catalogue item shape" => {
                        "'definitions.json' does not match .roproj/v3"
                    }
                    other => panic!("unexpected fixture label {other}"),
                };
                assert!(
                    message.contains(expected),
                    "{label} hit an unintended representation gate: {message}"
                );
            }
            other => panic!("{label} reached an unrelated gate: {other}"),
        }
        println!("FIXTURE_MUTATION_GATE_PASS label={label} error={error}");
    }
}

#[test]
fn real_import_keeps_the_unedited_26_date_and_new_occurrence_identities() {
    let csv = include_bytes!("../../../../acceptance/date-definition-save-v3/fixtures/mixed.csv");
    let source = import_csv(csv, &ImportOptions::default()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Text,
            ImportFieldType::Number,
            ImportFieldType::Number,
            ImportFieldType::Date,
        ]],
        extra_columns: vec![vec![]],
    };
    for occurrence in [
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000005",
    ] {
        let (mut runtime, imported) = import_workbook(&source, &selection, occurrence).unwrap();
        let prefix = format!("import_{occurrence}_");
        assert_eq!(
            imported.metadata.sheets[0].rows[0].entity_id,
            format!("{prefix}0008")
        );
        assert_eq!(
            imported.metadata.sheets[0].columns[4].field_id,
            format!("{prefix}0007")
        );
        assert_eq!(
            imported.opened.table.rows[0].fields[4].stored,
            Some(StoredValueProjection::Date {
                value: Date::parse("2026-09-26").unwrap()
            })
        );
        let before = runtime.observe_occurrence();
        let error = runtime
            .handle(DesignerRequest::NewTable {
                occurrence_id: occurrence.into(),
                name: "rejected".into(),
                columns: vec![NewTableColumnInput {
                    name: "value".into(),
                    field_type: "not-a-type".into(),
                }],
            })
            .unwrap_err();
        assert_eq!(
            error.failure_projection(&before.revision).code,
            "invalid_table_operation"
        );
        assert_eq!(runtime.observe_occurrence(), before);
    }
    let invalid = ImportSelection {
        column_types: vec![],
        extra_columns: vec![vec![]],
    };
    assert_eq!(
        import_workbook(&source, &invalid, "00000000-0000-4000-8000-000000000005")
            .err()
            .unwrap()
            .failure_projection("resident/3")
            .code,
        "invalid_tracker_operation"
    );
}

const CAPTURE_HEAD: &str = "6ed764735ad2bbd7be6a04a67eaaaf4d8e107817";
const CAPTURE_RUN: &str = "00000000-0000-4000-8000-000000000001";

fn complete_capture_context() -> fixtures::CaptureInputs<'static> {
    fixtures::CaptureInputs {
        required: Some("1"),
        directory: Some("/tmp/capture"),
        head: Some(CAPTURE_HEAD),
        run_id: Some(CAPTURE_RUN),
    }
}

#[test]
fn capture_context_distinguishes_regression_from_qualified_capture() {
    use fixtures::{CaptureInputs, resolve_capture_context};
    assert_eq!(
        resolve_capture_context(CaptureInputs::default(), None, None),
        Ok(None)
    );
    let complete = resolve_capture_context(
        complete_capture_context(),
        Some(CAPTURE_HEAD),
        Some(CAPTURE_RUN),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        (complete.directory, complete.head, complete.run_id),
        ("/tmp/capture", CAPTURE_HEAD, CAPTURE_RUN)
    );
}

#[test]
fn capture_context_rejects_partial_runtime_inputs() {
    use fixtures::{CaptureInputs, resolve_capture_context};
    let full = complete_capture_context();
    for partial in [
        CaptureInputs {
            required: Some("1"),
            ..CaptureInputs::default()
        },
        CaptureInputs {
            directory: Some("/tmp/capture"),
            ..CaptureInputs::default()
        },
        CaptureInputs {
            head: Some(CAPTURE_HEAD),
            ..CaptureInputs::default()
        },
        CaptureInputs {
            run_id: Some(CAPTURE_RUN),
            ..CaptureInputs::default()
        },
        CaptureInputs {
            directory: None,
            ..full
        },
        CaptureInputs { head: None, ..full },
        CaptureInputs {
            run_id: None,
            ..full
        },
    ] {
        assert!(resolve_capture_context(partial, Some(CAPTURE_HEAD), Some(CAPTURE_RUN)).is_err());
    }
}

#[test]
fn capture_context_rejects_malformed_runtime_inputs() {
    use fixtures::{CaptureInputs, resolve_capture_context};
    let full = complete_capture_context();
    for malformed in [
        CaptureInputs {
            required: Some("0"),
            ..full
        },
        CaptureInputs {
            required: Some(""),
            ..full
        },
        CaptureInputs {
            directory: Some(""),
            ..full
        },
        CaptureInputs {
            directory: Some("relative"),
            ..full
        },
        CaptureInputs {
            directory: Some("/tmp/\0capture"),
            ..full
        },
        CaptureInputs {
            head: Some(""),
            ..full
        },
        CaptureInputs {
            head: Some("bad"),
            ..full
        },
        CaptureInputs {
            head: Some("6ED764735AD2BBD7BE6A04A67EAAAF4D8E107817"),
            ..full
        },
        CaptureInputs {
            run_id: Some(""),
            ..full
        },
        CaptureInputs {
            run_id: Some("not-a-uuid"),
            ..full
        },
        CaptureInputs {
            run_id: Some("00000000_0000-4000-8000-000000000001"),
            ..full
        },
        CaptureInputs {
            run_id: Some("abcdefAB-0000-4000-8000-000000000001"),
            ..full
        },
    ] {
        assert!(resolve_capture_context(malformed, Some(CAPTURE_HEAD), Some(CAPTURE_RUN)).is_err());
    }
}

#[test]
fn capture_context_requires_both_matching_compiled_identities() {
    use fixtures::{CaptureInputs, resolve_capture_context};
    for compiled in [
        (None, None),
        (Some(CAPTURE_HEAD), None),
        (None, Some(CAPTURE_RUN)),
        (
            Some("0000000000000000000000000000000000000000"),
            Some(CAPTURE_RUN),
        ),
        (
            Some(CAPTURE_HEAD),
            Some("00000000-0000-4000-8000-000000000002"),
        ),
    ] {
        assert!(
            resolve_capture_context(complete_capture_context(), compiled.0, compiled.1).is_err()
        );
        if compiled != (None, None) {
            assert!(
                resolve_capture_context(CaptureInputs::default(), compiled.0, compiled.1).is_err()
            );
        }
    }
    assert!(
        resolve_capture_context(
            CaptureInputs::default(),
            Some(CAPTURE_HEAD),
            Some(CAPTURE_RUN)
        )
        .is_err()
    );
}
