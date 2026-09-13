use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::Value as JsonValue;
use tachiko_storage::save;
use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value,
};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-cli-large-stdout-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn large_document() -> Document {
    let schema_id = SchemaId::from("record-schema");
    let field_id = FieldId::from("payload-field");
    let schema = Schema {
        id: schema_id.clone(),
        key: SchemaKey::from("record"),
        fields: BTreeMap::from([(
            field_id.clone(),
            FieldDefinition {
                id: field_id.clone(),
                key: FieldKey::from("payload"),
                field_type: FieldType::Text,
                required: true,
            },
        )]),
    };

    let payload = "x".repeat(320);
    let entities = (0..2_400)
        .map(|index| {
            let id = EntityId::from(format!("record-{index:04}"));
            (
                id.clone(),
                Entity {
                    id,
                    key: EntityKey::from(format!("record_{index:04}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([(
                        field_id.clone(),
                        Value::Text(format!("{index:04}-{payload}")),
                    )]),
                },
            )
        })
        .collect();

    Document {
        id: DocumentId::from("large-stdout-delivery"),
        title: "Large stdout delivery".to_owned(),
        schemas: BTreeMap::from([(schema_id, schema)]),
        entities,
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn analyze_arguments(path: &Path) -> Vec<String> {
    vec![
        "analyze".to_owned(),
        "document".to_owned(),
        path.to_string_lossy().into_owned(),
        "--source-state".to_owned(),
        "large-stdout-delivery".to_owned(),
    ]
}

#[test]
fn large_structured_stdout_is_delivered_whole_through_a_pipe() {
    let temp = TempDir::new();
    let source = temp.path().join("large.ro");
    save(&source, &large_document()).unwrap();
    let arguments = analyze_arguments(&source);

    let reference_path = temp.path().join("reference.json");
    let reference_file = File::create(&reference_path).unwrap();
    let reference_status = Command::new(env!("CARGO_BIN_EXE_tachiko"))
        .args(&arguments)
        .stdout(Stdio::from(reference_file))
        .stderr(Stdio::piped())
        .status()
        .unwrap();
    assert!(reference_status.success());

    let reference = fs::read(&reference_path).unwrap();
    assert!(
        reference.len() >= 512 * 1024,
        "fixture must exceed a normal pipe buffer; got {} bytes",
        reference.len()
    );
    let _: JsonValue = serde_json::from_slice(&reference).expect("reference output must be JSON");

    for attempt in 0..3 {
        let output = Command::new(env!("CARGO_BIN_EXE_tachiko"))
            .args(&arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "piped attempt {attempt} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _: JsonValue = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("piped attempt {attempt} returned invalid JSON: {error}"));
        assert_eq!(
            output.stdout, reference,
            "piped attempt {attempt} differed from complete redirected stdout"
        );
    }
}
