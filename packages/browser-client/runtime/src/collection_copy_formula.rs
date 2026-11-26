//! Remap formula dependencies for a whole collection or a copied rectangle.
use std::collections::BTreeMap;

use tachiko_workspace_engine::ColumnCriterion;

use crate::{
    ColumnRef, ConditionalAggregate, DesignerError, EntityId, Expression, FieldId, FieldRef,
    SchemaId, Value, table_error,
};

pub(super) struct CopyReferences<'a> {
    entities: &'a BTreeMap<EntityId, EntityId>,
    fields: &'a BTreeMap<FieldId, FieldId>,
    schemas: &'a BTreeMap<SchemaId, SchemaId>,
    source_row: Option<&'a EntityId>,
}

impl<'a> CopyReferences<'a> {
    pub(super) fn whole(
        entities: &'a BTreeMap<EntityId, EntityId>,
        fields: &'a BTreeMap<FieldId, FieldId>,
        schemas: &'a BTreeMap<SchemaId, SchemaId>,
    ) -> Self {
        Self {
            entities,
            fields,
            schemas,
            source_row: None,
        }
    }

    pub(super) fn for_row(self, source_row: &'a EntityId) -> Self {
        Self {
            source_row: Some(source_row),
            ..self
        }
    }

    pub(super) fn value(&self, value: &Value) -> Result<Value, DesignerError> {
        match value {
            // A typed reference column retains its source schema in a range copy.
            Value::Reference(entity) if self.source_row.is_none() => Ok(Value::Reference(
                self.entities.get(entity).unwrap_or(entity).clone(),
            )),
            Value::Formula(expression) => {
                let mut expression = expression.clone();
                self.expression(&mut expression)?;
                Ok(Value::Formula(expression))
            }
            Value::Reference(_)
            | Value::Number(_)
            | Value::Text(_)
            | Value::Boolean(_)
            | Value::Date(_) => Ok(value.clone()),
        }
    }

    pub(super) fn expression(&self, expression: &mut Expression) -> Result<(), DesignerError> {
        match expression {
            Expression::Reference(reference) | Expression::AnchoredReference(reference) => {
                self.cell(reference)
            }
            Expression::SameRowReference(_) | Expression::AnchoredSameRowReference(_) => {
                self.same_row(expression)
            }
            Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                self.columns([column])
            }
            Expression::ColumnConditional {
                aggregate,
                criteria,
            } => self.conditional(aggregate, criteria),
            Expression::Lookup {
                key,
                key_column,
                result_column,
                if_missing,
            } => self.lookup(key, [key_column, result_column], if_missing.as_deref_mut()),
            _ => {
                for child in expression.children_mut() {
                    self.expression(child)?;
                }
                Ok(())
            }
        }
    }

    fn conditional(
        &self,
        aggregate: &mut ConditionalAggregate,
        criteria: &mut [ColumnCriterion],
    ) -> Result<(), DesignerError> {
        let value_column = match aggregate {
            ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
                Some(column)
            }
            ConditionalAggregate::Count => None,
        };
        self.columns(
            value_column
                .into_iter()
                .chain(criteria.iter_mut().map(|criterion| &mut criterion.column)),
        )?;
        for criterion in criteria {
            if let Some(expression) = &mut criterion.value {
                self.expression(expression)?;
            }
        }
        Ok(())
    }

    fn lookup(
        &self,
        key: &mut Expression,
        columns: [&mut ColumnRef; 2],
        if_missing: Option<&mut Expression>,
    ) -> Result<(), DesignerError> {
        self.columns(columns)?;
        self.expression(key)?;
        if let Some(fallback) = if_missing {
            self.expression(fallback)?;
        }
        Ok(())
    }

    fn cell(&self, reference: &mut FieldRef) -> Result<(), DesignerError> {
        if let Some(entity) = self.entities.get(&reference.entity) {
            if let Some(field) = self.fields.get(&reference.field) {
                *reference = FieldRef::new(entity.clone(), field.clone());
            } else if self.source_row.is_none() {
                return Err(table_error(&format!(
                    "internal formula reference '{}' has no copied field",
                    reference.field
                )));
            }
        }
        Ok(())
    }

    fn same_row(&self, expression: &mut Expression) -> Result<(), DesignerError> {
        let (field, anchored) = match expression {
            Expression::SameRowReference(field) => (field, false),
            Expression::AnchoredSameRowReference(field) => (field, true),
            _ => unreachable!("same-row reference expected"),
        };
        if let Some(copied) = self.fields.get(field) {
            field.clone_from(copied);
        } else if let Some(row) = self.source_row {
            let reference = FieldRef::new(row.clone(), field.clone());
            *expression = if anchored {
                Expression::AnchoredReference(reference)
            } else {
                Expression::Reference(reference)
            };
        } else {
            return Err(table_error(&format!(
                "internal same-row formula field '{field}' has no copied field"
            )));
        }
        Ok(())
    }

    fn columns<'b>(
        &self,
        columns: impl IntoIterator<Item = &'b mut ColumnRef>,
    ) -> Result<(), DesignerError> {
        let columns: Vec<_> = columns.into_iter().collect();
        // LOOKUP and conditional aggregates share a row domain across columns.
        // The whole dependency must fit in the copy to move to the new schema.
        if self.source_row.is_some()
            && columns.iter().any(|column| {
                !self.schemas.contains_key(&column.schema)
                    || !self.fields.contains_key(&column.field)
            })
        {
            return Ok(());
        }
        for column in columns {
            if let Some(schema) = self.schemas.get(&column.schema) {
                let field = self.fields.get(&column.field).ok_or_else(|| {
                    table_error(&format!(
                        "internal column-total field '{}' has no copied field",
                        column.field
                    ))
                })?;
                column.schema.clone_from(schema);
                column.field.clone_from(field);
            }
        }
        Ok(())
    }
}
