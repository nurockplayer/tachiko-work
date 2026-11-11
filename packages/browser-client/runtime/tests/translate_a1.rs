use serde_json::Value as Json;
use std::collections::BTreeMap;
use tachiko_designer_runtime::{DesignerRuntime, process_wire_request};
use tachiko_workspace_engine::{
    Date, Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
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
                        date_format: None,
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
        ("1+SUM(D2:D4)", "formula_cycle", 2, 12),
        ("1+AVERAGE(D2:D4)", "formula_cycle", 2, 16),
        ("1+COUNT(D2:D4)", "formula_cycle", 2, 14),
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
fn translate_a1_qualified_current_collection_keeps_relative_row() {
    let mut rt = Some(fixture());
    for (text, source) in [("Items!B3", "[.qty]"), ("Items!$B$3", "[r2.qty]")] {
        let mut input = request(text);
        input["geometry"]["sheets"] = serde_json::from_str(
            r#"{"Items":{"collection_id":"Items","header_row":1,"rows":["r1","r2","r3"],"columns":["name","qty","price","total"]}}"#,
        )
        .unwrap();
        let reply = call(&mut rt, &input);
        assert_eq!(reply["status"], "ok", "{text}: {reply}");
        assert_eq!(reply["response"]["payload"]["source"], source, "{text}");
        assert_eq!(
            reply["response"]["payload"]["notes"],
            serde_json::from_str::<Json>("[]").unwrap(),
            "{text}"
        );
    }
}

#[test]
fn translate_a1_scalar_extremum_preserves_excel_blank_semantics() {
    for (text, price) in [
        ("MIN(B3,C3)", 2.0),
        ("MAX(B3,C3)", 2.0),
        ("MIN (B3,C3)", 2.0),
        ("MAX (B3,C3)", -2.0),
        ("mIn \t(B3,C3)", 2.0),
        ("mAx \t(B3,C3)", -2.0),
    ] {
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
        document
            .entities
            .get_mut("r2")
            .unwrap()
            .fields
            .insert("price".into(), Value::Number(Number::new(price).unwrap()));
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
            Some(price),
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

#[test]
#[allow(clippy::too_many_lines)] // Preserve the brief's ordered conditional-call table.
fn translate_a1_conditional_aggregates_translate_whole_columns() {
    let mut rt = Some(fixture());
    for (text, source) in [
        (
            "SUMIF(B2:B4,B3,C2:C4)",
            "SUMIF([items.qty:column],[.qty],[items.price:column])",
        ),
        (
            "sumif(B2:B4,B3,C2:C4)",
            "SUMIF([items.qty:column],[.qty],[items.price:column])",
        ),
        (
            "SUMIF($B$2:$B$4,$B$3,$C$2:$C$4)",
            "SUMIF([items.qty:column],[r2.qty],[items.price:column])",
        ),
        (
            "SUMIF(B2:B4,B$3,C2:C4)",
            "SUMIF([items.qty:column],[r2.qty],[items.price:column])",
        ),
        (
            "SUMIF(B2:B4,\">=5\",C2:C4)",
            "SUMIF([items.qty:column],\">=5\",[items.price:column])",
        ),
        (
            "SUMIF(B2:B4,\"\",C2:C4)",
            "SUMIF([items.qty:column],\"\",[items.price:column])",
        ),
        ("SUMIF(B2:B4,\">=5\")", "SUMIF([items.qty:column],\">=5\")"),
        (
            "COUNTIF(B2:B4,\"<>\")",
            "COUNTIF([items.qty:column],\"<>\")",
        ),
        ("COUNTIF(B2:B4,5)", "COUNTIF([items.qty:column],5)"),
        (
            "SUMIFS(C2:C4,A2:A4,B3)",
            "SUMIFS([items.price:column],[items.name:column],[.qty])",
        ),
        (
            "AVERAGEIF(C2:C4,B3,B2:B4)",
            "AVERAGEIF([items.price:column],[.qty],[items.qty:column])",
        ),
        (
            "AVERAGEIFS(C2:C4,B2:B4,\"2\")",
            "AVERAGEIFS([items.price:column],[items.qty:column],\"2\")",
        ),
        (
            "SUMIFS(C2:C4,A2:A4,\"5\")",
            "SUMIFS([items.price:column],[items.name:column],\"5\")",
        ),
        (
            "COUNTIFS(A2:A4,B3,C2:C4,\"<>\")",
            "COUNTIFS([items.name:column],[.qty],[items.price:column],\"<>\")",
        ),
        (
            "IF(SUMIF(B2:B4,B3,C2:C4)>0,1,2)",
            "if(SUMIF([items.qty:column],[.qty],[items.price:column])>0,1,2)",
        ),
    ] {
        ok(&mut rt, text, source, &["range_tracks_collection"]);
    }
    let mut cross = request("SUMIFS(Other!A2:A2,Other!A2:A2,B3)");
    cross["geometry"]["sheets"] = serde_json::from_str(
        r#"{"Other":{"collection_id":"Other","header_row":1,"rows":["o1"],"columns":["x"]}}"#,
    )
    .unwrap();
    let reply = call(&mut rt, &cross);
    assert_eq!(
        reply["response"]["payload"]["source"], "SUMIFS([other.x:column],[other.x:column],[.qty])",
        "{reply}"
    );
    assert_eq!(
        reply["response"]["payload"]["notes"],
        serde_json::from_str::<Json>(r#"["range_tracks_collection"]"#).unwrap(),
        "{reply}"
    );
    for (text, code) in [
        ("SUMIF(B2:B4,\"Food\",C2:C4)", "unsupported_criterion"),
        ("SUMIF(B2:B3,B3,C2:C4)", "unsupported_range"),
        ("SUMIF(B2:B4,B3,C1:C4)", "unsupported_range"),
        ("SUMIF(B1:B4,B3,C1:C4)", "unsupported_range"),
        // A criterion is a number or one cell, the forms XLSX carries.
        ("COUNTIF(B2:B4,MIN(B3,5))", "unsupported_criterion"),
        ("COUNTIF(B2:B4,B3*2)", "unsupported_criterion"),
        ("SUMIF(B2:B4)", "unsupported_function"),
        ("SUMIFS(C2:C4,A2:A4)", "unsupported_function"),
        (
            format!("COUNTIFS({})", ["B2:B4,5"; 17].join(",")).as_str(),
            "unsupported_function",
        ),
    ] {
        refused(&mut rt, &request(text), code);
    }
    refused(
        &mut rt,
        &cross_with_other(&cross, "SUMIFS(B2:B4,Other!A2:A2,B3)"),
        "unsupported_range",
    );
}

fn cross_with_other(base: &Json, text: &str) -> Json {
    let mut request = request(text);
    request["geometry"]["sheets"] = base["geometry"]["sheets"].clone();
    request
}

#[test]
fn translate_a1_conditional_criterion_spans_and_date_rules() {
    let mut rt = Some(fixture());
    for (text, code, start, end) in [
        (
            "SUMIF(B2:B4,\"Food\",C2:C4)",
            "unsupported_criterion",
            12,
            18,
        ),
        (
            "1+SUMIF(B2:B4,\"Food\",C2:C4)",
            "unsupported_criterion",
            14,
            20,
        ),
        (
            "SUMIF(B2:B4,\">\"&B3,C2:C4)",
            "unsupported_operator",
            15,
            16,
        ),
        ("1+COUNTIF(B2:B4,B3*2)", "unsupported_criterion", 16, 20),
    ] {
        let reply = call(&mut rt, &request(text));
        assert_eq!(reply["error"]["code"], code, "{text}: {reply}");
        assert_eq!(reply["error"]["span"]["start"], start, "{text}: {reply}");
        assert_eq!(reply["error"]["span"]["end"], end, "{text}: {reply}");
    }
}

fn date_runtime() -> DesignerRuntime {
    let document = date_document();
    DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000").unwrap()
}

fn date_document() -> Document {
    let mut document = fixture_document();
    let schema = document.schemas.get_mut("Items").unwrap();
    schema.fields.insert(
        "when".into(),
        FieldDefinition {
            id: "when".into(),
            key: "when".into(),
            display_name: None,
            number_format: None,
            date_format: None,
            field_type: FieldType::Date,
            required: true,
            constraint: FieldConstraint::None,
        },
    );
    for row in ["r1", "r2", "r3"] {
        document.entities.get_mut(row).unwrap().fields.insert(
            "when".into(),
            Value::Date(Date::parse("2026-09-05").unwrap()),
        );
    }
    document
}

fn date_request(text: &str) -> Json {
    let mut request = request(text);
    request["geometry"]["columns"] =
        serde_json::from_str(r#"["name","qty","price","total","when"]"#).unwrap();
    request
}

#[test]
fn translate_a1_conditional_number_criteria_cannot_test_a_date_column() {
    let mut rt = Some(date_runtime());
    for (text, start, end) in [
        ("SUMIF(E2:E4,5,B2:B4)", 12, 13),
        ("SUMIF(E2:E4,B2,B2:B4)", 12, 14),
    ] {
        let reply = call(&mut rt, &date_request(text));
        assert_eq!(reply["error"]["code"], "formula_type_mismatch", "{reply}");
        assert_eq!(reply["error"]["span"]["start"], start, "{reply}");
        assert_eq!(reply["error"]["span"]["end"], end, "{reply}");
    }
    // A Text cell counts by its content: Excel compares this with date serials.
    let mut document = date_document();
    document
        .entities
        .get_mut("r1")
        .unwrap()
        .fields
        .insert("name".into(), Value::Text(">=45000".into()));
    let mut text_cell = Some(
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000").unwrap(),
    );
    let reply = call(&mut text_cell, &date_request("COUNTIF(E2:E4,A2)"));
    assert_eq!(reply["error"]["code"], "formula_type_mismatch", "{reply}");
    // Excel matches a Date cell with a number's serial; Tachiko doesn't.
    let reply = call(&mut rt, &date_request("COUNTIF(B2:B4,E2)"));
    assert_eq!(reply["error"]["code"], "formula_type_mismatch", "{reply}");
    // Text that isn't an exact date refuses, since Excel reads more spellings.
    let reply = call(&mut rt, &date_request("SUMIF(E2:E4,A2,B2:B4)"));
    assert_eq!(reply["error"]["code"], "formula_type_mismatch", "{reply}");
    let reply = call(&mut rt, &date_request("COUNTIF(E2:E4,\"\")"));
    assert_eq!(reply["status"], "ok", "{reply}");
    let mut document = date_document();
    document
        .entities
        .get_mut("r1")
        .unwrap()
        .fields
        .insert("name".into(), Value::Text("2026-09-05".into()));
    let mut date_text = Some(
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000").unwrap(),
    );
    let reply = call(&mut date_text, &date_request("SUMIF(E2:E4,A2,B2:B4)"));
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(
        reply["response"]["payload"]["source"],
        "SUMIF([items.when:column],[r1.name],[items.qty:column])"
    );
}

#[test]
fn translate_a1_leaves_column_rules_to_the_commit() {
    let mut document = fixture_document();
    let constraint = tachiko_workspace_engine::FieldConstraint::NumberInclusiveRange {
        min: Number::new(1.0).unwrap(),
        max: Number::new(5.0).unwrap(),
    };
    document
        .schemas
        .get_mut("Items")
        .unwrap()
        .fields
        .get_mut("total")
        .unwrap()
        .constraint = constraint;
    let mut rt = Some(
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000000").unwrap(),
    );
    // A valid formula outside the rule is translated; the rule is judged at commit.
    ok(&mut rt, "2*3", "2*3", &[]);
    ok(&mut rt, "2+1", "2+1", &[]);
    refused(&mut rt, &request("D3*2"), "formula_cycle");
    refused(&mut rt, &request("2*"), "formula_syntax");
    let update: Json = serde_json::from_str(r#"{"type":"formula_update","expected_revision":"resident/0","target":{"entity":"r2","field":"total"},"source":"2*3"}"#).unwrap();
    let reply = call(&mut rt, &update);
    assert_eq!(reply["status"], "error", "{reply}");
    assert_eq!(
        reply["error"]["code"], "field_constraint_violation",
        "{reply}"
    );
}
