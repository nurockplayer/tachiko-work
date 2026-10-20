//! ADR-0045 persistence/history via the host boundary. Captures can also be
//! replayed through the compiled WASM ABI by `optional_native_persistence.mjs`.
#![cfg(not(target_arch = "wasm32"))]

use serde_json::{Value, json};
use tachiko_designer_runtime::host_bridge::{DesignerHost, HostReply};

const ORIGINAL: &str = "00000000-0000-4000-8000-000000000554";
const FRESH: &str = "00000000-0000-4000-8000-000000000555";

#[derive(Default)]
struct Journey {
    host: DesignerHost,
    records: Vec<Value>,
}

impl Journey {
    fn record(
        &mut self,
        operation: &str,
        request: &[u8],
        project: &[u8],
        reply: &HostReply,
    ) -> Value {
        let parsed: Value = serde_json::from_slice(&reply.reply).unwrap();
        self.records.push(
            json!({"operation":operation,"request":request,"project":project,
            "reply":parsed,"bytes":reply.bytes}),
        );
        parsed
    }

    fn request(&mut self, request: &Value) -> Value {
        let bytes = serde_json::to_vec(request).unwrap();
        let reply = HostReply {
            reply: self.host.request(&bytes),
            bytes: None,
        };
        self.record("request_run", &bytes, &[], &reply)
    }

    fn ok(&mut self, request: &Value) -> Value {
        let reply = self.request(request);
        assert_eq!(reply["status"], "ok", "{reply}");
        reply["response"]["payload"].clone()
    }

    fn table(&mut self) -> Value {
        self.ok(&json!({"type":"query_table","collection":"items"}))
    }

    fn mutate(&mut self, mut request: Value) {
        request["expected_revision"] = self.table()["revision"].clone();
        self.ok(&request);
    }

    fn save(&mut self, portable: bool) -> Vec<u8> {
        let revision = self.table()["revision"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec();
        let (operation, reply) = if portable {
            (
                "portable_ro_export",
                self.host.export_portable_ro(&revision),
            )
        } else {
            ("project_export_v3", self.host.export_project_v3(&revision))
        };
        let bytes = reply.bytes.clone().unwrap();
        let parsed = self.record(operation, &revision, &[], &reply);
        assert_eq!(parsed["status"], "ok", "{parsed}");
        bytes
    }

    fn reopen(&mut self, bytes: &[u8], portable: bool) {
        let (operation, reply) = if portable {
            (
                "portable_ro_open",
                self.host.open_portable_ro(bytes, FRESH.as_bytes()),
            )
        } else {
            (
                "project_open",
                self.host.open_project(bytes, FRESH.as_bytes()),
            )
        };
        let reply = self.record(
            operation,
            FRESH.as_bytes(),
            bytes,
            &HostReply { reply, bytes: None },
        );
        assert_eq!(reply["status"], "ok", "{reply}");
    }

    fn export(&mut self, format: &str) {
        let table = self.table();
        let request = serde_json::to_vec(&json!({"type":"export_native_tables",
            "expected_revision":table["revision"],"collection":"items","format":format}))
        .unwrap();
        let reply = self.host.spreadsheet(&request, &[]);
        let parsed = self.record("spreadsheet_run", &request, &[], &reply);
        assert_eq!(parsed["status"], "ok", "{parsed}");
    }

    fn capture(&self, name: &str) {
        if let Ok(root) = std::env::var("TACHIKO_OPTIONAL_PERSISTENCE_TRANSCRIPT") {
            std::fs::write(
                format!("{root}.{name}.json"),
                serde_json::to_vec(&self.records).unwrap(),
            )
            .unwrap();
        }
    }
}

fn fixture(sole: bool) -> Journey {
    let mut j = Journey::default();
    let mut columns = vec![json!({"name":"note","field_type":"text","required":false})];
    if !sole {
        columns.extend([
            json!({"name":"amount","field_type":"number","required":false}),
            json!({"name":"flag","field_type":"boolean"}),
        ]);
    }
    j.ok(&json!({"type":"new_table","occurrence_id":ORIGINAL,"name":"items","columns":columns}));
    let table = j.table();
    for row in 0..3 {
        let mut initializers = Vec::new();
        for column in table["columns"].as_array().unwrap() {
            let input = match column["key"].as_str().unwrap() {
                "note" if row == 1 => Some(json!({"kind":"text","value":""})),
                "note" if row == 2 => Some(json!({"kind":"text","value":"present"})),
                "amount" if row > 0 => {
                    Some(json!({"kind":"number","input":if row == 1 {"0"} else {"7"}}))
                }
                "flag" => Some(json!({"kind":"boolean","value":false})),
                _ => None,
            };
            if let Some(input) = input {
                initializers.push(json!({"field":column["id"],"input":input}));
            }
        }
        j.mutate(json!({"type":"insert_row","collection":"items","initializers":initializers}));
    }
    j
}

fn note_target(table: &Value) -> Value {
    let column = table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|col| col["key"] == "note")
        .unwrap();
    json!({"entity":table["rows"][0]["id"],"field":column["id"]})
}

fn assert_fresh_edit_history(j: &mut Journey, before: &Value, portable: bool) {
    let opened = j.table();
    assert_eq!(opened["revision"], "resident/0");
    assert_eq!(opened["columns"], before["columns"]);
    assert_eq!(opened["rows"], before["rows"]);
    assert_eq!(opened["native_table_profile"], true);
    for direction in ["undo", "redo"] {
        let reply = j.request(&json!({"type":direction,"expected_revision":"resident/0"}));
        assert_eq!(reply["status"], "error", "{reply}");
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no operation is available in this history direction"),
            "{reply}"
        );
    }
    let saved = j.save(portable);
    j.mutate(json!({"type":"edit_scalar","target":note_target(&opened),"input":{"kind":"text","value":"edited after reopen"}}));
    let edited = j.save(portable);
    assert_ne!(edited, saved);
    j.mutate(json!({"type":"undo"}));
    assert_eq!(
        j.save(portable),
        saved,
        "Undo restores exact absence/empty Text and requiredness"
    );
    assert_eq!(j.table()["rows"], before["rows"]);
    j.mutate(json!({"type":"redo"}));
    assert_eq!(j.save(portable), edited);
    j.mutate(json!({"type":"undo"}));
    j.export("csv");
    j.export("xlsx");
}

#[test]
fn canonical_and_portable_reopen_preserve_sparse_rows_requiredness_and_fresh_history() {
    for sole in [false, true] {
        for portable in [false, true] {
            let mut j = fixture(sole);
            let initial = j.table();
            assert_eq!(
                initial["rows"][0]["fields"].as_array().unwrap().len(),
                usize::from(!sole)
            );
            assert_eq!(
                initial["rows"][0]["blank_fields"].as_array().unwrap().len(),
                if sole { 1 } else { 2 }
            );
            for stage in ["sparse", "required", "relaxed"] {
                let before = j.table();
                let bytes = j.save(portable);
                // Saving itself retains the live history and does not advance a revision.
                assert_eq!(j.table(), before);
                if stage == "sparse" {
                    j.mutate(json!({"type":"undo"}));
                    assert_eq!(j.table()["rows"].as_array().unwrap().len(), 2);
                    j.mutate(json!({"type":"redo"}));
                    assert_eq!(j.save(portable), bytes);
                }
                j.reopen(&bytes, portable);
                assert_eq!(j.save(portable), bytes);
                assert_fresh_edit_history(&mut j, &before, portable);
                let target = note_target(&j.table());
                if stage == "sparse" {
                    j.mutate(json!({"type":"edit_scalar","target":target,"input":{"kind":"text","value":"filled"}}));
                    j.mutate(json!({"type":"set_column_requiredness","collection":"items","field":target["field"],"required":true}));
                } else if stage == "required" {
                    j.mutate(json!({"type":"set_column_requiredness","collection":"items","field":target["field"],"required":false}));
                    j.mutate(
                        json!({"type":"edit_scalar","target":target,"input":{"kind":"clear"}}),
                    );
                }
            }
            j.capture(&format!("sole-{sole}-portable-{portable}"));
        }
    }
}
