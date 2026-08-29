use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, DesignerWireReply,
    StoredValueProjection, process_wire_request,
};

#[test]
fn bootstrap_exposes_fixture_collections_without_a_document_snapshot() {
    let mut runtime = DesignerRuntime::moonfall().expect("fixture should be valid");

    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap)
        .expect("bootstrap should succeed")
    else {
        panic!("expected bootstrap response");
    };

    assert_eq!(bootstrap.title, "Moonfall Balance");
    assert_eq!(bootstrap.revision, "resident/0");
    assert_eq!(bootstrap.default_collection, "weapons");
    assert_eq!(
        bootstrap
            .collections
            .iter()
            .map(|collection| collection.key.as_str())
            .collect::<Vec<_>>(),
        ["characters", "economy", "items", "weapons"]
    );
    assert_eq!(bootstrap.control_field.entity, "shop");
    assert_eq!(bootstrap.control_field.field, "upgrade_cost");
}

#[test]
fn table_query_keeps_stored_formula_and_calculated_values_distinct() {
    let mut runtime = DesignerRuntime::moonfall().expect("fixture should be valid");

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "weapons".to_owned(),
        })
        .expect("bounded table query should succeed")
    else {
        panic!("expected table response");
    };

    assert_eq!(table.revision, "resident/0");
    assert_eq!(table.collection.key, "weapons");
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].key, "iron_sword");

    let damage = table.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == "damage")
        .expect("damage projection");
    assert_eq!(
        damage
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(36.0)
    );
    assert!(damage.formula.is_none());
    assert!(damage.calculated.is_none());
    assert!(damage.editable_number);

    let dps = table.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == "dps")
        .expect("DPS projection");
    assert!(dps.stored.is_none());
    assert_eq!(
        dps.formula.as_ref().map(|formula| formula.source.as_str()),
        Some("([iron_sword.damage] / [iron_sword.attack_interval])")
    );
    assert_eq!(
        dps.calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(40.0)
    );
    assert!(!dps.editable_number);
}

#[test]
fn number_edit_publishes_once_and_refreshes_only_direct_and_dependent_fields() {
    let mut runtime = DesignerRuntime::moonfall().expect("fixture should be valid");

    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditNumber {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: "45".to_owned(),
        })
        .expect("valid Number edit should execute")
    else {
        panic!("expected publication response");
    };

    assert_eq!(publication.base_revision, "resident/0");
    assert_eq!(publication.resulting_revision, "resident/1");
    assert_eq!(publication.fields, ["iron_sword.damage".into()]);
    assert_eq!(publication.affected_calculations, ["iron_sword.dps".into()]);
    assert!(!publication.fields.contains(&"shop.upgrade_cost".into()));
    assert!(
        !publication
            .affected_calculations
            .contains(&"shop.upgrade_cost".into())
    );

    let DesignerResponse::Fields(refreshed) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: publication.resulting_revision.clone(),
            fields: vec!["iron_sword.damage".into(), "iron_sword.dps".into()],
        })
        .expect("selective refresh should succeed")
    else {
        panic!("expected fields response");
    };
    assert_eq!(refreshed.revision, "resident/1");
    assert_eq!(refreshed.fields.len(), 2);
    assert_eq!(
        refreshed.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(45.0)
    );
    assert_eq!(
        refreshed.fields[1]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(50.0)
    );
}

#[test]
fn stale_and_calculation_failing_edits_leave_the_published_projection_unchanged() {
    let mut runtime = DesignerRuntime::moonfall().expect("fixture should be valid");
    runtime
        .handle(DesignerRequest::EditNumber {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: "45".to_owned(),
        })
        .expect("first edit should publish");

    let stale = runtime
        .handle(DesignerRequest::EditNumber {
            expected_revision: "resident/0".to_owned(),
            target: "iron_sword.damage".into(),
            input: "50".to_owned(),
        })
        .expect_err("stale expected revision must fail");
    let stale = stale.failure_projection("resident/1");
    assert_eq!(stale.code, "stale_revision");
    assert_eq!(stale.current_revision, "resident/1");

    let invalid = runtime
        .handle(DesignerRequest::EditNumber {
            expected_revision: "resident/1".to_owned(),
            target: "iron_sword.attack_interval".into(),
            input: "0".to_owned(),
        })
        .expect_err("division-by-zero candidate must fail validation");
    let invalid = invalid.failure_projection("resident/1");
    assert_eq!(invalid.code, "validation_failed");
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "formula.division_by_zero")
    );

    let DesignerResponse::Fields(current) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: "resident/1".to_owned(),
            fields: vec![
                "iron_sword.attack_interval".into(),
                "iron_sword.damage".into(),
                "iron_sword.dps".into(),
            ],
        })
        .expect("current fields should remain queryable")
    else {
        panic!("expected fields response");
    };
    assert_eq!(current.revision, "resident/1");
    assert_eq!(
        current.fields[0]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(0.9)
    );
    assert_eq!(
        current.fields[1]
            .stored
            .as_ref()
            .and_then(StoredValueProjection::number),
        Some(45.0)
    );
    assert_eq!(
        current.fields[2]
            .calculated
            .as_ref()
            .and_then(CalculationProjection::number),
        Some(50.0)
    );
}

#[test]
fn wire_bridge_returns_bounded_results_and_structured_failures() {
    let mut runtime = None;

    let bootstrap = process_wire_request(&mut runtime, br#"{"type":"bootstrap"}"#);
    let bootstrap_text = String::from_utf8(bootstrap.clone()).expect("JSON should be UTF-8");
    assert!(!bootstrap_text.contains("\"schemas\""));
    assert!(!bootstrap_text.contains("\"entities\""));
    let DesignerWireReply::Ok { response } =
        serde_json::from_slice(&bootstrap).expect("reply should decode")
    else {
        panic!("expected successful bootstrap reply");
    };
    assert!(matches!(response, DesignerResponse::Bootstrap(_)));

    let invalid = process_wire_request(
        &mut runtime,
        br#"{"type":"edit_number","expected_revision":"resident/0","target":{"entity":"iron_sword","field":"damage"},"input":"not-a-number"}"#,
    );
    let DesignerWireReply::Error { error } =
        serde_json::from_slice(&invalid).expect("failure should decode")
    else {
        panic!("expected structured failure reply");
    };
    assert_eq!(error.code, "invalid_number");
    assert_eq!(error.current_revision, "resident/0");
}
