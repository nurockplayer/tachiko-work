//! Preflight keeps the per-formula failure when no address index can be built.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    CanonicalAuthoringProjectionError, Document, Entity, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, Value, WorkspaceError,
    preflight_formula_projections,
};

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: FieldKey::from(id),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Number,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        "s".into(),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("f".into(), number_field("f")),
                ("g".into(), number_field("g")),
            ]),
        },
    );
    // The stored id and the nested id disagree, so no address index can build.
    let entity = Entity {
        id: "other".into(),
        key: "row".into(),
        schema: "s".into(),
        fields: BTreeMap::from([
            (
                FieldId::from("f"),
                Value::Formula(Expression::Reference(FieldRef::new("e1", "g"))),
            ),
            (FieldId::from("g"), Value::Number(Number::new(1.0).unwrap())),
        ]),
    };
    document.entities.insert("e1".into(), entity);
    document
}

#[test]
fn preflight_keeps_the_formula_failure_when_the_index_cannot_build() {
    let error = preflight_formula_projections(&document()).unwrap_err();

    match error {
        WorkspaceError::FormulaProjection { field, source } => {
            assert_eq!(field, FieldRef::new("e1", "f"));
            assert_eq!(
                source,
                CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
                    targets: BTreeSet::from([FieldRef::new("e1", "g")]),
                }
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
