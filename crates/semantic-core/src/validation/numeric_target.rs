use super::{
    Diagnostic, DiagnosticCode, DiagnosticFact, FieldRef, FieldType, SemanticSubject,
    core_diagnostic, field_type_name,
};

pub(super) fn numeric_target_mismatch(
    path: &str,
    owner: &FieldRef,
    reference: &FieldRef,
    field_type: &FieldType,
) -> Diagnostic {
    core_diagnostic(
        path,
        DiagnosticCode::FORMULA_REFERENCE_TYPE_MISMATCH,
        format!("formula reference '{reference}' does not target a numeric field"),
        vec![SemanticSubject::EntityField(owner.clone())],
    )
    .with_related_subjects(vec![SemanticSubject::EntityField(reference.clone())])
    .with_fact(DiagnosticFact::new("expected_kind", "number"))
    .with_fact(DiagnosticFact::new(
        "actual_kind",
        field_type_name(field_type),
    ))
}
