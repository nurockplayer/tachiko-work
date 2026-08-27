use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_storage::{load, materialize_roproj, save};
use tachiko_workspace_engine::{
    Document, DocumentId, DocumentOverview, Entity, EntityId, Expression, FieldAddress,
    FieldDefinition, FieldId, FieldKey, FieldKind, FieldRef, FieldType, Number, Schema, SchemaId,
    SchemaKey, Value, explain_field, overview,
};
use uuid::Uuid;

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

type AuthoringField = (String, String, FieldKind);
type AuthoringEntity = (String, String, String, Vec<AuthoringField>);

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
    run_from(arguments, Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn run_from(arguments: &[&str], current_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tachiko"))
        .args(arguments)
        .current_dir(current_dir)
        .output()
        .unwrap()
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).unwrap();
        }
    }
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn balance_document(damage: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                key: SchemaKey::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field("damage")),
                    (
                        FieldId::from("attack_interval"),
                        number_field("attack_interval"),
                    ),
                    (FieldId::from("dps"), number_field("dps")),
                    (
                        FieldId::from("name"),
                        FieldDefinition {
                            id: FieldId::from("name"),
                            key: FieldKey::from("name"),
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
                key: "sword".into(),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number(damage)),
                    (FieldId::from("attack_interval"), number(1.25)),
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

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Number,
        required: true,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn entity_by_key<'document>(document: &'document Document, key: &str) -> &'document Entity {
    document
        .entities
        .values()
        .find(|entity| entity.key.as_str() == key)
        .unwrap_or_else(|| panic!("missing entity key '{key}'"))
}

fn field_value_by_key<'document>(
    document: &'document Document,
    entity_key: &str,
    field_key: &str,
) -> &'document Value {
    let entity = entity_by_key(document, entity_key);
    let schema = &document.schemas[&entity.schema];
    let field_id = schema
        .fields
        .values()
        .find(|field| field.key.as_str() == field_key)
        .unwrap_or_else(|| panic!("missing field key '{field_key}'"))
        .id
        .clone();
    &entity.fields[&field_id]
}

fn projected_formula(document: &Document, entity_key: &str, field_key: &str) -> String {
    explain_field(document, &FieldAddress::new(entity_key, field_key))
        .expect("stored formula should explain through current keys")
        .expression
        .unwrap_or_else(|| panic!("{entity_key}.{field_key} is not a formula"))
}

fn authoring_projection(view: DocumentOverview) -> Vec<AuthoringEntity> {
    view.entities
        .into_iter()
        .map(|entity| {
            (
                entity.key.to_string(),
                entity.label,
                entity.schema.to_string(),
                entity
                    .fields
                    .into_iter()
                    .map(|field| (field.key.to_string(), field.display_value, field.kind))
                    .collect(),
            )
        })
        .collect()
}

fn with_attack_interval(mut document: Document, attack_interval: f64) -> Document {
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(attack_interval));
    document
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
fn init_starter_matches_the_legacy_example_at_the_authoring_boundary() {
    let temp = TempDir::new();
    let path = temp.path().join("starter.ro");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/game-balance/game-balance.ro");

    let output = run(&["init", path.to_str().unwrap(), "--id", "new-game-balance"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let initialized = load(path).expect("initialized starter should load");
    let checked_in = load(example_path).expect("checked-in example should load");
    assert_eq!(
        authoring_projection(overview(&initialized).unwrap()),
        authoring_projection(overview(&checked_in).unwrap())
    );
    assert_ne!(
        initialized.id, checked_in.id,
        "migration and new creation must have distinct identity"
    );
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
fn init_generates_a_uuid_v7_identity_and_derives_a_human_title_from_the_file_name() {
    let temp = TempDir::new();
    let path = temp.path().join("  My Balance Data  .ro");

    let output = run(&["init", path.to_str().unwrap(), "--template", "empty"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = load(path).unwrap();
    assert_eq!(document.title, "My Balance Data");
    let id = Uuid::parse_str(document.id.as_str()).expect("normal creation uses a UUID");
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn init_uses_document_as_the_default_title_when_the_file_stem_is_blank() {
    let temp = TempDir::new();
    let path = temp.path().join("   .ro");

    let output = run(&["init", path.to_str().unwrap(), "--template", "empty"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(load(path).unwrap().title, "document");
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
fn roproj_workflow_operates_outside_git() {
    let temp = TempDir::new();
    assert!(
        temp.path()
            .ancestors()
            .all(|ancestor| !ancestor.join(".git").exists()),
        "test fixture must have no .git ancestor"
    );
    let input = temp.path().join("balance.ro");
    let canonical = temp.path().join("balance.roproj");
    let noncanonical = temp.path().join("noncanonical.roproj");
    let canonicalized = temp.path().join("canonicalized.roproj");
    save(&input, &balance_document(100.0)).unwrap();

    let materialized = run_from(
        &[
            "roproj",
            "materialize",
            input.to_str().unwrap(),
            canonical.to_str().unwrap(),
        ],
        temp.path(),
    );
    assert!(
        materialized.status.success(),
        "{}",
        String::from_utf8_lossy(&materialized.stderr)
    );

    let validated = run_from(
        &["roproj", "validate", canonical.to_str().unwrap()],
        temp.path(),
    );
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    let canonical_snapshot = snapshot_tree(&canonical);

    let repeated = run_from(
        &[
            "roproj",
            "materialize",
            input.to_str().unwrap(),
            canonical.to_str().unwrap(),
        ],
        temp.path(),
    );
    assert!(!repeated.status.success());
    assert_eq!(snapshot_tree(&canonical), canonical_snapshot);

    copy_tree(&canonical, &noncanonical);
    fs::write(noncanonical.join("entities/extra.jsonl"), []).unwrap();
    let noncanonical_snapshot = snapshot_tree(&noncanonical);

    let canonicalized_output = run_from(
        &[
            "roproj",
            "canonicalize",
            noncanonical.to_str().unwrap(),
            canonicalized.to_str().unwrap(),
        ],
        temp.path(),
    );
    assert!(
        canonicalized_output.status.success(),
        "{}",
        String::from_utf8_lossy(&canonicalized_output.stderr)
    );

    let revalidated = run_from(
        &["roproj", "validate", canonicalized.to_str().unwrap()],
        temp.path(),
    );
    assert!(
        revalidated.status.success(),
        "{}",
        String::from_utf8_lossy(&revalidated.stderr)
    );
    assert_eq!(snapshot_tree(&canonicalized), canonical_snapshot);
    assert_eq!(snapshot_tree(&noncanonical), noncanonical_snapshot);
}

#[test]
fn roproj_materialize_rejects_workspace_invalid_document_before_publication() {
    let temp = TempDir::new();
    let input = temp.path().join("invalid.ro");
    let output = temp.path().join("invalid.roproj");
    let mut document = balance_document(100.0);
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));
    save(&input, &document).unwrap();
    let before = snapshot_tree(temp.path());

    let result = run_from(
        &[
            "roproj",
            "materialize",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
        ],
        temp.path(),
    );

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("divided by zero"));
    assert!(!output.exists());
    assert_eq!(snapshot_tree(temp.path()), before);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn roproj_canonicalize_rejects_workspace_invalid_tree_before_publication() {
    let temp = TempDir::new();
    let input = temp.path().join("invalid-input.roproj");
    let output = temp.path().join("invalid-output.roproj");
    let mut document = balance_document(100.0);
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));
    materialize_roproj(&input, &document).unwrap();
    fs::write(input.join("entities/extra.jsonl"), []).unwrap();
    let before = snapshot_tree(temp.path());

    let result = run_from(
        &[
            "roproj",
            "canonicalize",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
        ],
        temp.path(),
    );

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("divided by zero"));
    assert!(!output.exists());
    assert_eq!(snapshot_tree(temp.path()), before);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn roproj_validate_rejects_workspace_invalid_canonical_tree_without_mutation() {
    let temp = TempDir::new();
    let input = temp.path().join("invalid.roproj");
    let mut document = balance_document(100.0);
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));
    materialize_roproj(&input, &document).unwrap();
    let before = snapshot_tree(&input);

    let result = run_from(
        &["roproj", "validate", input.to_str().unwrap()],
        temp.path(),
    );

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("divided by zero"));
    assert_eq!(snapshot_tree(&input), before);
}

#[test]
fn roproj_validate_rejects_bounded_noncanonical_tree_without_mutation() {
    let temp = TempDir::new();
    let input = temp.path().join("noncanonical.roproj");
    materialize_roproj(&input, &balance_document(100.0)).unwrap();
    fs::write(input.join("entities/extra.jsonl"), []).unwrap();
    let before = snapshot_tree(&input);

    let result = run_from(
        &["roproj", "validate", input.to_str().unwrap()],
        temp.path(),
    );

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("representation"));
    assert_eq!(snapshot_tree(&input), before);
}

#[test]
fn roproj_outputs_never_overwrite_existing_files_or_directories() {
    let temp = TempDir::new();
    let direct_input = temp.path().join("input.ro");
    let tree_input = temp.path().join("input.roproj");
    save(&direct_input, &balance_document(100.0)).unwrap();
    materialize_roproj(&tree_input, &balance_document(100.0)).unwrap();
    fs::write(tree_input.join("entities/extra.jsonl"), []).unwrap();

    for (operation, input) in [
        ("materialize", direct_input.as_path()),
        ("canonicalize", tree_input.as_path()),
    ] {
        let existing_file = temp.path().join(format!("{operation}-existing-file"));
        fs::write(&existing_file, "preserve file").unwrap();
        let file_result = run_from(
            &[
                "roproj",
                operation,
                input.to_str().unwrap(),
                existing_file.to_str().unwrap(),
            ],
            temp.path(),
        );
        assert!(!file_result.status.success());
        assert_eq!(fs::read_to_string(existing_file).unwrap(), "preserve file");

        let existing_directory = temp.path().join(format!("{operation}-existing-directory"));
        fs::create_dir(&existing_directory).unwrap();
        fs::write(existing_directory.join("marker"), "preserve directory").unwrap();
        let directory_before = snapshot_tree(&existing_directory);
        let directory_result = run_from(
            &[
                "roproj",
                operation,
                input.to_str().unwrap(),
                existing_directory.to_str().unwrap(),
            ],
            temp.path(),
        );
        assert!(!directory_result.status.success());
        assert_eq!(snapshot_tree(&existing_directory), directory_before);
    }
}

#[test]
fn roproj_help_exposes_explicit_operations() {
    let output = run(&["roproj", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("materialize"));
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("canonicalize"));
    assert!(stdout.contains("never-overwritten"));
}

#[test]
fn direct_ro_validate_remains_a_read_only_direct_ro_command() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    save(&input, &balance_document(100.0)).unwrap();
    let before = fs::read(&input).unwrap();

    let result = run_from(&["validate", input.to_str().unwrap()], temp.path());

    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("valid"));
    assert_eq!(fs::read(&input).unwrap(), before);
    assert_eq!(
        snapshot_tree(temp.path()),
        vec![(PathBuf::from("balance.ro"), before)]
    );
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
        .insert(FieldId::from("attack_interval"), number(0.0));
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
    assert_eq!(exported["format_version"], 2);
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
    assert!(text.contains("document id:"));
    assert!(text.contains("weapons · Iron Sword (iron_sword) ["));
    assert!(text.contains("dps ["));
    assert!(text.contains(": 40 (formula)"));
    assert!(text.contains("weapon ["));
    assert!(text.contains(": → iron_sword (reference → weapons)"));
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
    assert!(formula_text.contains("formula: ([iron_sword.damage] / [iron_sword.attack_interval])"));
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
    let original = load(&input).unwrap();
    let edited = load(&output_path).unwrap();
    assert_eq!(
        field_value_by_key(&original, "iron_sword", "damage"),
        &number(36.0)
    );
    assert_eq!(
        field_value_by_key(&edited, "iron_sword", "damage"),
        &number(45.0)
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
    assert!(String::from_utf8_lossy(&broken_reference.stderr).contains("existing entity key"));

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
    let original = load(&input).unwrap();
    assert_eq!(
        field_value_by_key(&original, "iron_sword", "damage"),
        &number(36.0)
    );
}

#[test]
fn entity_duplicate_creates_a_rebased_copy_without_changing_the_source() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("with-steel-sword.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    let output = run(&[
        "entity",
        "duplicate",
        input.to_str().unwrap(),
        "iron_sword",
        "steel_sword",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(load(&input).unwrap().entities.len(), 4);
    let copied = load(&output_path).unwrap();
    assert_eq!(copied.entities.len(), 5);
    let source = entity_by_key(&copied, "iron_sword");
    let duplicate = entity_by_key(&copied, "steel_sword");
    assert_ne!(duplicate.id, source.id);
    let duplicate_id = Uuid::parse_str(duplicate.id.as_str()).expect("duplicate ID is a UUID");
    assert_eq!(duplicate_id.get_version_num(), 7);
    assert_eq!(
        projected_formula(&copied, "steel_sword", "dps"),
        "([steel_sword.damage] / [steel_sword.attack_interval])"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("steel_sword"));
    assert!(stdout.contains("wrote"));
    assert!(stdout.contains("tachiko show"));
}

#[test]
fn entity_rename_changes_only_the_human_key_and_preserves_bound_relationships() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("renamed.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    let original = load(&input).unwrap();
    let stable_id = entity_by_key(&original, "iron_sword").id.clone();
    let original_formula = field_value_by_key(&original, "shop", "matches_for_sword").clone();

    let output = run(&[
        "entity",
        "rename",
        input.to_str().unwrap(),
        "iron_sword",
        "moonblade",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let renamed = load(&output_path).unwrap();
    assert!(
        renamed
            .entities
            .values()
            .all(|entity| entity.key.as_str() != "iron_sword")
    );
    assert_eq!(entity_by_key(&renamed, "moonblade").id, stable_id);
    assert_eq!(
        field_value_by_key(&renamed, "alric", "weapon"),
        &Value::Reference(stable_id.clone())
    );
    assert_eq!(
        field_value_by_key(&renamed, "tempered_blade", "grants_weapon"),
        &Value::Reference(stable_id)
    );
    assert_eq!(
        field_value_by_key(&renamed, "shop", "matches_for_sword"),
        &original_formula
    );
    assert_eq!(
        projected_formula(&renamed, "shop", "matches_for_sword"),
        "([moonblade.price] / [shop.gold_per_match])"
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("iron_sword -> moonblade")
    );
}

#[test]
fn entity_remove_refuses_live_dependents_and_removes_an_unreferenced_entity() {
    let temp = TempDir::new();
    let starter = temp.path().join("starter.ro");
    let blocked_output = temp.path().join("blocked.ro");
    assert!(run(&["init", starter.to_str().unwrap()]).status.success());

    let blocked = run(&[
        "entity",
        "remove",
        starter.to_str().unwrap(),
        "iron_sword",
        "--output",
        blocked_output.to_str().unwrap(),
    ]);

    assert!(!blocked.status.success());
    let stderr = String::from_utf8(blocked.stderr).unwrap();
    for dependent in [
        "alric.weapon",
        "shop.matches_for_sword",
        "tempered_blade.grants_weapon",
    ] {
        assert!(stderr.contains(dependent), "missing {dependent}: {stderr}");
    }
    assert!(!blocked_output.exists());

    let self_referencing = temp.path().join("self-referencing.ro");
    let removed_output = temp.path().join("removed.ro");
    save(&self_referencing, &balance_document(100.0)).unwrap();
    let removed = run(&[
        "entity",
        "remove",
        self_referencing.to_str().unwrap(),
        "sword",
        "--output",
        removed_output.to_str().unwrap(),
    ]);

    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(load(&removed_output).unwrap().entities.is_empty());
    assert!(
        String::from_utf8(removed.stdout)
            .unwrap()
            .contains("removed sword")
    );
}

#[test]
fn entity_commands_preserve_input_and_existing_output_paths() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let existing = temp.path().join("existing.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());
    fs::write(&existing, "preserve me").unwrap();

    let same_path = run(&[
        "entity",
        "duplicate",
        input.to_str().unwrap(),
        "iron_sword",
        "steel_sword",
        "--output",
        input.to_str().unwrap(),
    ]);
    assert!(!same_path.status.success());
    assert!(String::from_utf8_lossy(&same_path.stderr).contains("same as the input"));
    assert_eq!(load(&input).unwrap().entities.len(), 4);

    let overwrite = run(&[
        "entity",
        "duplicate",
        input.to_str().unwrap(),
        "iron_sword",
        "steel_sword",
        "--output",
        existing.to_str().unwrap(),
    ]);
    assert!(!overwrite.status.success());
    assert_eq!(fs::read_to_string(existing).unwrap(), "preserve me");
}

#[test]
fn formula_set_writes_a_calculated_formula_and_prints_semantic_impact() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let output_path = temp.path().join("capped-dps.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    let output = run(&[
        "formula",
        "set",
        input.to_str().unwrap(),
        "iron_sword.dps",
        "--expression",
        "min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let edited = load(&output_path).unwrap();
    assert!(matches!(
        field_value_by_key(&edited, "iron_sword", "dps"),
        Value::Formula(Expression::Minimum { .. })
    ));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("affected dps: 40 -> 45"));
    assert!(stdout.contains("wrote"));
    assert!(stdout.contains("tachiko explain"));
    let original = load(&input).unwrap();
    assert_eq!(
        projected_formula(&original, "iron_sword", "dps"),
        "([iron_sword.damage] / [iron_sword.attack_interval])"
    );
}

#[test]
fn formula_set_rejects_parse_reference_cycle_and_target_errors_without_output() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    for (name, field, expression, expected) in [
        ("parse", "iron_sword.dps", "min(1,", "byte"),
        (
            "reference",
            "iron_sword.dps",
            "[missing.damage]",
            "cannot be resolved",
        ),
        (
            "cycle",
            "iron_sword.dps",
            "[iron_sword.dps] + 1",
            "dependency cycle",
        ),
        ("target", "iron_sword.name", "1 + 2", "not numeric"),
    ] {
        let output_path = temp.path().join(format!("{name}.ro"));
        let output = run(&[
            "formula",
            "set",
            input.to_str().unwrap(),
            field,
            "--expression",
            expression,
            "--output",
            output_path.to_str().unwrap(),
        ]);

        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(expected),
            "{name} missing '{expected}': {stderr}"
        );
        assert!(!output_path.exists(), "{name} created an output");
    }
}

#[test]
fn formula_set_preserves_same_path_and_existing_output_bytes() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    let existing = temp.path().join("existing.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());
    fs::write(&existing, "preserve me").unwrap();

    let same_path = run(&[
        "formula",
        "set",
        input.to_str().unwrap(),
        "iron_sword.dps",
        "--expression",
        "[iron_sword.damage] + 1",
        "--output",
        input.to_str().unwrap(),
    ]);
    assert!(!same_path.status.success());
    assert!(String::from_utf8_lossy(&same_path.stderr).contains("same as the input"));

    let overwrite = run(&[
        "formula",
        "set",
        input.to_str().unwrap(),
        "iron_sword.dps",
        "--expression",
        "[iron_sword.damage] + 1",
        "--output",
        existing.to_str().unwrap(),
    ]);
    assert!(!overwrite.status.success());
    assert_eq!(fs::read_to_string(existing).unwrap(), "preserve me");
}

#[test]
fn formula_expression_option_transports_canonical_spaced_and_hyphen_values() {
    let temp = TempDir::new();
    let input = temp.path().join("balance.ro");
    assert!(run(&["init", input.to_str().unwrap()]).status.success());

    for (name, expression) in [
        ("negative", "-1"),
        ("negative-reference", "-[iron_sword.damage]"),
        ("multiply", "[iron_sword.damage] * 2"),
    ] {
        let output_path = temp.path().join(format!("{name}.ro"));
        let output = run(&[
            "formula",
            "set",
            input.to_str().unwrap(),
            "iron_sword.dps",
            "--expression",
            expression,
            "--output",
            output_path.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{expression} did not reach the parser: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output_path.exists());
    }

    let explanation = run(&["explain", input.to_str().unwrap(), "iron_sword.dps"]);
    assert!(explanation.status.success());
    let canonical = String::from_utf8(explanation.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("formula: "))
        .expect("explain should print canonical formula syntax")
        .to_owned();
    let no_op_path = temp.path().join("canonical-no-op.ro");
    let no_op = run(&[
        "formula",
        "set",
        input.to_str().unwrap(),
        "iron_sword.dps",
        "--expression",
        &canonical,
        "--output",
        no_op_path.to_str().unwrap(),
    ]);
    assert!(!no_op.status.success());
    assert!(String::from_utf8_lossy(&no_op.stderr).contains("already has that value"));
    assert!(!no_op_path.exists());
}

#[test]
fn semantic_analyst_cli_emits_structured_inspection_formula_and_dependency_results() {
    let temp = TempDir::new();
    let path = temp.path().join("balance.ro");
    save(&path, &balance_document(100.0)).unwrap();

    let inspection = run(&[
        "analyze",
        "document",
        path.to_str().unwrap(),
        "--source-state",
        "main@base",
    ]);
    assert!(inspection.status.success());
    let inspection: serde_json::Value = serde_json::from_slice(&inspection.stdout).unwrap();
    assert_eq!(inspection["source"]["document_id"], "balance");
    assert_eq!(inspection["source"]["source_label"], "main@base");
    assert_eq!(inspection["schemas"][0]["id"], "weapon");
    assert_eq!(inspection["entities"][0]["id"], "sword");

    let formula = run(&[
        "analyze",
        "field",
        path.to_str().unwrap(),
        "sword.dps",
        "--source-state",
        "main@base",
    ]);
    assert!(formula.status.success());
    let formula: serde_json::Value = serde_json::from_slice(&formula.stdout).unwrap();
    assert_eq!(formula["source"]["source_label"], "main@base");
    assert_eq!(formula["calculated_value"], 80.0);
    assert_eq!(
        formula["formula_source"],
        "([sword.damage] / [sword.attack_interval])"
    );
    assert_eq!(
        formula["direct_dependencies"][0]["field"],
        "attack_interval"
    );
    assert_eq!(formula["direct_dependencies"][1]["field"], "damage");

    let input = run(&["analyze", "field", path.to_str().unwrap(), "sword.damage"]);
    assert!(input.status.success());
    let input: serde_json::Value = serde_json::from_slice(&input.stdout).unwrap();
    assert_eq!(input["downstream_impacts"][0]["field"]["field"], "dps");
    assert_eq!(input["downstream_impacts"][0]["value"], 80.0);
}

#[test]
fn semantic_analyst_cli_emits_structured_changes_affected_areas_and_validation_failures() {
    let temp = TempDir::new();
    let before_path = temp.path().join("before.ro");
    let after_path = temp.path().join("after.ro");
    let invalid_path = temp.path().join("invalid.ro");
    let before = balance_document(100.0);
    let after = balance_document(120.0);
    let mut invalid = before.clone();
    invalid
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));
    save(&before_path, &before).unwrap();
    save(&after_path, &after).unwrap();
    save(&invalid_path, &invalid).unwrap();

    let changes = run(&[
        "analyze",
        "changes",
        before_path.to_str().unwrap(),
        after_path.to_str().unwrap(),
        "--before-state",
        "main@base",
        "--after-state",
        "main@buffed",
    ]);
    assert!(changes.status.success());
    let changes: serde_json::Value = serde_json::from_slice(&changes.stdout).unwrap();
    assert_eq!(changes["before"]["source_label"], "main@base");
    assert_eq!(changes["after"]["source_label"], "main@buffed");
    assert!(changes["changes"].as_array().unwrap().iter().any(|change| {
        change["kind"] == "field_changed" && change["field"]["field"] == "damage"
    }));
    assert!(changes["changes"].as_array().unwrap().iter().any(|change| {
        change["kind"] == "formula_impact"
            && change["field"]["field"] == "dps"
            && change["before"] == 80.0
            && change["after"] == 96.0
    }));
    assert_eq!(changes["affected_fields"][0]["field"], "damage");
    assert_eq!(changes["affected_fields"][1]["field"], "dps");
    assert_eq!(changes["affected_entities"][0], "sword");
    assert_eq!(changes["affected_schemas"][0], "weapon");

    let validation = run(&[
        "analyze",
        "validation",
        invalid_path.to_str().unwrap(),
        "--source-state",
        "working-tree",
    ]);
    assert!(validation.status.success());
    let validation: serde_json::Value = serde_json::from_slice(&validation.stdout).unwrap();
    assert_eq!(validation["source"]["source_label"], "working-tree");
    assert_eq!(validation["is_valid"], false);
    assert_eq!(
        validation["diagnostics"][0]["code"],
        "formula.division_by_zero"
    );
}

#[test]
fn semantic_analyst_cli_reports_unknown_targets_explicitly() {
    let temp = TempDir::new();
    let path = temp.path().join("balance.ro");
    save(&path, &balance_document(100.0)).unwrap();

    let output = run(&["analyze", "field", path.to_str().unwrap(), "missing.damage"]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("analysis target 'missing.damage' does not exist")
    );
}

#[test]
fn merge_writes_a_merged_document_and_prints_semantic_impact() {
    let temp = TempDir::new();
    let base_path = temp.path().join("base.ro");
    let ours_path = temp.path().join("ours.ro");
    let theirs_path = temp.path().join("theirs.ro");
    let merged_path = temp.path().join("merged.ro");
    let base = balance_document(100.0);
    let ours = balance_document(120.0);
    let theirs = with_attack_interval(balance_document(100.0), 1.0);
    save(&base_path, &base).unwrap();
    save(&ours_path, &ours).unwrap();
    save(&theirs_path, &theirs).unwrap();

    let output = run(&[
        "merge",
        base_path.to_str().unwrap(),
        ours_path.to_str().unwrap(),
        theirs_path.to_str().unwrap(),
        "--output",
        merged_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        load(&merged_path).unwrap().entities["sword"].fields["damage"],
        number(120.0)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("wrote"));
    assert!(stdout.contains("affected dps"));
}

#[test]
fn merge_title_only_change_prints_the_semantic_impact() {
    let temp = TempDir::new();
    let base_path = temp.path().join("base.ro");
    let ours_path = temp.path().join("ours.ro");
    let theirs_path = temp.path().join("theirs.ro");
    let merged_path = temp.path().join("merged.ro");
    let base = balance_document(100.0);
    let mut ours = base.clone();
    ours.title = "Rebalanced".to_owned();
    save(&base_path, &base).unwrap();
    save(&ours_path, &ours).unwrap();
    save(&theirs_path, &base).unwrap();

    let output = run(&[
        "merge",
        base_path.to_str().unwrap(),
        ours_path.to_str().unwrap(),
        theirs_path.to_str().unwrap(),
        "--output",
        merged_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(load(&merged_path).unwrap().title, "Rebalanced");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("title: \"Balance\" -> \"Rebalanced\""));
    assert!(!stdout.contains("No semantic changes."));
}

#[test]
fn merge_schema_definition_only_change_prints_the_semantic_impact() {
    let temp = TempDir::new();
    let base_path = temp.path().join("base.ro");
    let ours_path = temp.path().join("ours.ro");
    let theirs_path = temp.path().join("theirs.ro");
    let merged_path = temp.path().join("merged.ro");
    let base = balance_document(100.0);
    let mut ours = base.clone();
    ours.schemas.get_mut("weapon").unwrap().fields.insert(
        FieldId::from("weight"),
        FieldDefinition {
            id: FieldId::from("weight"),
            key: FieldKey::from("weight"),
            field_type: FieldType::Number,
            required: false,
        },
    );
    save(&base_path, &base).unwrap();
    save(&ours_path, &ours).unwrap();
    save(&theirs_path, &base).unwrap();

    let output = run(&[
        "merge",
        base_path.to_str().unwrap(),
        ours_path.to_str().unwrap(),
        theirs_path.to_str().unwrap(),
        "--output",
        merged_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        load(&merged_path).unwrap().schemas["weapon"]
            .fields
            .contains_key("weight")
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Schema weapon"));
    assert!(stdout.contains("weight added: number (optional)"));
    assert!(!stdout.contains("No semantic changes."));
}

#[test]
fn merge_reports_typed_conflicts_without_creating_output() {
    let temp = TempDir::new();
    let base_path = temp.path().join("base.ro");
    let ours_path = temp.path().join("ours.ro");
    let theirs_path = temp.path().join("theirs.ro");
    let merged_path = temp.path().join("merged.ro");
    save(&base_path, &balance_document(100.0)).unwrap();
    save(&ours_path, &balance_document(120.0)).unwrap();
    save(&theirs_path, &balance_document(140.0)).unwrap();

    let output = run(&[
        "merge",
        base_path.to_str().unwrap(),
        ours_path.to_str().unwrap(),
        theirs_path.to_str().unwrap(),
        "--output",
        merged_path.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("entities.sword.fields.damage"));
    assert!(stderr.contains("FieldValue(Number(Number(100.0)))"));
    assert!(stderr.contains("FieldValue(Number(Number(120.0)))"));
    assert!(stderr.contains("FieldValue(Number(Number(140.0)))"));
    assert!(!merged_path.exists());
}

#[test]
fn merge_preserves_existing_output_bytes() {
    let temp = TempDir::new();
    let base_path = temp.path().join("base.ro");
    let ours_path = temp.path().join("ours.ro");
    let theirs_path = temp.path().join("theirs.ro");
    let merged_path = temp.path().join("merged.ro");
    save(&base_path, &balance_document(100.0)).unwrap();
    save(&ours_path, &balance_document(120.0)).unwrap();
    save(
        &theirs_path,
        &with_attack_interval(balance_document(100.0), 1.0),
    )
    .unwrap();
    let original = b"do not overwrite";
    fs::write(&merged_path, original).unwrap();

    let output = run(&[
        "merge",
        base_path.to_str().unwrap(),
        ours_path.to_str().unwrap(),
        theirs_path.to_str().unwrap(),
        "--output",
        merged_path.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(fs::read(&merged_path).unwrap(), original);
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
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
        "Grow, rename, or remove entities safely",
        "Create or revise computed fields safely",
        "Compare two document versions",
    ] {
        assert!(text.contains(phrase), "missing help text: {phrase}\n{text}");
    }
}

#[test]
fn entity_help_makes_lifecycle_operations_discoverable() {
    let output = run(&["entity", "--help"]);

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for phrase in [
        "Duplicate an entity",
        "Rename an entity",
        "Remove an unreferenced entity",
    ] {
        assert!(text.contains(phrase), "missing help text: {phrase}\n{text}");
    }
}

#[test]
fn formula_help_makes_computational_authoring_discoverable() {
    let output = run(&["formula", "--help"]);

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Set a numeric field formula"));
}
