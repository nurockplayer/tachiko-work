use tachiko_semantic_core::{FieldConstraint, FieldDefinition, Schema};

use crate::{OptionalChoice, choose, choose_optional};

pub(super) fn apply_common_schema(
    base: &Schema,
    left: &Schema,
    right: &Schema,
    output: &mut Schema,
) {
    for (field_id, output_field) in &mut output.fields {
        if let Some(constraint) = select_field_constraint(
            base.fields.get(field_id),
            left.fields.get(field_id),
            right.fields.get(field_id),
        ) {
            output_field.constraint = constraint;
        }
    }
}

pub(super) fn apply_optional_schema(
    base: Option<&Schema>,
    left: Option<&Schema>,
    right: Option<&Schema>,
    output: &mut Schema,
) {
    let OptionalChoice::Chosen(Some(selected_schema)) = choose_optional(base, left, right) else {
        return;
    };
    for (field_id, output_field) in &mut output.fields {
        if let Some(selected_field) = selected_schema.fields.get(field_id) {
            output_field.constraint = selected_field.constraint.clone();
        }
    }
}

fn select_field_constraint(
    base: Option<&FieldDefinition>,
    left: Option<&FieldDefinition>,
    right: Option<&FieldDefinition>,
) -> Option<FieldConstraint> {
    match (base, left, right) {
        (Some(b), Some(l), Some(r)) => choose(&b.constraint, &l.constraint, &r.constraint),
        (b, l, r) => match choose_optional(b, l, r) {
            OptionalChoice::Chosen(Some(field)) => Some(field.constraint.clone()),
            OptionalChoice::Chosen(None) => None,
            OptionalChoice::Conflict => {
                unreachable!("conflicting optional field emitted a conflict")
            }
        },
    }
}
