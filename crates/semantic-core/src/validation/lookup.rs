use super::{
    ColumnRef, Diagnostic, DiagnosticCode, Document, Expression, FieldRef, FieldType,
    SemanticSubject, core_diagnostic, validate_formula_column, validate_formula_scalar_column,
};

fn column_type<'a>(document: &'a Document, column: &ColumnRef) -> Option<&'a FieldType> {
    Some(
        &document
            .schemas
            .get(&column.schema)?
            .fields
            .get(&column.field)?
            .field_type,
    )
}

pub(super) fn validate_lookup(
    document: &Document,
    owner: &FieldRef,
    (key, key_column, result_column): (&Expression, &ColumnRef, &ColumnRef),
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_formula_scalar_column(document, owner, key_column, path, diagnostics);
    let auto = document.entities.get(&owner.entity).and_then(|entity| {
        column_type(
            document,
            &ColumnRef::new(entity.schema.clone(), owner.field.clone()),
        )
    }) == Some(&FieldType::Auto);
    if !(auto && column_type(document, result_column) == Some(&FieldType::Auto)) {
        validate_formula_column(document, owner, result_column, path, diagnostics);
    }
    let column_type = column_type(document, key_column);
    let key_type = match key {
        Expression::DeletedReference {} => None,
        Expression::Reference(reference) | Expression::AnchoredReference(reference) => document
            .entities
            .get(&reference.entity)
            .and_then(|entity| document.schemas.get(&entity.schema))
            .and_then(|schema| schema.fields.get(&reference.field))
            .map(|field| &field.field_type),
        Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
            document
                .entities
                .get(&owner.entity)
                .and_then(|entity| document.schemas.get(&entity.schema))
                .and_then(|schema| schema.fields.get(field))
                .map(|field| &field.field_type)
        }
        _ => Some(&FieldType::Number),
    };
    let dynamic_key = auto
        && column_type == Some(&FieldType::Auto)
        && matches!(
            key_type,
            Some(FieldType::Number | FieldType::Text | FieldType::Date | FieldType::Boolean)
        );
    if key_column.schema != result_column.schema
        || (key_type != column_type
            && !dynamic_key
            && !matches!(key, Expression::DeletedReference {}))
    {
        diagnostics.push(core_diagnostic(
            path,
            DiagnosticCode::FORMULA_REFERENCE_TYPE_MISMATCH,
            "LOOKUP requires columns in one schema and a key of the key column's type",
            vec![SemanticSubject::EntityField(owner.clone())],
        ));
    }
}
