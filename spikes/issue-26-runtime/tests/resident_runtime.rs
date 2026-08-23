use tachiko_issue_26_runtime_spike::{
    Command, CommandResult, ResidentRuntime, execute_snapshot, synthetic_document,
};
use tachiko_workspace_engine::{Document, FieldAddress, calculate_fields};

#[test]
fn resident_runtime_keeps_the_document_in_rust_and_returns_projection_results() {
    let initial = synthetic_document(2).expect("synthetic fixture should be valid");
    let snapshot = serde_json::to_vec(&initial).expect("fixture should serialize");
    let mut runtime = ResidentRuntime::open(&snapshot).expect("snapshot should open");

    let overview = runtime
        .execute(Command::Overview)
        .expect("query should succeed");
    assert_eq!(overview.revision, 0);
    assert_eq!(
        overview.result,
        CommandResult::Overview {
            schema_count: 1,
            entity_count: 2,
            formula_count: 2,
        }
    );

    let mutation = runtime
        .execute(Command::SetScalar {
            address: FieldAddress::new("entity_0000", "base"),
            input: "11".to_owned(),
        })
        .expect("mutation should succeed");
    assert_eq!(mutation.revision, 1);
    let CommandResult::Mutation {
        change_count,
        diff_text,
        calculated,
    } = mutation.result
    else {
        panic!("expected mutation result");
    };
    assert_eq!(change_count, 2);
    assert!(diff_text.contains("base"));
    assert!(diff_text.contains("computed"));
    assert_eq!(calculated.len(), 2);
    assert!(
        calculated
            .iter()
            .all(|field| field.address.starts_with("entity_0000."))
    );
    assert_eq!(
        calculated
            .iter()
            .find(|field| field.address == "entity_0000.computed")
            .map(|field| field.value),
        Some(22.0)
    );

    let current_snapshot = runtime.snapshot().expect("resident state should serialize");
    let current: Document =
        serde_json::from_slice(&current_snapshot).expect("resident snapshot should decode");
    let calculated = calculate_fields(&current).expect("resident snapshot should remain valid");
    assert_eq!(calculated.len(), 6);
}

#[test]
fn snapshot_roundtrip_and_resident_execution_have_the_same_semantic_result() {
    let initial = synthetic_document(3).expect("synthetic fixture should be valid");
    let snapshot = serde_json::to_vec(&initial).expect("fixture should serialize");
    let command = Command::SetScalar {
        address: FieldAddress::new("entity_0001", "multiplier"),
        input: "4".to_owned(),
    };

    let mut resident = ResidentRuntime::open(&snapshot).expect("snapshot should open");
    let resident_response = resident
        .execute(command.clone())
        .expect("resident mutation should succeed");
    let resident_snapshot = resident
        .snapshot()
        .expect("resident state should serialize");

    let roundtrip = execute_snapshot(&snapshot, command).expect("snapshot mutation should succeed");

    assert_eq!(roundtrip.response, resident_response);
    assert_eq!(roundtrip.snapshot, resident_snapshot);
}

#[test]
fn merge_uses_explicit_branch_snapshots_without_moving_resident_ours_state() {
    let base = synthetic_document(2).expect("synthetic fixture should be valid");
    let base_snapshot = serde_json::to_vec(&base).expect("fixture should serialize");
    let ours = execute_snapshot(
        &base_snapshot,
        Command::SetScalar {
            address: FieldAddress::new("entity_0000", "base"),
            input: "10".to_owned(),
        },
    )
    .expect("ours branch should succeed");
    let theirs = execute_snapshot(
        &base_snapshot,
        Command::SetScalar {
            address: FieldAddress::new("entity_0001", "multiplier"),
            input: "3".to_owned(),
        },
    )
    .expect("theirs branch should succeed");
    let theirs_document = serde_json::from_slice(&theirs.snapshot).expect("theirs should decode");
    let mut runtime = ResidentRuntime::open(&ours.snapshot).expect("ours should become resident");

    let merged = runtime
        .execute(Command::Merge {
            base,
            theirs: theirs_document,
        })
        .expect("independent branches should merge");

    assert_eq!(merged.revision, 1);
    let CommandResult::Merge {
        merged,
        conflict_count,
        change_count,
        calculated,
        ..
    } = merged.result
    else {
        panic!("expected merge result");
    };
    assert!(merged);
    assert_eq!(conflict_count, 0);
    assert_eq!(change_count, 4);
    assert_eq!(calculated.len(), 4);
    assert_eq!(
        calculated
            .iter()
            .find(|field| field.address == "entity_0000.computed")
            .map(|field| field.value),
        Some(20.0)
    );
    assert_eq!(
        calculated
            .iter()
            .find(|field| field.address == "entity_0001.computed")
            .map(|field| field.value),
        Some(6.0)
    );
}
