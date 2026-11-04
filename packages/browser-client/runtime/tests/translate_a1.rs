use serde_json::Value as Json;
use std::collections::BTreeMap;
use tachiko_designer_runtime::{DesignerRuntime, process_wire_request};
use tachiko_workspace_engine::{
    Document, Entity, FieldDefinition, FieldType, Number, Schema, Value,
};

fn fixture_document() -> Document {
    let mut doc = Document::empty("translate", "Translate");
    for (schema, fields, rows) in [
        (
            "Items",
            vec![
                ("name", FieldType::Text),
                ("qty", FieldType::Number),
                ("price", FieldType::Number),
                ("total", FieldType::Number),
            ],
            vec!["r1", "r2", "r3"],
        ),
        ("Other", vec![("x", FieldType::Number)], vec!["o1"]),
    ] {
        let definitions = fields
            .iter()
            .map(|(name, kind)| {
                (
                    (*name).into(),
                    FieldDefinition {
                        id: (*name).into(),
                        key: (*name).into(),
                        display_name: None,
                        number_format: None,
                        field_type: kind.clone(),
                        required: true,
                        constraint: tachiko_workspace_engine::FieldConstraint::None,
                    },
                )
            })
            .collect();
        doc.schemas.insert(
            schema.into(),
            Schema {
                id: schema.into(),
                key: schema.to_ascii_lowercase().into(),
                display_name: Some(schema.into()),
                field_order: None,
                fields: definitions,
            },
        );
        for row in rows {
            let values = fields
                .iter()
                .map(|(name, kind)| {
                    (
                        (*name).into(),
                        if *kind == FieldType::Text {
                            Value::Text("x".into())
                        } else {
                            Value::Number(Number::new(2.0).unwrap())
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            doc.entities.insert(
                row.into(),
                Entity {
                    id: row.into(),
                    key: row.into(),
                    schema: schema.into(),
                    fields: values,
                },
            );
        }
    }
    doc
}

fn fixture() -> DesignerRuntime {
    DesignerRuntime::from_document(fixture_document(), "00000000-0000-4000-8000-000000000000")
        .unwrap()
}

fn request(text: &str) -> Json {
    let mut request: Json = serde_json::from_str(r#"{"type":"translate_a1_formula","expected_revision":"resident/0","collection_id":"Items","row_entity_id":"r2","target_field_id":"total","text":"","geometry":{"header_row":1,"rows":["r1","r2","r3"],"columns":["name","qty","price","total"]}}"#).unwrap();
    request["text"] = text.into();
    request
}
fn call(runtime: &mut Option<DesignerRuntime>, request: &Json) -> Json {
    serde_json::from_slice(&process_wire_request(
        runtime,
        &serde_json::to_vec(request).unwrap(),
    ))
    .unwrap()
}
fn ok(runtime: &mut Option<DesignerRuntime>, text: &str, source: &str, notes: &[&str]) {
    let reply = call(runtime, &request(text));
    assert_eq!(reply["status"], "ok", "{text}: {reply}");
    assert_eq!(reply["response"]["payload"]["source"], source);
    assert_eq!(
        reply["response"]["payload"]["notes"],
        serde_json::to_value(notes).unwrap()
    );
}
fn refused(runtime: &mut Option<DesignerRuntime>, request: &Json, code: &str) {
    let reply = call(runtime, request);
    assert_eq!(reply["status"], "error", "{reply}");
    assert_eq!(reply["error"]["code"], code, "{reply}");
    assert!(reply["error"]["span"].is_object(), "{reply}");
}

#[test]
#[allow(clippy::too_many_lines)] // Preserve the Steward's ordered 20-case acceptance table.
fn translate_a1_steward_acceptance() {
    let mut rt = Some(fixture());
    ok(&mut rt, "B3*C3", "[.qty]*[.price]", &[]);
    ok(
        &mut rt,
        "B2*C3",
        "[r1.qty]*[.price]",
        &["relative_copy_semantics_changed"],
    );
    ok(&mut rt, "B$3*C3", "[r2.qty]*[.price]", &[]);
    ok(&mut rt, "$B3*C3", "[.qty]*[.price]", &[]);
    ok(&mut rt, "$B$3*C3", "[r2.qty]*[.price]", &[]);
    for text in ["SUM(B2:B4)", "SUM(B1:B4)"] {
        ok(
            &mut rt,
            text,
            "[items.qty:total]",
            &["range_tracks_collection"],
        );
    }
    ok(
        &mut rt,
        "AVERAGE(C2:C4)",
        "[items.price:average]",
        &["range_tracks_collection"],
    );
    ok(
        &mut rt,
        "COUNT(B2:B4)",
        "[items.qty:count]",
        &["range_tracks_collection"],
    );
    refused(&mut rt, &request("SUM(B2:B3)"), "unsupported_range");
    refused(&mut rt, &request("MIN(B2:B4)"), "unsupported_range");
    let mut filtered = request("SUM(B2:B3)");
    filtered["row_entity_id"] = "r1".into();
    filtered["geometry"]["rows"] = serde_json::from_str(r#"["r1","r3"]"#).unwrap();
    refused(&mut rt, &filtered, "unsupported_range");
    filtered["text"] = "B3*2".into();
    let reply = call(&mut rt, &filtered);
    assert_eq!(
        reply["response"]["payload"]["source"], "[r3.qty]*2",
        "{reply}"
    );
    assert_eq!(
        reply["response"]["payload"]["notes"][0],
        "relative_copy_semantics_changed"
    );
    for (text, code) in [
        ("A3&\"x\"", "unsupported_operator"),
        ("VLOOKUP(B3,A2:D4,2)", "unsupported_function"),
        ("B1*2", "header_reference"),
        ("B9*2", "out_of_bounds"),
        ("Z3*2", "out_of_bounds"),
        ("Other!A2", "unknown_sheet"),
        ("A3*2", "formula_type_mismatch"),
        ("D3*2", "formula_cycle"),
    ] {
        refused(&mut rt, &request(text), code);
    }
    let mut cross = request("Other!A2");
    cross["geometry"]["sheets"] = serde_json::from_str(
        r#"{"Other":{"collection_id":"Other","header_row":1,"rows":["o1"],"columns":["x"]}}"#,
    )
    .unwrap();
    let reply = call(&mut rt, &cross);
    assert_eq!(reply["response"]["payload"]["source"], "[o1.x]", "{reply}");
    ok(&mut rt, "B3*10%", "[.qty]*10%", &[]);
    for geometry in [
        r#"{"header_row":1,"rows":["r1","r1","r3"],"columns":["name","qty","price","total"]}"#,
        r#"{"header_row":1,"rows":["r1","r2","r3"],"columns":["name","qty","price","missing"]}"#,
        r#"{"header_row":1,"rows":["r1","o1","r3"],"columns":["name","qty","price","total"]}"#,
    ] {
        let mut bad = request("B3");
        bad["geometry"] = serde_json::from_str(geometry).unwrap();
        refused(&mut rt, &bad, "invalid_geometry");
    }
    let mut stale = request("B3");
    stale["expected_revision"] = "resident/older".into();
    refused(&mut rt, &stale, "stale_revision");
    let before = rt
        .as_ref()
        .unwrap()
        .export_project_v3("resident/0")
        .unwrap()
        .bytes;
    let a = call(&mut rt, &request("B3*C3"));
    let b = call(&mut rt, &request("B3*C3"));
    assert_eq!(a, b);
    assert_eq!(
        before,
        rt.as_ref()
            .unwrap()
            .export_project_v3("resident/0")
            .unwrap()
            .bytes
    );
}

#[test]
fn translate_a1_geometry_and_spans() {
    let mut rt = Some(fixture());
    for (text, code, start, end) in [
        ("2+B9", "out_of_bounds", 2, 4),
        ("2+A3", "formula_type_mismatch", 2, 4),
        ("2+D3", "formula_cycle", 2, 4),
        ("B3&2", "unsupported_operator", 2, 3),
        ("BOGUS(B3)", "unsupported_function", 0, 5),
        ("B3+)", "formula_syntax", 3, 4),
    ] {
        let reply = call(&mut rt, &request(text));
        assert_eq!(reply["error"]["code"], code, "{reply}");
        assert_eq!(reply["error"]["span"]["start"], start, "{reply}");
        assert_eq!(reply["error"]["span"]["end"], end, "{reply}");
    }
    for geometry in [
        r#"{"header_row":0,"rows":["r1","r2","r3"],"columns":["name","qty","price","total"]}"#,
        r#"{"header_row":1,"rows":["r1","r2","r3"],"columns":["name","qty","price"]}"#,
        r#"{"header_row":1,"rows":["r1","r2","r3"],"columns":["name","qty","price","qty"]}"#,
        r#"{"header_row":1,"rows":["missing","r2","r3"],"columns":["name","qty","price","total"]}"#,
    ] {
        let mut bad = request("B3");
        bad["geometry"] = serde_json::from_str(geometry).unwrap();
        refused(&mut rt, &bad, "invalid_geometry");
    }
    let mut bad = request("B3");
    bad["target_field_id"] = "missing".into();
    refused(&mut rt, &bad, "invalid_geometry");
    bad = request("B3");
    bad["row_entity_id"] = "o1".into();
    refused(&mut rt, &bad, "invalid_geometry");
    bad = request("B3");
    bad["geometry"]["sheets"] = serde_json::from_str(r#"{"Other":{"collection_id":"Other","header_row":1,"rows":["o1"],"columns":["x"]},"other":{"collection_id":"Other","header_row":1,"rows":["o1"],"columns":["x"]}}"#).unwrap();
    refused(&mut rt, &bad, "invalid_geometry");
}

#[test]
fn translate_a1_engine_functions_and_numeric_syntax() {
    let mut rt = Some(fixture());
    for (text, source) in [
        (
            "IF(AND(B3>0,NOT(C3=0)),ROUND(B3/C3,2),0)",
            "if(and([.qty]>0,not([.price]=0)),round([.qty]/[.price],2),0)",
        ),
        (
            "IF(OR(B3<>0,C3<=2),1,0)",
            "if(or([.qty]<>0,[.price]<=2),1,0)",
        ),
        ("IF(ISBLANK(A3),0,1)", "if(isblank([.name]),0,1)"),
        ("(B3+1.2e-3)*-2", "([.qty]+1.2e-3)*-2"),
    ] {
        ok(&mut rt, text, source, &[]);
    }
    for text in ["MIN(B3,C3)", "MAX(B3,C3)"] {
        let reply = call(&mut rt, &request(text));
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(
            reply["response"]["payload"]["source"]
                .as_str()
                .unwrap()
                .contains("[.qty]")
        );
    }
    for (text, code) in [
        ("B3+", "formula_syntax"),
        ("=B3", "formula_syntax"),
        ("MAX(B2:B4)", "unsupported_range"),
        ("SUM(A2:A4)", "unsupported_range"),
    ] {
        refused(&mut rt, &request(text), code);
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn translate_a1_native_host_dispatch() {
    let rt = fixture();
    let project = rt.export_project_v3("resident/0").unwrap();
    let mut host = tachiko_designer_runtime::host_bridge::DesignerHost::new();
    let opened: Json = serde_json::from_slice(
        &host.open_project(&project.bytes, b"00000000-0000-4000-8000-000000000001"),
    )
    .unwrap();
    assert_eq!(opened["status"], "ok", "{opened}");
    for (text, code) in [("B3*C3", None), ("SUM(B2:B3)", Some("unsupported_range"))] {
        let reply: Json =
            serde_json::from_slice(&host.request(&serde_json::to_vec(&request(text)).unwrap()))
                .unwrap();
        if let Some(code) = code {
            assert_eq!(reply["error"]["code"], code, "{reply}");
        } else {
            assert_eq!(
                reply["response"]["payload"]["source"], "[.qty]*[.price]",
                "{reply}"
            );
        }
    }
}

#[test]
fn translate_a1_existing_formula_and_indirect_cycle() {
    let mut rt = Some(fixture());
    let update: Json = serde_json::from_str(r#"{"type":"formula_update","expected_revision":"resident/0","target":{"entity":"r2","field":"qty"},"source":"[.total]*2"}"#).unwrap();
    let reply = call(&mut rt, &update);
    assert_eq!(reply["status"], "ok", "{reply}");
    let revision = reply["response"]["payload"]["resulting_revision"].clone();
    let mut cycle = request("2+B3");
    cycle["expected_revision"] = revision.clone();
    let reply = call(&mut rt, &cycle);
    assert_eq!(reply["error"]["code"], "formula_cycle", "{reply}");
    assert_eq!(reply["error"]["span"]["start"], 2, "{reply}");
    let mut existing = request("D3*2");
    existing["target_field_id"] = "qty".into();
    existing["expected_revision"] = revision;
    let reply = call(&mut rt, &existing);
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(reply["response"]["payload"]["source"], "[.total]*2");
    refused(&mut rt, &request("B3"), "stale_revision");
}

#[test]
fn translate_a1_scalar_extremum_preserves_excel_blank_semantics() {
    for text in ["MIN(B3,C3)", "MAX(B3,C3)"] {
        let mut document = fixture_document();
        document
            .schemas
            .get_mut("Items")
            .unwrap()
            .fields
            .get_mut("qty")
            .unwrap()
            .required = false;
        document
            .entities
            .get_mut("r2")
            .unwrap()
            .fields
            .remove("qty");
        let mut rt = Some(
            DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000")
                .unwrap(),
        );
        let translated = call(&mut rt, &request(text));
        assert_eq!(translated["status"], "ok", "{translated}");
        let source = translated["response"]["payload"]["source"].clone();
        assert!(
            source
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .contains("isblank"),
            "{source}"
        );
        let mut update: Json = serde_json::from_str(r#"{"type":"formula_update","expected_revision":"resident/0","target":{"entity":"r2","field":"total"},"source":""}"#).unwrap();
        update["source"] = source;
        let reply = call(&mut rt, &update);
        assert_eq!(reply["status"], "ok", "{reply}");
        let mut query: Json = serde_json::from_str(r#"{"type":"query_fields","expected_revision":"","fields":[{"entity":"r2","field":"total"}]}"#).unwrap();
        query["expected_revision"] = reply["response"]["payload"]["resulting_revision"].clone();
        let reply = call(&mut rt, &query);
        assert_eq!(
            reply["response"]["payload"]["fields"][0]["calculated"]["value"].as_f64(),
            Some(2.0),
            "{reply}"
        );
    }
}

#[test]
fn translate_a1_review_coordinate_operator_and_unicode_regressions() {
    let mut rt = Some(fixture());
    for text in [
        "B0*2",
        "B18446744073709551616*2",
        "ZZZZZZZZZZZZZZZZZZZZZZ3*2",
    ] {
        refused(&mut rt, &request(text), "out_of_bounds");
        let reply = call(&mut rt, &request(text));
        assert_eq!(reply["error"]["span"]["start"], 0, "{reply}");
        assert_eq!(
            usize::try_from(reply["error"]["span"]["end"].as_u64().unwrap()).unwrap(),
            text.find('*').unwrap()
        );
    }
    refused(&mut rt, &request("B$z*2"), "formula_syntax");
    for text in ["B3^2", "B3#", "B3@2"] {
        let reply = call(&mut rt, &request(text));
        assert_eq!(reply["error"]["code"], "unsupported_operator", "{reply}");
        assert_eq!(reply["error"]["span"]["start"], 2, "{reply}");
        assert_eq!(reply["error"]["span"]["end"], 3, "{reply}");
    }
    let mut cross = request("'été'!A2");
    cross["geometry"]["sheets"] = serde_json::from_str(
        r#"{"Été":{"collection_id":"Other","header_row":1,"rows":["o1"],"columns":["x"]}}"#,
    )
    .unwrap();
    let reply = call(&mut rt, &cross);
    assert_eq!(reply["response"]["payload"]["source"], "[o1.x]", "{reply}");
    cross["geometry"]["sheets"]["été"] = cross["geometry"]["sheets"]["Été"].clone();
    refused(&mut rt, &cross, "invalid_geometry");
}

#[test]
fn translate_a1_review_guard_expansion_has_source_token_span() {
    let mut document = fixture_document();
    for field in ["qty", "price"] {
        document
            .schemas
            .get_mut("Items")
            .unwrap()
            .fields
            .get_mut(field)
            .unwrap()
            .required = false;
    }
    let mut rt = Some(
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000").unwrap(),
    );
    let text = format!("1+{}", vec!["MIN(B3,C3)"; 30].join("+"));
    let reply = call(&mut rt, &request(&text));
    assert_eq!(reply["error"]["code"], "formula_syntax", "{reply}");
    assert_eq!(reply["error"]["span"]["start"], 2, "{reply}");
    assert_eq!(reply["error"]["span"]["end"], 5, "{reply}");
}
