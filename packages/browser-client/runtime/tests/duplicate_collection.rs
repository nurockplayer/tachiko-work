use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, FieldProjection,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";

fn table(
    runtime: &mut DesignerRuntime,
    collection: &str,
) -> tachiko_designer_runtime::TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .unwrap()
    else {
        panic!("expected table projection");
    };
    table
}

fn field<'a>(
    table: &'a tachiko_designer_runtime::TableProjection,
    key: &str,
) -> &'a FieldProjection {
    let field_id = table
        .columns
        .iter()
        .find(|column| column.key == key)
        .unwrap()
        .id
        .as_str();
    table
        .rows
        .first()
        .unwrap()
        .fields
        .iter()
        .find(|field| field.target.field == field_id)
        .unwrap()
}

#[test]
fn duplicate_collection_remaps_internal_formula_refs_and_keeps_external_refs() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/0".into(),
            target: "monthly_summary.remaining".into(),
            source: "[monthly_summary.planned_total] + [utilities.actual]".into(),
        })
        .unwrap();
    let source = table(&mut runtime, "budget_summary");
    let source_value = field(&source, "remaining")
        .calculated
        .as_ref()
        .and_then(CalculationProjection::number);

    runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/1".into(),
            collection: "budget_summary".into(),
            name: "October Summary".into(),
        })
        .unwrap();
    let copy = table(&mut runtime, "october-summary");
    assert_ne!(source.collection.id, copy.collection.id);
    assert_eq!(source.rows.len(), copy.rows.len());
    assert!(
        source
            .columns
            .iter()
            .all(|column| copy.columns.iter().all(|other| other.id != column.id))
    );
    assert!(
        source
            .rows
            .iter()
            .all(|row| copy.rows.iter().all(|other| other.id != row.id))
    );
    let copied_remaining = field(&copy, "remaining");
    let copied_source = copied_remaining.formula.as_ref().unwrap().source.clone();
    assert!(copied_source.contains("october-summary"));
    assert!(copied_source.contains("utilities"));
    assert_eq!(
        copied_remaining
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        source_value
    );

    let copy_planned = field(&copy, "planned_total").target.clone();
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: "resident/2".into(),
            target: copy_planned,
            source: "1000".into(),
        })
        .unwrap();
    let source_after = table(&mut runtime, "budget_summary");
    assert_eq!(
        field(&source_after, "planned_total").stored,
        field(&source, "planned_total").stored
    );
    assert_eq!(
        field(&source_after, "remaining").formula,
        field(&source, "remaining").formula
    );

    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/3".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/4".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/5".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/6".into(),
        })
        .unwrap();
    assert!(
        runtime
            .handle(DesignerRequest::QueryTable {
                collection: "october-summary".into(),
            })
            .is_err()
    );
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/7".into(),
        })
        .unwrap();
    assert_eq!(
        table(&mut runtime, "october-summary").rows.len(),
        copy.rows.len()
    );
}

#[test]
fn duplicate_collection_rejects_stale_revision_without_mutating_bytes() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let before = runtime.export_project("resident/0").unwrap();
    assert!(
        runtime
            .handle(DesignerRequest::DuplicateCollection {
                expected_revision: "resident/1".into(),
                collection: "budget_summary".into(),
                name: "October Summary".into(),
            })
            .is_err()
    );
    assert_eq!(runtime.export_project("resident/0").unwrap(), before);
}

#[test]
fn duplicate_collection_does_not_reuse_identities_after_undo() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/0".into(),
            collection: "budget_summary".into(),
            name: "January Summary".into(),
        })
        .unwrap();
    let first = table(&mut runtime, "january-summary");
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/2".into(),
            collection: "budget_summary".into(),
            name: "February Summary".into(),
        })
        .unwrap();
    let second = table(&mut runtime, "february-summary");
    assert_ne!(first.collection.id, second.collection.id);
    assert!(
        first
            .columns
            .iter()
            .all(|column| second.columns.iter().all(|other| other.id != column.id))
    );
    assert!(
        first
            .rows
            .iter()
            .all(|row| second.rows.iter().all(|other| other.id != row.id))
    );
}
