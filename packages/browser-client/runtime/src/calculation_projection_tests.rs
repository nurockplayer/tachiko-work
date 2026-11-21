use serde_json::json;

use super::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime,
    FormulaCalculationOutcome, Number, calculation_projection,
};

#[test]
fn calculated_date_projects_tag_and_day_number() {
    let date = tachiko_semantic_core::Date::parse("2026-01-02").unwrap();
    let projection = calculation_projection(&FormulaCalculationOutcome::Date(date));
    assert_eq!(
        serde_json::to_value(projection).unwrap(),
        json!({"status": "value", "value": 20455.0, "result": {"kind": "date", "value": "2026-01-02"}})
    );
}

#[test]
fn calculated_number_preserves_main_wire_bytes_and_failure_keeps_its_code() {
    let number =
        calculation_projection(&FormulaCalculationOutcome::Value(Number::new(2.5).unwrap()));
    assert_eq!(
        serde_json::to_vec(&number).unwrap().as_slice(),
        br#"{"status":"value","value":2.5}"#
    );
    let failure = calculation_projection(&FormulaCalculationOutcome::Failure(
        tachiko_formula_engine::CalculationFailure::DivisionByZero,
    ));
    let failure = serde_json::to_value(failure).unwrap();
    assert_eq!(failure["status"], "failure");
    assert_eq!(failure["code"], "division_by_zero");
    assert!(failure.get("result").is_none());
    assert_eq!(
        serde_json::to_value(calculation_projection(
            &FormulaCalculationOutcome::Unavailable
        ))
        .unwrap(),
        json!({"status": "unavailable"})
    );
}

#[test]
fn tagged_results_roundtrip_and_legacy_value_still_decodes() {
    for result in [
        json!({"kind": "number", "value": 2.5}),
        json!({"kind": "text", "value": "hello"}),
        json!({"kind": "boolean", "value": false}),
        json!({"kind": "date", "value": "2026-01-02"}),
    ] {
        // The legacy numeric field is independent of the result's tag.
        let wire = json!({"status": "value", "value": 2.5, "result": result});
        let decoded: CalculationProjection = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded.number(), Some(2.5));
        assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
    }
    let legacy = json!({"status": "value", "value": 2.5});
    let decoded: CalculationProjection = serde_json::from_value(legacy.clone()).unwrap();
    assert!(matches!(
        decoded,
        CalculationProjection::Value { result: None, .. }
    ));
    assert_eq!(serde_json::to_value(decoded).unwrap(), legacy);
}

#[test]
fn malformed_calculated_results_refuse_instead_of_losing_the_tag() {
    for result in [
        json!({"kind": "date", "value": "2026-02-30"}),
        json!({"kind": "number", "value": "2.5"}),
        json!({"kind": "text", "value": false}),
        json!({"kind": "boolean", "value": 1}),
        json!({"kind": "unknown", "value": 0}),
    ] {
        let wire = json!({"status": "value", "value": 2.5, "result": result});
        assert!(serde_json::from_value::<CalculationProjection>(wire).is_err());
    }
}

#[test]
fn budget_table_wire_omits_number_tags_beside_stored_dates() {
    let mut runtime = DesignerRuntime::budget("00000000-0000-4000-8000-000000000842").unwrap();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "budget_items".into(),
        })
        .unwrap()
    else {
        panic!("expected budget table");
    };
    let wire = serde_json::to_value(table).unwrap();
    let mut formulas = 0;
    let mut stored_dates = 0;
    for row in wire["rows"].as_array().unwrap() {
        for field in row["fields"].as_array().unwrap() {
            if field["stored"]["kind"] == "date" {
                stored_dates += 1;
            }
            let calculated = &field["calculated"];
            if calculated["status"] == "value" {
                assert!(calculated.get("result").is_none());
                assert_eq!(calculated.as_object().unwrap().len(), 2);
                assert!(calculated["value"].is_number());
                formulas += 1;
            }
        }
    }
    assert!(formulas > 0);
    assert!(stored_dates > 0);
}
