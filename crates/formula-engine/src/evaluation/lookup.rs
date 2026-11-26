use std::collections::BTreeSet;

use super::{
    CalculatedValue, CalculationFailure, ColumnRef, CriterionCell, CriterionKey, EntityId,
    Evaluation, Expression, FieldRef, FieldType, Number, ValueStore, exact_key,
};

impl<V: ValueStore> Evaluation<'_, V> {
    pub(super) fn lookup_result_number(
        &self,
        reference: &FieldRef,
    ) -> Result<Number, CalculationFailure> {
        if crate::field_type(self.document, reference) == Some(&FieldType::Auto) {
            return match self.calculated_reference(reference)? {
                CalculatedValue::Number(number) => Ok(number),
                _ => Err(CalculationFailure::TypeMismatch),
            };
        }
        self.reference(reference)
    }

    pub(super) fn lookup_row(
        &mut self,
        key: &Expression,
        column: &ColumnRef,
    ) -> Result<Option<EntityId>, CalculationFailure> {
        let key = match key {
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                self.lookup_reference_key(reference)?
            }
            Expression::SameRowReference(field) | Expression::AnchoredSameRowReference(field) => {
                self.lookup_reference_key(&FieldRef::new(self.owner.entity.clone(), field.clone()))?
            }
            expression => exact_key(CriterionCell::Number(self.number(expression)?.get())),
        };
        self.columns
            .lookup_index(self.document, self.values, column);
        Ok(self.columns.lookup_indexes[column].get(&key).cloned())
    }

    fn lookup_reference_key(
        &self,
        reference: &FieldRef,
    ) -> Result<CriterionKey, CalculationFailure> {
        if let Some(entity) = self.document.entities.get(&reference.entity) {
            if let Some(cell) = self.criterion_cell(entity, &reference.field)? {
                return Ok(exact_key(cell));
            }
        }
        Err(CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([reference.clone()]),
        })
    }
}
