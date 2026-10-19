//! ADR-0045 Rust/wire qualification; public TypeScript exposure is slice 2.
use serde_json::{Value, json};
use tachiko_designer_runtime::{DesignerRuntime, process_wire_request};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000551";

struct Journey {
    runtime: Option<DesignerRuntime>,
    transcript: Vec<Value>,
    collection: String,
}

impl Default for Journey {
    fn default() -> Self {
        Self {
            runtime: None,
            transcript: Vec::new(),
            collection: "items".into(),
        }
    }
}

impl Journey {
    fn request(&mut self, request: Value) -> Value {
        let reply: Value = serde_json::from_slice(&process_wire_request(
            &mut self.runtime,
            &serde_json::to_vec(&request).unwrap(),
        ))
        .unwrap();
        let mut record = serde_json::Map::new();
        record.insert("request".into(), request);
        record.insert("reply".into(), reply.clone());
        self.transcript.push(Value::Object(record));
        reply
    }

    fn ok(&mut self, request: Value) -> Value {
        let reply = self.request(request);
        assert_eq!(reply["status"], "ok", "{reply}");
        reply["response"]["payload"].clone()
    }

    fn table(&mut self) -> Value {
        self.ok(json!({"type":"query_table","collection":self.collection}))
    }

    fn tree(&mut self) -> Vec<u8> {
        let table = self.table();
        let runtime = self.runtime.as_ref().unwrap();
        let revision = table["revision"].as_str().unwrap();
        if table["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|column| column["field_type"] == "date")
        {
            // The legacy canonical-v1 export does not carry Date. Its existing
            // private v2 project carrier still proves exact snapshot equality.
            runtime.export_project(revision).unwrap().bytes
        } else {
            runtime.export_canonical_tree(revision).unwrap().bytes
        }
    }

    fn mutate(&mut self, mut request: Value) {
        let table = self.table();
        request["expected_revision"] = table["revision"].clone();
        self.ok(request);
    }

    fn refused(&mut self, mut request: Value) -> Value {
        let before = self.tree();
        let table = self.table();
        if request.get("expected_revision").is_none() {
            request["expected_revision"] = table["revision"].clone();
        }
        let reply = self.request(request);
        assert_eq!(reply["status"], "error", "{reply}");
        assert_eq!(self.table(), table);
        assert_eq!(self.tree(), before);
        reply
    }

    fn round_trip(&mut self, request: Value) {
        let before = self.tree();
        self.mutate(request);
        let after = self.tree();
        assert_ne!(before, after);
        self.mutate(json!({"type":"undo"}));
        assert_eq!(
            self.tree(),
            before,
            "Undo restores presence and values exactly"
        );
        self.mutate(json!({"type":"redo"}));
        assert_eq!(self.tree(), after, "Redo is one exact operation");
    }

    fn fixture() -> Self {
        let mut journey = Self::default();
        journey.ok(
            json!({"type":"new_table","occurrence_id":OCCURRENCE,"name":"items","columns":[
                {"name":"label","field_type":"text"},
                {"name":"note","field_type":"text","required":false},
                {"name":"amount","field_type":"number","required":false}
            ]}),
        );
        journey
    }
}

#[test]
#[allow(clippy::too_many_lines)] // A sequential native/WASM transcript also proves action-sized history.
fn wire_journey_preserves_sparse_values_and_atomic_history() {
    let mut j = Journey::fixture();
    let t = j.table();
    let label = &t["columns"][0]["id"];
    let note = &t["columns"][1]["id"];
    let amount = &t["columns"][2]["id"];
    assert_eq!(t["columns"][0]["required"], true);
    assert_eq!(t["columns"][1]["required"], false);
    j.refused(json!({"type":"insert_row","collection":"items","initializers":[]}));
    j.round_trip(json!({"type":"insert_row","collection":"items","initializers":[{"field":label,"input":{"kind":"text","value":"first"}}]}));
    let row = j.table()["rows"][0]["id"].clone();
    let target = |field: &Value| json!({"entity":row,"field":field});
    let table = j.table();
    assert_eq!(table["native_table_profile"], true);
    assert_eq!(table["rows"][0]["fields"].as_array().unwrap().len(), 1);
    assert_eq!(table["rows"][0]["blank_fields"], json!([note, amount]));
    let blank_query = j.ok(json!({"type":"query_fields","expected_revision":table["revision"],"fields":[target(note)]}));
    assert_eq!(blank_query["fields"][0]["blank"], true);
    let no_change =
        j.refused(json!({"type":"edit_scalar","target":target(note),"input":{"kind":"clear"}}));
    assert_eq!(no_change["error"]["code"], "no_change");
    j.mutate(json!({"type":"undo"}));
    assert_eq!(j.table()["rows"], json!([]));
    j.mutate(json!({"type":"redo"}));
    j.round_trip(json!({"type":"edit_cells","edits":[
        {"target":target(note),"input":{"kind":"text","value":""}},
        {"target":target(amount),"input":{"kind":"number","input":"0"}}
    ]}));
    let filled = j.table();
    assert_ne!(filled["rows"][0]["fields"][1]["blank"], true);
    assert_eq!(filled["rows"][0]["fields"][1]["stored"]["value"], "");
    assert_eq!(filled["rows"][0]["fields"][2]["stored"]["value"], 0.0);
    j.refused(json!({"type":"edit_cells","edits":[
        {"target":target(note),"input":{"kind":"clear"}},
        {"target":target(amount),"input":{"kind":"text","value":"wrong"}}
    ]}));
    j.refused(json!({"type":"edit_scalar","target":target(label),"input":{"kind":"clear"}}));
    j.round_trip(json!({"type":"edit_cells","edits":[
        {"target":target(note),"input":{"kind":"clear"}},
        {"target":target(amount),"input":{"kind":"number","input":"5"}}
    ]}));
    j.refused(
        json!({"type":"set_column_requiredness","collection":"items","field":note,"required":true}),
    );
    j.round_trip(json!({"type":"set_column_requiredness","collection":"items","field":amount,"required":true}));
    j.refused(json!({"type":"edit_scalar","target":target(amount),"input":{"kind":"clear"}}));
    j.round_trip(json!({"type":"set_column_requiredness","collection":"items","field":amount,"required":false}));
    j.refused(json!({"type":"set_column_requiredness","collection":"items","field":amount,"required":false}));
    j.refused(json!({"type":"set_column_requiredness","collection":"items","field":amount,"required":true,"expected_revision":"resident/0"}));
    j.round_trip(json!({"type":"paste_cells","collection":"items","start_entity":row,"start_field":note,"rows":[["",{"kind":"clear"}]]}));
    assert_eq!(j.table()["rows"][0]["fields"][1]["stored"]["value"], "");
    assert_eq!(j.table()["rows"][0]["blank_fields"], json!([amount]));
    j.refused(json!({"type":"paste_cells","collection":"items","start_entity":row,"start_field":note,"rows":[[{"kind":"clear"},""]]}));
    j.round_trip(json!({"type":"paste_cells","collection":"items","start_entity":null,"start_field":label,"rows":[["second"]]}));
    let rows = j.table()["rows"].as_array().unwrap().clone();
    assert_eq!(rows[1]["blank_fields"], json!([note, amount]));
    j.round_trip(json!({"type":"add_column","collection":"items","name":"extra","field_type":"text","required":false}));
    j.round_trip(json!({"type":"add_column","collection":"items","name":"partial","field_type":"number","required":false,"initializers":[{"entity":row,"input":{"kind":"number","input":"7"}}]}));
    let table = j.table();
    let partial_index = table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|column| column["key"] == "partial")
        .unwrap();
    assert_eq!(
        table["rows"][0]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["target"]["field"] == table["columns"][partial_index]["id"])
            .unwrap()["stored"]["value"],
        7.0
    );
    assert!(
        table["rows"][1]["blank_fields"]
            .as_array()
            .unwrap()
            .contains(&table["columns"][partial_index]["id"])
    );
    for initializers in [
        json!([{"entity":"unknown","input":{"kind":"text","value":"x"}}]),
        json!([{"entity":row,"input":{"kind":"text","value":"x"}},{"entity":row,"input":{"kind":"text","value":"y"}}]),
        json!([{"entity":row,"input":{"kind":"number","input":"1"}}]),
    ] {
        j.refused(json!({"type":"add_column","collection":"items","name":"bad","field_type":"text","required":false,"initializers":initializers}));
    }
    let partial = table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|col| col["key"] == "partial")
        .unwrap()["id"]
        .clone();
    j.round_trip(json!({"type":"remove_column","collection":"items","field":partial}));
    j.refused(json!({"type":"add_column","collection":"items","name":"required_missing","field_type":"text","initializers":[]}));
    for kind in ["boolean", "date"] {
        j.refused(json!({"type":"add_column","collection":"items","name":"outside","field_type":kind,"required":false}));
        j.refused(json!({"type":"new_table","occurrence_id":OCCURRENCE,"name":"outside","columns":[{"name":"x","field_type":kind,"required":false}]}));
    }
    j.refused(json!({"type":"paste_cells","collection":"items","start_entity":null,"start_field":note,"rows":[["missing required label"]]}));
    // The actual WASM ABI replays these exact requests against native replies.
    if let Ok(path) = std::env::var("TACHIKO_OPTIONAL_TRANSCRIPT") {
        std::fs::write(path, serde_json::to_vec(&j.transcript).unwrap()).unwrap();
    }
}

#[test]
fn optional_rows_keep_the_128_row_gate() {
    let mut j = Journey::default();
    j.ok(json!({"type":"new_table","occurrence_id":OCCURRENCE,"name":"items","columns":[{"name":"label","field_type":"text","required":false}]}));
    let label = j.table()["columns"][0]["id"].clone();
    let rows = (0..128)
        .map(|index| vec![format!("row{index}")])
        .collect::<Vec<_>>();
    j.mutate(json!({"type":"paste_cells","collection":"items","start_entity":null,"start_field":label,"rows":rows}));
    assert_eq!(j.table()["rows"].as_array().unwrap().len(), 128);
    j.refused(json!({"type":"insert_row","collection":"items","initializers":[{"field":label,"input":{"kind":"text","value":"129"}}]}));
    j.refused(json!({"type":"paste_cells","collection":"items","start_entity":null,"start_field":label,"rows":[["129"]]}));
    if let Ok(path) = std::env::var("TACHIKO_OPTIONAL_TRANSCRIPT") {
        std::fs::write(
            format!("{path}.rowgate"),
            serde_json::to_vec(&j.transcript).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn runtime_clear_of_formula_input_refuses_whole_batch_and_preserves_history() {
    use std::collections::BTreeMap;
    use tachiko_workspace_engine::{
        Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
        Number, Schema, Value as Stored,
    };
    let mut doc = Document::empty("doc", "Formula consumer");
    for (schema, fields) in [
        (
            "items",
            vec![
                ("label", FieldType::Text, true),
                ("amount", FieldType::Number, false),
            ],
        ),
        ("summary", vec![("total", FieldType::Number, true)]),
    ] {
        let fields = fields
            .into_iter()
            .map(|(id, field_type, required)| {
                (
                    id.into(),
                    FieldDefinition {
                        id: id.into(),
                        key: id.into(),
                        display_name: None,
                        field_type,
                        required,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect();
        doc.schemas.insert(
            schema.into(),
            Schema {
                id: schema.into(),
                key: schema.into(),
                display_name: None,
                fields,
            },
        );
    }
    doc.entities.insert(
        "a".into(),
        Entity {
            id: "a".into(),
            key: "a".into(),
            schema: "items".into(),
            fields: BTreeMap::from([
                ("label".into(), Stored::Text("before".into())),
                ("amount".into(), Stored::Number(Number::new(3.0).unwrap())),
            ]),
        },
    );
    doc.entities.insert(
        "sum".into(),
        Entity {
            id: "sum".into(),
            key: "sum".into(),
            schema: "summary".into(),
            fields: BTreeMap::from([(
                "total".into(),
                Stored::Formula(Expression::Reference(FieldRef::new("a", "amount"))),
            )]),
        },
    );
    let mut j = Journey {
        runtime: Some(DesignerRuntime::from_document(doc, OCCURRENCE).unwrap()),
        ..Journey::default()
    };
    let initial = j
        .runtime
        .as_ref()
        .unwrap()
        .export_project("resident/0")
        .unwrap()
        .bytes;
    // No display names: this also exercises request-local v2 evidence selection.
    j.refused(json!({"type":"edit_cells","edits":[
        {"target":{"entity":"a","field":"label"},"input":{"kind":"text","value":"would publish a prefix"}},
        {"target":{"entity":"a","field":"amount"},"input":{"kind":"clear"}}
    ]}));
    j.refused(json!({"type":"paste_cells","collection":"items","start_entity":"a","start_field":"amount","rows":[[{"kind":"clear"}]]}));
    j.refused(json!({"type":"undo"}));
    j.round_trip(json!({"type":"set_column_requiredness","collection":"items","field":"amount","required":true}));
    j.round_trip(json!({"type":"set_column_requiredness","collection":"items","field":"amount","required":false}));
    if let Ok(path) = std::env::var("TACHIKO_OPTIONAL_TRANSCRIPT") {
        std::fs::write(
            format!("{path}.formula"),
            serde_json::to_vec(
                &json!({"project":initial,"occurrence_id":OCCURRENCE,"records":j.transcript}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn all_blank_insert_and_unchanged_batches_create_no_extra_history() {
    let mut j = Journey::default();
    j.ok(
        json!({"type":"new_table","occurrence_id":OCCURRENCE,"name":"items","columns":[
            {"name":"note","field_type":"text","required":false},
            {"name":"amount","field_type":"number","required":false}
        ]}),
    );
    j.round_trip(json!({"type":"insert_row","collection":"items","initializers":[]}));
    let table = j.table();
    let row = &table["rows"][0];
    assert_eq!(row["fields"], json!([]));
    assert_eq!(row["blank_fields"].as_array().unwrap().len(), 2);
    for request in [
        json!({"type":"edit_cells","edits":table["columns"].as_array().unwrap().iter().map(|column| json!({"target":{"entity":row["id"],"field":column["id"]},"input":{"kind":"clear"}})).collect::<Vec<_>>()}),
        json!({"type":"paste_cells","collection":"items","start_entity":row["id"],"start_field":table["columns"][0]["id"],"rows":[[{"kind":"clear"},{"kind":"clear"}]]}),
    ] {
        let error = j.refused(request);
        assert_eq!(error["error"]["code"], "no_change");
    }
    j.mutate(json!({"type":"undo"}));
    assert_eq!(j.table()["rows"], json!([]));
    j.refused(json!({"type":"undo"}));
    j.mutate(json!({"type":"redo"}));
    assert_eq!(j.table()["rows"], table["rows"]);
}

#[test]
fn imported_capacity_blank_set_clear_reopen_and_history_preserve_absence() {
    use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
    use tachiko_designer_runtime::{
        ImportFieldType, ImportSelection, import_workbook, open_project,
    };
    // Real 65-row import forces the capacity profile. The first Text is absent.
    let csv = format!("note,amount\n,10\n{}", "present,10\n".repeat(64));
    let workbook = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    let (runtime, imported) = import_workbook(
        &workbook,
        &ImportSelection {
            column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Number]],
            extra_columns: vec![vec![]],
        },
        OCCURRENCE,
    )
    .unwrap();
    let mut j = Journey {
        runtime: Some(runtime),
        collection: imported.opened.table.collection.key.clone(),
        ..Journey::default()
    };
    let initial = j
        .runtime
        .as_ref()
        .unwrap()
        .export_project("resident/0")
        .unwrap()
        .bytes;
    let table = j.table();
    assert_eq!(table["rows"].as_array().unwrap().len(), 65);
    let note = &table["columns"][0]["id"];
    let amount = &table["columns"][1]["id"];
    let entity = &table["rows"][0]["id"];
    assert_eq!(table["rows"][0]["blank_fields"], json!([note]));
    j.round_trip(json!({"type":"edit_cells","edits":[
        {"target":{"entity":entity,"field":note},"input":{"kind":"text","value":""}},
        {"target":{"entity":entity,"field":amount},"input":{"kind":"clear"}}
    ]}));
    assert_eq!(j.table()["rows"][0]["blank_fields"], json!([amount]));
    j.round_trip(json!({"type":"edit_scalar","target":{"entity":entity,"field":amount},"input":{"kind":"number","input":"0"}}));
    j.round_trip(json!({"type":"edit_scalar","target":{"entity":entity,"field":note},"input":{"kind":"clear"}}));
    j.round_trip(json!({"type":"paste_cells","collection":j.collection,"start_entity":entity,"start_field":note,"rows":[[{"kind":"text","value":"filled"},{"kind":"clear"}]]}));
    let before = j.table();
    let revision = before["revision"].as_str().unwrap();
    let saved = j
        .runtime
        .as_ref()
        .unwrap()
        .export_project(revision)
        .unwrap()
        .bytes;
    let reopened = open_project(&mut None, &saved, "00000000-0000-4000-8000-000000000552").unwrap();
    assert_eq!(
        serde_json::to_value(reopened.table.rows).unwrap(),
        before["rows"]
    );
    if let Ok(path) = std::env::var("TACHIKO_OPTIONAL_TRANSCRIPT") {
        std::fs::write(
            format!("{path}.capacity"),
            serde_json::to_vec(
                &json!({"project":initial,"occurrence_id":OCCURRENCE,"records":j.transcript}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One import/native capability matrix also supplies native/WASM replay evidence.
fn blank_editability_matches_native_authoring_for_imported_collections() {
    use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
    use tachiko_designer_runtime::{ImportFieldType, ImportSelection, import_workbook};
    let input = |kind: &str| match kind {
        "text" => json!({"kind":"text","value":"changed"}),
        "number" => json!({"kind":"number","input":"2"}),
        "boolean" => json!({"kind":"boolean","value":false}),
        "date" => json!({"kind":"date","value":"2026-10-21"}),
        _ => panic!("unexpected scalar kind"),
    };
    for (kind, field_type, source, native) in [
        ("boolean", ImportFieldType::Boolean, "true", false),
        ("date", ImportFieldType::Date, "2026-10-20", false),
        ("text", ImportFieldType::Text, "anchor", true),
    ] {
        let csv = format!("note,amount,other\n,,{source}\npresent,1,\n");
        let workbook = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        let (runtime, imported) = import_workbook(
            &workbook,
            &ImportSelection {
                column_types: vec![vec![
                    ImportFieldType::Text,
                    ImportFieldType::Number,
                    field_type,
                ]],
                extra_columns: vec![vec![]],
            },
            OCCURRENCE,
        )
        .unwrap();
        let initial = runtime.export_project("resident/0").unwrap().bytes;
        let mut j = Journey {
            runtime: Some(runtime),
            collection: imported.opened.table.collection.key.clone(),
            ..Journey::default()
        };
        let table = j.table();
        assert_eq!(table["native_table_profile"] == true, native);
        let columns = table["columns"].as_array().unwrap();
        assert!(columns.iter().all(|column| column["required"] == false));
        let mut blanks = Vec::new();
        for row in table["rows"].as_array().unwrap() {
            for id in row["blank_fields"].as_array().unwrap() {
                // Sparse row payloads advertise no capability for absent cells.
                assert!(
                    !row["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|field| field["target"]["field"] == *id)
                );
                blanks.push(json!({"entity":row["id"],"field":id}));
            }
        }
        assert_eq!(blanks.len(), 3);
        let query = j.ok(
            json!({"type":"query_fields","expected_revision":table["revision"],"fields":blanks}),
        );
        let fields = query["fields"].as_array().unwrap();
        let explained = j.ok(json!({"type":"explain_field","expected_revision":table["revision"],"target":fields[0]["target"]}));
        assert_eq!(explained["value"], fields[0]);
        for field in fields {
            assert_eq!(field["blank"], true);
            let column = columns
                .iter()
                .find(|column| column["id"] == field["target"]["field"])
                .unwrap();
            let scalar = column["field_type"].as_str().unwrap();
            let expected = if native { json!(scalar) } else { Value::Null };
            assert_eq!(field["editable_scalar"], expected, "{kind}: {field}");
            let set = json!({"type":"edit_scalar","target":field["target"],"input":input(scalar)});
            let clear = json!({"type":"edit_cells","edits":[{"target":field["target"],"input":{"kind":"clear"}}]});
            if native {
                j.round_trip(set);
                j.round_trip(clear);
            } else {
                j.refused(set);
                j.refused(clear);
            }
        }
        // The converse: present scalar capabilities remain usable even in a
        // non-native collection. This repair must not suppress those editors.
        for row in table["rows"].as_array().unwrap() {
            for field in row["fields"].as_array().unwrap() {
                let column = columns
                    .iter()
                    .find(|column| column["id"] == field["target"]["field"])
                    .unwrap();
                let scalar = column["field_type"].as_str().unwrap();
                assert_eq!(field["editable_scalar"], scalar);
                j.round_trip(
                    json!({"type":"edit_scalar","target":field["target"],"input":input(scalar)}),
                );
            }
        }
        if let Ok(path) = std::env::var("TACHIKO_OPTIONAL_TRANSCRIPT") {
            std::fs::write(
                format!("{path}.editability-{kind}"),
                serde_json::to_vec(
                    &json!({"project":initial,"occurrence_id":OCCURRENCE,"records":j.transcript}),
                )
                .unwrap(),
            )
            .unwrap();
        }
    }
}
