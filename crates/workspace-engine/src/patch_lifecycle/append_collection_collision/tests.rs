use crate::patch_lifecycle::{
    FieldConstraint, FieldId, SchemaId, SemanticCommand, SemanticPatchBody,
    duplicate_constraint_write, tests::collection_command,
};

#[test]
fn first_colliding_collection_append_wins_in_command_order() {
    let constraint = |schema: &str| SemanticCommand::SetFieldConstraint {
        schema: SchemaId::from(schema),
        field: FieldId::from("field"),
        constraint: FieldConstraint::None,
    };
    let body = SemanticPatchBody::atomic_batch(vec![
        constraint("alpha"),
        constraint("zeta"),
        collection_command("zeta", true),
        collection_command("alpha", true),
    ])
    .unwrap();

    assert_eq!(
        duplicate_constraint_write(&body),
        Some((SchemaId::from("zeta"), FieldId::from("field")))
    );
}
