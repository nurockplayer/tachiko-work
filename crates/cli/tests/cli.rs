use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};
use tachiko_storage::{load, save};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("tachiko-cli-{}-{sequence}", std::process::id()));
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

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tachiko"))
        .args(arguments)
        .output()
        .unwrap()
}

fn balance_document(damage: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field()),
                    (FieldId::from("attack_interval"), number_field()),
                    (FieldId::from("dps"), number_field()),
                    (
                        FieldId::from("name"),
                        FieldDefinition {
                            field_type: FieldType::Text,
                            required: true,
                        },
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("sword"),
            Entity {
                id: EntityId::from("sword"),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), Value::Number(damage)),
                    (FieldId::from("attack_interval"), Value::Number(1.25)),
                    (
                        FieldId::from("dps"),
                        Value::Formula(Expression::Divide {
                            left: Box::new(Expression::Reference(FieldRef::new("sword", "damage"))),
                            right: Box::new(Expression::Reference(FieldRef::new(
                                "sword",
                                "attack_interval",
                            ))),
                        }),
                    ),
                    (FieldId::from("name"), Value::Text("Sword".to_owned())),
                ]),
            },
        )]),
    }
}

fn number_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required: true,
    }
}

#[test]
fn init_creates_a_valid_semantic_document() {
    let temp = TempDir::new();
    let path = temp.path().join("new.ro");

    let output = run(&[
        "init",
        path.to_str().unwrap(),
        "--id",
        "new-balance",
        "--title",
        "New Balance",
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = load(&path).unwrap();
    assert_eq!(document.id, DocumentId::from("new-balance"));
    assert_eq!(document.title, "New Balance");
    assert_eq!(document.schemas.len(), 4);
    assert_eq!(document.entities.len(), 4);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ready to explore"));
    assert!(stdout.contains("tachiko show"));
}

#[test]
fn init_can_explicitly_create_an_empty_document() {
    let temp = TempDir::new();
    let path = temp.path().join("scratch.ro");

    let output = run(&["init", path.to_str().unwrap(), "--template", "empty"]);

    assert!(output.status.success());
    let document = load(path).unwrap();
    assert!(document.schemas.is_empty());
    assert!(document.entities.is_empty());
}

#[test]
fn init_derives_a_valid_identifier_from_a_human_file_name() {
    let temp = TempDir::new();
    let path = temp.path().join("My Balance Data.ro");

    let output = run(&["init", path.to_str().unwrap(), "--template", "empty"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(load(path).unwrap().id, DocumentId::from("my-balance-data"));
}

#[test]
fn init_refuses_to_overwrite_an_existing_document() {
    let temp = TempDir::new();
    let path = temp.path().join("existing.ro");
    fs::write(&path, "preserve me").unwrap();

    let output = run(&["init", path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(path).unwrap(), "preserve me");
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
}

#[test]
fn validate_is_ci_safe_for_valid_and_invalid_documents() {
    let temp = TempDir::new();
    let valid_path = temp.path().join("valid.ro");
    let invalid_path = temp.path().join("invalid.ro");
    save(&valid_path, &balance_document(100.0)).unwrap();
    fs::write(&invalid_path, "{\"format_version\":1}").unwrap();

    let valid = run(&["validate", valid_path.to_str().unwrap()]);
    let invalid = run(&["validate", invalid_path.to_str().unwrap()]);

    assert!(valid.status.success());
    assert!(String::from_utf8_lossy(&valid.stdout).contains("valid"));
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("error:"));
}

#[test]
fn validate_rejects_formula_division_by_zero() {
    let temp = TempDir::new();
    let path = temp.path().join("division-by-zero.ro");
    let mut document = balance_document(100.0);
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(0.0));
    save(&path, &document).unwrap();

    let output = run(&["validate", path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("divided by zero"));
}

#[test]
fn validate_rejects_formula_cycles() {
    let temp = TempDir::new();
    let path = temp.path().join("cycle.ro");
    let mut document = balance_document(100.0);
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Reference(FieldRef::new("sword", "dps"))),
    );
    save(&path, &document).unwrap();

    let output = run(&["validate", path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("dependency cycle"));
}

#[test]
fn calculate_emits_sorted_machine_readable_results() {
    let temp = TempDir::new();
    let path = temp.path().join("balance.ro");
    save(&path, &balance_document(100.0)).unwrap();

    let output = run(&["calculate", path.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(values["sword.attack_interval"], 1.25);
    assert_eq!(values["sword.damage"], 100.0);
    assert_eq!(values["sword.dps"], 80.0);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.find("attack_interval").unwrap() < text.find("damage").unwrap());
}

#[test]
fn diff_explains_direct_and_formula_impact() {
    let temp = TempDir::new();
    let before = temp.path().join("before.ro");
    let after = temp.path().join("after.ro");
    save(&before, &balance_document(100.0)).unwrap();
    save(&after, &balance_document(120.0)).unwrap();

    let output = run(&["diff", before.to_str().unwrap(), after.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Weapon Sword"));
    assert!(text.contains("damage: 100 -> 120"));
    assert!(text.contains("affected dps: 80 -> 96"));
}

#[test]
fn export_materializes_formula_results_without_losing_semantic_identity() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("balance.json");
    save(&input, &balance_document(100.0)).unwrap();

    let output = run(&[
        "export",
        input.to_str().unwrap(),
        output_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output_path).unwrap()).unwrap();
    assert_eq!(exported["format_version"], 1);
    assert_eq!(exported["document_id"], "balance");
    assert_eq!(exported["entities"]["sword"]["schema"], "weapon");
    assert_eq!(exported["entities"]["sword"]["fields"]["dps"], 80.0);
}

#[test]
fn export_refuses_to_overwrite_an_existing_file() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("existing.json");
    save(&input, &balance_document(100.0)).unwrap();
    fs::write(&output_path, "preserve me").unwrap();

    let output = run(&[
        "export",
        input.to_str().unwrap(),
        output_path.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(output_path).unwrap(), "preserve me");
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
}

#[test]
fn show_turns_a_document_into_a_readable_semantic_overview() {
    let temp = TempDir::new();
    let path = temp.path().join("balance.ro");
    let initialized = run(&["init", path.to_str().unwrap()]);
    assert!(initialized.status.success());

    let output = run(&["show", path.to_str().unwrap()]);

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("balance · 4 schemas · 4 entities · 3 formulas"));
    assert!(text.contains("weapons · Iron Sword [iron_sword]"));
    assert!(text.contains("dps: 40 (formula)"));
    assert!(text.contains("weapon: → iron_sword (reference → weapons)"));
}

#[test]
fn explain_makes_formula_and_impact_relationships_discoverable() {
    let temp = TempDir::new();
    let path = temp.path().join("balance.ro");
    assert!(run(&["init", path.to_str().unwrap()]).status.success());

    let formula = run(&["explain", path.to_str().unwrap(), "iron_sword.dps"]);
    assert!(formula.status.success());
    let formula_text = String::from_utf8(formula.stdout).unwrap();
    assert!(formula_text.contains("iron_sword.dps = 40"));
    assert!(formula_text.contains("formula: (iron_sword.damage / iron_sword.attack_interval)"));
    assert!(formula_text.contains("depends on:"));
    assert!(formula_text.contains("iron_sword.damage"));

    let input = run(&["explain", path.to_str().unwrap(), "iron_sword.damage"]);
    assert!(input.status.success());
    let input_text = String::from_utf8(input.stdout).unwrap();
    assert!(input_text.contains("iron_sword.damage = 36"));
    assert!(input_text.contains("affects:"));
    assert!(input_text.contains("iron_sword.dps = 40"));
}

#[test]
fn set_writes_a_new_valid_document_and_prints_semantic_impact() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("buffed.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    let output = run(&[
        "set",
        input.to_str().unwrap(),
        "iron_sword.damage",
        "45",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("damage: 36 -> 45"));
    assert!(text.contains("affected dps: 40 -> 50"));
    assert!(text.contains("wrote"));
    assert_eq!(
        load(&input).unwrap().entities["iron_sword"].fields["damage"],
        Value::Number(36.0)
    );
    assert_eq!(
        load(&output_path).unwrap().entities["iron_sword"].fields["damage"],
        Value::Number(45.0)
    );
}

#[test]
fn set_refuses_invalid_field_syntax_formula_edits_and_existing_outputs() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let existing = temp.path().join("existing.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());
    fs::write(&existing, "preserve me").unwrap();

    let invalid_ref = run(&[
        "set",
        input.to_str().unwrap(),
        "damage",
        "45",
        "--output",
        temp.path().join("unused.ro").to_str().unwrap(),
    ]);
    assert!(!invalid_ref.status.success());
    assert!(String::from_utf8_lossy(&invalid_ref.stderr).contains("entity.field"));

    let formula = run(&[
        "set",
        input.to_str().unwrap(),
        "iron_sword.dps",
        "50",
        "--output",
        temp.path().join("formula.ro").to_str().unwrap(),
    ]);
    assert!(!formula.status.success());
    assert!(String::from_utf8_lossy(&formula.stderr).contains("edit its inputs"));

    let broken_reference = run(&[
        "set",
        input.to_str().unwrap(),
        "alric.weapon",
        "missing_weapon",
        "--output",
        temp.path().join("broken-reference.ro").to_str().unwrap(),
    ]);
    assert!(!broken_reference.status.success());
    assert!(
        String::from_utf8_lossy(&broken_reference.stderr)
            .contains("referenced entity 'missing_weapon' does not exist")
    );

    let overwrite = run(&[
        "set",
        input.to_str().unwrap(),
        "iron_sword.damage",
        "45",
        "--output",
        existing.to_str().unwrap(),
    ]);
    assert!(!overwrite.status.success());
    assert_eq!(fs::read_to_string(existing).unwrap(), "preserve me");

    let same_path = run(&[
        "set",
        input.to_str().unwrap(),
        "iron_sword.damage",
        "45",
        "--output",
        input.to_str().unwrap(),
    ]);
    assert!(!same_path.status.success());
    assert!(String::from_utf8_lossy(&same_path.stderr).contains("same as the input"));
    assert_eq!(
        load(&input).unwrap().entities["iron_sword"].fields["damage"],
        Value::Number(36.0)
    );
}

#[test]
fn top_level_help_describes_the_complete_first_user_workflow() {
    let output = run(&["--help"]);

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for phrase in [
        "Create a semantic document",
        "Browse entities and calculated values",
        "Explain a field",
        "Create a changed document",
        "Compare two document versions",
    ] {
        assert!(text.contains(phrase), "missing help text: {phrase}\n{text}");
    }
}
