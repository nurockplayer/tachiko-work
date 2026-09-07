use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget,
    ScalarEditInput, open_project,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000341";

#[test]
fn quarterly_plan_supports_authoritative_human_edit_and_impact() {
    let mut runtime = fixture_runtime();
    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: OCCURRENCE.to_owned(),
        })
        .unwrap()
    else {
        panic!("expected bootstrap");
    };
    assert_eq!(bootstrap.collections.len(), 2);
    let old_revision = bootstrap.revision;
    let edit = |revision: &str, input: &str| DesignerRequest::EditScalar {
        expected_revision: revision.to_owned(),
        target: FieldTarget::from("e-tax.f-rate"),
        input: ScalarEditInput::Number {
            input: input.to_owned(),
        },
    };
    let DesignerResponse::Published(publication) =
        runtime.handle(edit(&old_revision, "0.5")).unwrap()
    else {
        panic!("expected publication");
    };
    assert_eq!(publication.base_revision, old_revision);
    assert_ne!(publication.resulting_revision, old_revision);
    assert_eq!(publication.fields, [FieldTarget::from("e-tax.f-rate")]);
    let mut affected = publication.affected_calculations.clone();
    affected.sort();
    assert_eq!(
        affected,
        ["e-feb.f-net", "e-jan.f-net", "e-mar.f-net"].map(FieldTarget::from)
    );
    let current = publication.resulting_revision;
    let before_rejection = query_plan(&mut runtime);
    let saved = runtime.export_project(&current).unwrap();
    for (revision, input, code) in [
        (old_revision.as_str(), "0.75", "stale_revision"),
        (current.as_str(), "not-a-number", "invalid_number"),
    ] {
        let failure = runtime
            .handle(edit(revision, input))
            .unwrap_err()
            .failure_projection(&current);
        assert_eq!(failure.code, code);
        assert_eq!(failure.current_revision, current);
        assert_eq!(query_plan(&mut runtime), before_rejection);
        assert_eq!(runtime.export_project(&current).unwrap().bytes, saved.bytes);
    }
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "plan".to_owned(),
        })
        .unwrap()
    else {
        panic!("expected plan table");
    };
    assert_eq!(table.revision, current);
    let mut reopened = None;
    let opened = open_project(
        &mut reopened,
        &saved.bytes,
        "00000000-0000-4000-8000-000000000342",
    )
    .unwrap();
    assert_eq!(
        reopened
            .as_ref()
            .unwrap()
            .export_project(&opened.bootstrap.revision)
            .unwrap()
            .bytes,
        saved.bytes
    );
    for row in table.rows {
        let expected = match row.id.as_str() {
            "e-jan" | "e-feb" => (800.0, 400.0),
            "e-mar" => (1000.0, 500.0),
            other => panic!("unexpected row {other}"),
        };
        for (field, value) in [("f-gross", expected.0), ("f-net", expected.1)] {
            let observed = row
                .fields
                .iter()
                .find(|item| item.target.field == field)
                .unwrap();
            let calculated = observed.calculated.as_ref();
            assert_eq!(
                calculated.and_then(CalculationProjection::number),
                Some(value)
            );
            assert!(observed.diagnostics.is_empty());
            assert!(observed.formula.is_some());
            assert!(observed.editable_scalar.is_none());
        }
    }
}

fn fixture_runtime() -> DesignerRuntime {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../experiments/open-sheet-export/canary.json"
    ))
    .unwrap();
    let bytes = serde_json::to_vec(&fixture["document"]).unwrap();
    let document = tachiko_storage::from_bytes(&bytes).unwrap();
    DesignerRuntime::from_document(document, OCCURRENCE).unwrap()
}

fn query_plan(runtime: &mut DesignerRuntime) -> serde_json::Value {
    serde_json::to_value(
        runtime
            .handle(DesignerRequest::QueryTable {
                collection: "plan".to_owned(),
            })
            .unwrap(),
    )
    .unwrap()
}
