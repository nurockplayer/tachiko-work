use serde_json::{Value as Json, json};
use tachiko_designer_runtime::{DesignerRuntime, process_wire_request};
use tachiko_workspace_engine::{
    Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000667";

fn grid(row_count: usize, column_count: usize) -> Document {
    let mut document = Document::empty("a1-capacity", "A1 Capacity");
    let fields = (0..column_count)
        .map(|index| {
            let key = format!("c{index:02}");
            (
                key.clone().into(),
                FieldDefinition {
                    id: key.clone().into(),
                    key: key.into(),
                    display_name: None,
                    number_format: None,
                    date_format: None,
                    field_type: FieldType::Number,
                    required: false,
                    constraint: FieldConstraint::None,
                },
            )
        })
        .collect();
    document.schemas.insert(
        "grid".into(),
        Schema {
            headerless: false,
            id: "grid".into(),
            key: "grid".into(),
            display_name: None,
            field_order: None,
            fields,
        },
    );
    for index in 0..row_count {
        // Each 48-byte ID contributes 51 bytes to the serialized row list.
        let id = format!("row-{index:044}");
        document.entities.insert(
            id.clone().into(),
            Entity {
                rank: None,
                id: id.into(),
                key: format!("r{index:05}").into(),
                schema: "grid".into(),
                fields: [("c01".into(), Value::Number(Number::new(2.0).unwrap()))].into(),
            },
        );
    }
    document
}

fn geometry(document: &Document) -> Json {
    let rows: Vec<_> = document
        .entities
        .keys()
        .map(tachiko_workspace_engine::EntityId::as_str)
        .collect();
    let columns: Vec<_> = document.schemas["grid"]
        .fields
        .keys()
        .map(tachiko_workspace_engine::FieldId::as_str)
        .collect();
    json!({"header_row": 1, "rows": rows, "columns": columns})
}

fn call(resident: &mut Option<DesignerRuntime>, request: &Json) -> Json {
    let input = serde_json::to_vec(request).unwrap();
    serde_json::from_slice(&process_wire_request(resident, &input)).unwrap()
}

fn translate(resident: &mut Option<DesignerRuntime>, text: &str, geometry: &Json) -> Json {
    let revision = resident.as_ref().unwrap().observe_occurrence().revision;
    let request = json!({
        "type": "translate_a1_formula",
        "expected_revision": revision,
        "collection_id": "grid",
        "row_entity_id": geometry["rows"][0],
        "target_field_id": "c00",
        "text": text,
        "geometry": geometry
    });
    call(resident, &request)
}

fn publish(resident: &mut Option<DesignerRuntime>, source: &Json, target: &Json) -> Json {
    let request = json!({
        "type": "formula_update",
        "expected_revision": resident.as_ref().unwrap().observe_occurrence().revision,
        "target": target,
        "source": source
    });
    call(resident, &request)
}

#[test]
fn text_capacity_refuses_large_geometry_and_formula_publication() {
    let document = grid(2_000, 2);
    let geometry = geometry(&document);
    let mut resident = Some(DesignerRuntime::from_document(document, OCCURRENCE).unwrap());
    let full_range = translate(&mut resident, "SUM(B2:B2001)", &geometry);
    assert_eq!(
        full_range["error"]["code"], "request_too_large",
        "{full_range}"
    );

    let row = &geometry["rows"][1];
    let cropped = json!({"header_row": 2, "rows": [row], "columns": geometry["columns"]});
    let reference = translate(&mut resident, "B3", &cropped);
    assert_eq!(
        reference["response"]["payload"]["source"], "[.c01]",
        "{reference}"
    );

    let constant = translate(&mut resident, "1+2", &cropped);
    assert_eq!(constant["status"], "ok", "{constant}");
    let target = json!({"entity": row, "field": "c00"});
    let reply = publish(
        &mut resident,
        &constant["response"]["payload"]["source"],
        &target,
    );
    assert_eq!(reply["status"], "error", "{reply}");
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Text capacity requires a bounded Text/Number rectangle"),
        "{reply}"
    );
}

#[test]
fn working_size_translates_and_evaluates_formulas_on_ten_thousand_rows() {
    let document = grid(10_000, 30);
    let geometry = geometry(&document);
    let mut resident =
        Some(DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap());
    let target = json!({"entity": geometry["rows"][0], "field": "c00"});

    for (text, expected_source, expected_value) in [
        ("1+2", "1+2", 3.0),
        ("SUM(B2:B10001)", "[grid.c01:total]", 20_000.0),
    ] {
        let translation = translate(&mut resident, text, &geometry);
        let source = &translation["response"]["payload"]["source"];
        assert_eq!(source, expected_source, "{translation}");
        let publication = publish(&mut resident, source, &target);
        assert_eq!(publication["status"], "ok", "{publication}");
        let request = json!({
            "type": "query_fields",
            "expected_revision": publication["response"]["payload"]["resulting_revision"],
            "fields": [target]
        });
        let fields = call(&mut resident, &request);
        assert_eq!(
            fields["response"]["payload"]["fields"][0]["calculated"]["value"].as_f64(),
            Some(expected_value),
            "{fields}"
        );
    }
}
