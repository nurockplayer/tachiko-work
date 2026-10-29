use super::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, NewTableColumnInput,
    PatchLifecycleError, open_project,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000604";

fn fixture() -> (DesignerRuntime, String) {
    let columns = ["Label", "Amount", "Note"].map(|name| NewTableColumnInput {
        name: name.into(),
        field_type: "text".into(),
        required: false,
    });
    let runtime = DesignerRuntime::new_table(OCCURRENCE, "Order", &columns).unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    (runtime, collection)
}

fn column_ids(runtime: &DesignerRuntime, collection: &str) -> Vec<String> {
    runtime
        .query_table(collection)
        .unwrap()
        .columns
        .into_iter()
        .map(|column| column.id)
        .collect()
}

fn move_column(
    runtime: &mut DesignerRuntime,
    collection: &str,
    field: &str,
    to_index: u32,
) -> Result<DesignerResponse, DesignerError> {
    runtime.handle(DesignerRequest::MoveColumn {
        expected_revision: runtime.current_revision().into(),
        collection: collection.into(),
        field: field.into(),
        to_index,
    })
}

fn undo(runtime: &mut DesignerRuntime) {
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
}

fn redo(runtime: &mut DesignerRuntime) {
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::Redo { expected_revision })
        .unwrap();
}

#[test]
fn move_column_moves_projection_and_undo_redo_restores_it() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    assert_eq!(initial.len(), 3);
    let moved = initial[0].clone();
    let moved_order = vec![initial[1].clone(), initial[2].clone(), moved.clone()];
    assert!(matches!(
        move_column(&mut runtime, &collection, &moved, 2).unwrap(),
        DesignerResponse::Published(_)
    ));
    assert_eq!(column_ids(&runtime, &collection), moved_order);
    undo(&mut runtime);
    assert_eq!(column_ids(&runtime, &collection), initial);
    redo(&mut runtime);
    assert_eq!(column_ids(&runtime, &collection), moved_order);
}

#[test]
fn out_of_range_index_is_clamped_to_the_last_column() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[0], 99).unwrap();
    assert_eq!(
        column_ids(&runtime, &collection),
        vec![initial[1].clone(), initial[2].clone(), initial[0].clone()]
    );
}

#[test]
fn move_to_the_current_position_publishes_nothing() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    let revision = runtime.current_revision().to_owned();
    assert!(move_column(&mut runtime, &collection, &initial[1], 1).is_err());
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(column_ids(&runtime, &collection), initial);
}

#[test]
fn added_column_lands_last_and_removed_column_leaves_the_order_consistent() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[0], 2).unwrap();
    let moved_order = column_ids(&runtime, &collection);
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision,
            collection: collection.clone(),
            name: "Added".into(),
            field_type: "text".into(),
            required: false,
            initializers: Vec::new(),
        })
        .unwrap();
    let added = column_ids(&runtime, &collection);
    assert_eq!(added.len(), 4);
    assert_eq!(added[..3], moved_order[..]);
    let expected_revision = runtime.current_revision().into();
    runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision,
            collection: collection.clone(),
            field: moved_order[1].clone(),
        })
        .unwrap();
    assert_eq!(
        column_ids(&runtime, &collection),
        vec![
            moved_order[0].clone(),
            moved_order[2].clone(),
            added[3].clone()
        ]
    );
}

#[test]
fn field_order_survives_export_and_reopen() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[0], 2).unwrap();
    let expected = column_ids(&runtime, &collection);
    let revision = runtime.current_revision().to_owned();
    let export = runtime.export_project_v3(&revision).unwrap();
    let mut reopened = None;
    open_project(&mut reopened, &export.bytes, OCCURRENCE).unwrap();
    let reopened = reopened.unwrap();
    assert_eq!(column_ids(&reopened, &collection), expected);
}

#[test]
fn csv_export_follows_the_field_order() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[0], 2).unwrap();
    let revision = runtime.current_revision().to_owned();
    let workbook = runtime
        .export_native_tables_csv_workbook(&revision, &collection)
        .unwrap();
    let headers = workbook.sheets[0]
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(headers, ["Amount", "Note", "Label"]);
}

#[test]
fn tracker_moves_use_the_projected_positions() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    assert_eq!(
        column_ids(&runtime, "tracker"),
        ["task", "estimate", "done"]
    );
    assert!(matches!(
        move_column(&mut runtime, "tracker", "task", 0),
        Err(DesignerError::Lifecycle(PatchLifecycleError::NoChange))
    ));
    move_column(&mut runtime, "tracker", "done", 0).unwrap();
    assert_eq!(
        column_ids(&runtime, "tracker"),
        ["done", "task", "estimate"]
    );
}

#[test]
fn undoing_a_removed_column_restores_its_position() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[2], 0).unwrap();
    let moved = column_ids(&runtime, &collection);
    runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision: runtime.current_revision().into(),
            collection: collection.clone(),
            field: moved[1].clone(),
        })
        .unwrap();
    undo(&mut runtime);
    assert_eq!(column_ids(&runtime, &collection), moved);
}

#[test]
fn a_duplicated_collection_keeps_the_column_order() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[2], 0).unwrap();
    let names = |runtime: &DesignerRuntime, collection: &str| {
        runtime
            .query_table(collection)
            .unwrap()
            .columns
            .into_iter()
            .map(|column| column.display_name)
            .collect::<Vec<_>>()
    };
    let expected = names(&runtime, &collection);
    let DesignerResponse::Duplicated(copy) = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: runtime.current_revision().into(),
            collection: collection.clone(),
            name: "Copy".into(),
        })
        .unwrap()
    else {
        panic!("expected a duplicated collection");
    };
    assert_eq!(names(&runtime, &copy.collection.key), expected);
}

#[test]
fn undoing_an_added_column_restores_the_previous_order() {
    let (mut runtime, collection) = fixture();
    let initial = column_ids(&runtime, &collection);
    move_column(&mut runtime, &collection, &initial[2], 0).unwrap();
    let schemas =
        |runtime: &DesignerRuntime| runtime.session.export_snapshot().document().schemas.clone();
    let before = schemas(&runtime);
    runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision: runtime.current_revision().into(),
            collection: collection.clone(),
            name: "Extra".into(),
            field_type: "text".into(),
            required: false,
            initializers: Vec::new(),
        })
        .unwrap();
    assert_ne!(schemas(&runtime), before);
    undo(&mut runtime);
    assert_eq!(schemas(&runtime), before);
}
