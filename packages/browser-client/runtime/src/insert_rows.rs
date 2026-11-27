use std::collections::BTreeSet;

use super::{
    CapacityLimit, CollectionSpec, DesignerError, DesignerRuntime, Entity, FieldId, FieldRef,
    PublicationProjection, RowIds, RowInitializer, RowPosition, SchemaId, SemanticCommand,
    auto_input, capacity, column_fill_formulas, fill_new_rows, has_scalar_row_authoring_shape,
    is_native_table_spec, native_table_row, row_initializer_values, table_error, tracker_error,
};

impl DesignerRuntime {
    pub(super) fn insert_row(
        &mut self,
        (position, count): (Option<&RowPosition>, usize),
        expected: &str,
        collection: &str,
        initializers: &[RowInitializer],
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let spec = self.collection_spec(collection)?.clone();
        if !is_native_table_spec(&spec) {
            return Err(table_error(
                "row insertion is available for native scalar tables",
            ));
        }
        validate_count(position, count, initializers)?;
        capacity::check_limit(
            CapacityLimit::TableRows,
            self.max_table_rows(),
            spec.entities.len().saturating_add(count),
            Some(collection),
        )?;
        if self.row_serial > usize::MAX - self.row_identity_reserve() {
            return Err(tracker_error("row identity allocation is exhausted"));
        }
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let schema = document
            .schemas
            .get(&SchemaId::from(spec.summary.id.clone()))
            .ok_or_else(|| table_error("native table schema is unavailable"))?;
        if !has_scalar_row_authoring_shape(document, schema, &spec.entities) {
            return Err(table_error(
                "native table schema is not a scalar row profile",
            ));
        }
        let index = insertion_index(&spec, position)?;
        let fills = column_fill_formulas(document, &spec);
        let values = row_initializer_values(&spec, initializers, &fills)?;
        let mut allocated = BTreeSet::new();
        let mut row_ids = RowIds::new(&self.row_namespace, self.row_serial, "native_table_row");
        let mut rows = (0..count)
            .map(|_| native_table_row(document, &spec, &mut row_ids, &mut allocated))
            .collect::<Result<Vec<_>, _>>()?;
        let displays = initializer_displays(&spec, &rows[0], initializers);
        let typed = values.keys().cloned().collect::<BTreeSet<_>>();
        rows[0].fields.extend(values);
        fill_new_rows(document, &[], &fills, &mut rows, &vec![typed; count]);
        let (mut forward, mut inverse) = tachiko_workspace_engine::row_order::plan_insertion(
            document,
            &schema.id,
            &mut rows,
            index,
            position.is_some(),
        );
        for entity in rows {
            inverse.push(SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            });
            forward.push(SemanticCommand::AppendEntity { entity });
        }
        let publication = self.record_input_edit(expected, forward, inverse, displays)?;
        self.row_serial = row_ids.serial();
        Ok(publication)
    }
}

fn validate_count(
    position: Option<&RowPosition>,
    count: usize,
    initializers: &[RowInitializer],
) -> Result<(), DesignerError> {
    if count == 0 {
        return Err(table_error("row insertion count must be positive"));
    }
    if count > 1 && (!matches!(position, Some(RowPosition::Before(_))) || !initializers.is_empty())
    {
        return Err(table_error(
            "inserting multiple blank rows requires a before anchor and no initializers",
        ));
    }
    Ok(())
}

fn insertion_index(
    spec: &CollectionSpec,
    position: Option<&RowPosition>,
) -> Result<usize, DesignerError> {
    let Some(RowPosition::Before(anchor) | RowPosition::After(anchor)) = position else {
        return Ok(spec.entities.len());
    };
    let index = spec
        .entities
        .iter()
        .position(|id| id == anchor)
        .ok_or_else(|| table_error("insertion anchor is unavailable in this collection"))?;
    Ok(index + usize::from(matches!(position, Some(RowPosition::After(_)))))
}

fn initializer_displays(
    spec: &CollectionSpec,
    entity: &Entity,
    initializers: &[RowInitializer],
) -> Vec<auto_input::CellDisplay> {
    let mut displays = Vec::new();
    for initializer in initializers {
        let field = FieldRef::new(entity.id.clone(), FieldId::from(initializer.field.clone()));
        let column = spec
            .columns
            .iter()
            .find(|column| column.id == field.field)
            .expect("row initializer field was validated");
        auto_input::collect_display(
            &column.field_type,
            &field,
            &initializer.input,
            &mut displays,
        );
    }
    displays
}
