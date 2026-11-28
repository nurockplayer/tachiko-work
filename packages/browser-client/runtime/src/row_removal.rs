use super::{
    BTreeSet, CollectionSpec, DesignerError, Document, EntityId, Expression, FieldRef, FieldType,
    SemanticCommand, Value,
};

pub(super) fn rewrite_formulas(
    document: &Document,
    spec: &CollectionSpec,
    removed: &BTreeSet<EntityId>,
    forward: &mut Vec<SemanticCommand>,
    inverse: &mut Vec<SemanticCommand>,
) -> Result<(), DesignerError> {
    let mut dependent_cells = 0;
    for entity in document
        .entities
        .values()
        .filter(|row| !removed.contains(&row.id))
    {
        for (field, value) in &entity.fields {
            let Some(expression) = affected_formula(value, removed) else {
                continue;
            };
            if document.schemas[&entity.schema].fields[field].field_type != FieldType::Auto {
                dependent_cells += 1;
                continue;
            }
            let target = FieldRef::new(entity.id.clone(), field.clone());
            let mut rewritten = expression.clone();
            mark_deleted_references(&mut rewritten, removed);
            forward.push(SemanticCommand::set_field_value(
                target.clone(),
                Value::Formula(rewritten),
            ));
            inverse.push(SemanticCommand::set_field_value(target, value.clone()));
        }
    }
    if dependent_cells > 0 {
        return Err(DesignerError::FormulaDependencyInUse {
            dependent_cells,
            subject: format!("removed rows in collection '{}'", spec.summary.display_name),
        });
    }
    Ok(())
}

fn affected_formula<'a>(value: &'a Value, removed: &BTreeSet<EntityId>) -> Option<&'a Expression> {
    match value {
        Value::Formula(expression) if references_removed_row(expression, removed) => {
            Some(expression)
        }
        _ => None,
    }
}

fn is_removed_reference(expression: &Expression, removed: &BTreeSet<EntityId>) -> bool {
    matches!(expression, Expression::Reference(target) | Expression::AnchoredReference(target)
        if removed.contains(&target.entity))
}

fn references_removed_row(expression: &Expression, removed: &BTreeSet<EntityId>) -> bool {
    is_removed_reference(expression, removed)
        || expression
            .children()
            .into_iter()
            .any(|child| references_removed_row(child, removed))
}

fn mark_deleted_references(expression: &mut Expression, removed: &BTreeSet<EntityId>) {
    if is_removed_reference(expression, removed) {
        *expression = Expression::DeletedReference {};
    } else {
        for child in expression.children_mut() {
            mark_deleted_references(child, removed);
        }
    }
}
