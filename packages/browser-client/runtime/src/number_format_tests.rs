use super::*;
use tachiko_workspace_engine::{NumberFormat, NumberFormatError, NumberFormatKind};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000583";

fn format() -> NumberFormat {
    NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Percent,
        thousands_separator: true,
    }
}

fn fixture() -> (DesignerRuntime, String, String) {
    let columns = [
        NewTableColumnInput {
            name: "Label".into(),
            field_type: "text".into(),
            required: false,
        },
        NewTableColumnInput {
            name: "Amount".into(),
            field_type: "number".into(),
            required: false,
        },
    ];
    let runtime = DesignerRuntime::new_table(OCCURRENCE, "Formats", &columns).unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let field = runtime
        .query_table(&collection)
        .unwrap()
        .columns
        .iter()
        .find(|column| column.field_type == "number")
        .unwrap()
        .id
        .clone();
    (runtime, collection, field)
}

fn set(
    runtime: &mut DesignerRuntime,
    collection: &str,
    field: &str,
    number_format: Option<NumberFormat>,
) -> Result<DesignerResponse, DesignerError> {
    runtime.handle(DesignerRequest::SetColumnNumberFormat {
        expected_revision: runtime.current_revision().into(),
        collection: collection.into(),
        field: field.into(),
        number_format,
    })
}

fn projected(runtime: &DesignerRuntime, collection: &str, field: &str) -> Option<NumberFormat> {
    runtime
        .query_table(collection)
        .unwrap()
        .columns
        .into_iter()
        .find(|column| column.id == field)
        .unwrap()
        .number_format
}

#[test]
fn number_format_set_clear_undo_redo_uses_normal_lifecycle() {
    let (mut runtime, collection, field) = fixture();
    assert_eq!(projected(&runtime, &collection, &field), None);
    let DesignerResponse::Published(publication) =
        set(&mut runtime, &collection, &field, Some(format())).unwrap()
    else {
        panic!("expected publication")
    };
    assert_eq!(publication.schema_fields.len(), 1);
    assert_eq!(projected(&runtime, &collection, &field), Some(format()));
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
    assert_eq!(projected(&runtime, &collection, &field), None);
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::Redo { expected_revision })
        .unwrap();
    assert_eq!(projected(&runtime, &collection, &field), Some(format()));
    set(&mut runtime, &collection, &field, None).unwrap();
    assert_eq!(projected(&runtime, &collection, &field), None);
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
    assert_eq!(projected(&runtime, &collection, &field), Some(format()));
}

#[test]
fn number_format_errors_and_stale_revisions_publish_nothing() {
    let (mut runtime, collection, field) = fixture();
    let original = runtime.session.export_snapshot().into_document();
    let revision = runtime.current_revision().to_owned();
    let mut invalid = format();
    invalid.decimals = 11;
    let error = set(&mut runtime, &collection, &field, Some(invalid)).unwrap_err();
    assert!(matches!(
        error,
        DesignerError::InvalidNumberFormat {
            source: NumberFormatError::DecimalsOutOfRange
        }
    ));
    assert_eq!(
        error.failure_projection(&revision).code,
        "invalid_number_format"
    );
    let mut invalid = format();
    invalid.kind = NumberFormatKind::Currency { code: "usd".into() };
    assert!(matches!(
        set(&mut runtime, &collection, &field, Some(invalid)),
        Err(DesignerError::InvalidNumberFormat {
            source: NumberFormatError::InvalidCurrencyCode
        })
    ));
    let text = runtime
        .query_table(&collection)
        .unwrap()
        .columns
        .into_iter()
        .find(|column| column.field_type == "text")
        .unwrap()
        .id;
    assert!(matches!(
        set(&mut runtime, &collection, &text, Some(format())),
        Err(DesignerError::InvalidNumberFormat {
            source: NumberFormatError::NonNumberField
        })
    ));
    assert!(matches!(
        runtime.handle(DesignerRequest::SetColumnNumberFormat {
            expected_revision: "resident/999".into(),
            collection: collection.clone(),
            field: field.clone(),
            number_format: Some(format()),
        }),
        Err(DesignerError::StaleQuery { .. })
    ));
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.session.export_snapshot().document(), &original);
}

#[test]
fn number_format_save_reopen_duplicate_and_remove() {
    let (mut runtime, collection, field) = fixture();
    set(&mut runtime, &collection, &field, Some(format())).unwrap();
    let expected = runtime.session.export_snapshot().into_document();
    let revision = runtime.current_revision().to_owned();
    assert!(matches!(
        runtime.export_project(&revision),
        Err(DesignerError::LegacyExportUnsupported { .. })
    ));
    let export = runtime.export_project_v3(&revision).unwrap();
    let mut reopened = None;
    open_project(&mut reopened, &export.bytes, OCCURRENCE).unwrap();
    let reopened = reopened.unwrap();
    assert_eq!(reopened.session.export_snapshot().document(), &expected);
    let copy = runtime
        .duplicate_collection(&revision, &collection, "Copy")
        .unwrap();
    let copied = runtime
        .query_table(&copy.collection.key)
        .unwrap()
        .columns
        .into_iter()
        .find(|column| column.field_type == "number")
        .unwrap();
    assert_ne!(copied.id, field);
    assert_eq!(copied.number_format, Some(format()));
    let revision = runtime.current_revision().to_owned();
    runtime
        .remove_column(&revision, &collection, &field)
        .unwrap();
    assert_eq!(
        runtime
            .query_table(&collection)
            .unwrap()
            .columns
            .into_iter()
            .filter_map(|column| column.number_format)
            .collect::<Vec<_>>(),
        [] as [NumberFormat; 0]
    );
    let revision = runtime.current_revision().to_owned();
    runtime
        .add_column_with_requiredness(&revision, &collection, "Amount", "text", false, &[])
        .unwrap();
    assert_eq!(
        runtime
            .query_table(&collection)
            .unwrap()
            .columns
            .into_iter()
            .filter_map(|column| column.number_format)
            .collect::<Vec<_>>(),
        [] as [NumberFormat; 0]
    );
}

#[test]
fn number_format_does_not_scale_stored_or_calculated_values() {
    let (mut runtime, collection, field) = fixture();
    let revision = runtime.current_revision().to_owned();
    runtime
        .insert_row(
            &revision,
            &collection,
            &[RowInitializer {
                field: field.clone(),
                input: ScalarEditInput::Number {
                    input: "0.125".into(),
                },
            }],
        )
        .unwrap();
    let before = runtime.query_table(&collection).unwrap().rows;
    set(&mut runtime, &collection, &field, Some(format())).unwrap();
    assert_eq!(runtime.query_table(&collection).unwrap().rows, before);
    let revision = runtime.current_revision().to_owned();
    runtime
        .update_formula(
            &revision,
            &FieldTarget {
                entity: before[0].id.clone(),
                field: field.clone(),
            },
            "0.125*2",
        )
        .unwrap();
    let calculated = runtime.query_table(&collection).unwrap().rows;
    set(&mut runtime, &collection, &field, None).unwrap();
    assert_eq!(runtime.query_table(&collection).unwrap().rows, calculated);
}
