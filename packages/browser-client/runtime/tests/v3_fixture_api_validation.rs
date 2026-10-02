//! Bounded existing-interface/fixture validation; no selected-v3 qualification.
#[path = "v3_acceptance_fixtures/mod.rs"]
mod fixtures;

use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerRuntime, ImportFieldType, ImportSelection,
    NewTableColumnInput, OpenedProjection, StoredValueProjection, import_workbook,
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
        let _typed_open_api: fn(&[u8]) -> Result<OpenedProjection, DesignerError> =
            fixtures::inspect_existing_api;
        let _typed_replace_api: fn(
            &mut Option<DesignerRuntime>,
            &[u8],
            &str,
        ) -> Result<OpenedProjection, DesignerError> = fixtures::open_existing_api;
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
