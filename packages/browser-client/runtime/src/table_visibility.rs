use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tachiko_workspace_engine::{
    EntityId, FieldId, FieldType, PresentationStore, SchemaId, TablePresentation,
};

use crate::{
    CollectionSpec, ColumnStyleProjection, DesignerError, DesignerRequest, DesignerResponse,
    DesignerRuntime, FieldConstraintProjection, FieldProjection, PublicationProjection,
    constraint_projection, default_required, field_type_name, interop_adapter, table_error,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ColumnProjection {
    #[serde(default = "default_required")]
    pub required: bool,
    pub id: String,
    pub key: String,
    pub display_name: String,
    /// Stored display name is present, even when equal to the generated key.
    /// Omission means the projected name is only the key fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name_explicit: Option<bool>,
    pub field_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropdown_options: Option<Vec<String>>,
    /// The declared durable constraint, if any. Absence means unconstrained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<FieldConstraintProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<tachiko_workspace_engine::NumberFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_format: Option<tachiko_workspace_engine::DateFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ColumnStyleProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Omission means visible.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RowProjection {
    pub id: String,
    /// Omission means visible.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    pub key: String,
    /// One-based stored position in the collection. A window sets it so a host
    /// keeps the row number a filter or view sort would otherwise change;
    /// every other reply omits the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    /// Declared fields without a value, in column presentation order.
    /// `fields` retains its sparse present-value payload; absence is never a placeholder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blank_fields: Vec<String>,
    pub fields: Vec<FieldProjection>,
    /// Resolved stored styles in column order, shared with imported rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<interop_adapter::CellStyle>,
}

/// The single column projection shared by the complete table query and the
/// bounded window query.
pub(super) fn column_projections(
    spec: &CollectionSpec,
    presentation: &PresentationStore,
) -> Vec<ColumnProjection> {
    let table = presentation
        .tables
        .get(&SchemaId::from(spec.summary.id.clone()));
    spec.columns
        .iter()
        .map(|column| ColumnProjection {
            required: column.required,
            id: column.id.to_string(),
            key: column.key.clone(),
            display_name: column.display_name.clone(),
            display_name_explicit: column.display_name_explicit.then_some(true),
            field_type: field_type_name(&column.field_type).to_owned(),
            dropdown_options: match &column.constraint {
                tachiko_workspace_engine::FieldConstraint::TextLiteralSet { values } => {
                    Some(values.clone())
                }
                _ if column.field_type == FieldType::Boolean => {
                    Some(vec!["true".to_owned(), "false".to_owned()])
                }
                _ => None,
            },
            constraint: constraint_projection(&column.constraint),
            number_format: column.number_format.clone(),
            date_format: column.date_format,
            style: table
                .and_then(|table| table.column_styles.get(&column.id))
                .and_then(|id| presentation.styles.get(id))
                .and_then(|style| {
                    (style.alignment.is_some() || style.wrap.is_some()).then_some(
                        ColumnStyleProjection {
                            alignment: style.alignment.map(Into::into),
                            wrap: style.wrap,
                        },
                    )
                }),
            hidden: table.is_some_and(|table| table.hidden_fields.contains(&column.id)),
            width: table
                .and_then(|table| table.column_widths.get(&column.id))
                .copied(),
        })
        .collect()
}

impl DesignerRuntime {
    pub(super) fn dispatch_table_presentation(
        &mut self,
        request: DesignerRequest,
    ) -> Result<DesignerResponse, DesignerError> {
        let publication = match request {
            DesignerRequest::SetColumnWidth {
                expected_revision,
                collection,
                field,
                width,
            } => self.set_column_width(&expected_revision, &collection, &field, width),
            DesignerRequest::SetTableVisibility {
                expected_revision,
                collection,
                entities,
                fields,
                hidden,
            } => self.set_table_visibility(
                &expected_revision,
                &collection,
                (entities, fields),
                hidden,
            ),
            _ => unreachable!("table presentation dispatch"),
        }?;
        Ok(DesignerResponse::Published(publication))
    }

    fn set_table_visibility(
        &mut self,
        expected: &str,
        collection: &str,
        (entities, fields): (Vec<String>, Vec<String>),
        hidden: bool,
    ) -> Result<PublicationProjection, DesignerError> {
        self.check_revision(expected)?;
        let schema = SchemaId::from(self.collection_spec(collection)?.summary.id.clone());
        let document = self.session.document();
        let entities: BTreeSet<_> = entities.into_iter().map(EntityId::from).collect();
        let fields: BTreeSet<_> = fields.into_iter().map(FieldId::from).collect();
        if entities.iter().any(|entity| {
            !document
                .entities
                .get(entity)
                .is_some_and(|row| row.schema == schema)
        }) {
            return Err(table_error("visibility row is unavailable in this table"));
        }
        if fields
            .iter()
            .any(|field| !document.schemas[&schema].fields.contains_key(field))
        {
            return Err(table_error(
                "visibility column is unavailable in this table",
            ));
        }
        let mut candidate = self.presentation.clone();
        let table = candidate.tables.entry(schema.clone()).or_default();
        update_hidden_ids(&mut table.hidden_rows, entities, hidden);
        update_hidden_ids(&mut table.hidden_fields, fields, hidden);
        if *table == TablePresentation::default() {
            candidate.tables.remove(&schema);
        }
        self.record_presentation_change(expected, candidate)
    }

    pub(super) fn row_hidden(&self, spec: &CollectionSpec, entity: &EntityId) -> bool {
        self.presentation
            .tables
            .get(spec.summary.id.as_str())
            .is_some_and(|table| table.hidden_rows.contains(entity))
    }
}

fn update_hidden_ids<T: Ord>(set: &mut BTreeSet<T>, mut targets: BTreeSet<T>, hidden: bool) {
    if hidden {
        set.append(&mut targets);
    } else {
        set.retain(|id| !targets.contains(id));
    }
}

pub(super) fn import_visibility(
    source: &interop_adapter::SourceSheet,
    sheet: &crate::interop_document::InteropSheetMetadata,
) -> TablePresentation {
    TablePresentation {
        hidden_rows: source
            .layout
            .hidden_rows
            .iter()
            .map(|&row| EntityId::from(sheet.rows[row].entity_id.clone()))
            .collect(),
        hidden_fields: source
            .layout
            .hidden_columns
            .iter()
            .map(|&column| FieldId::from(sheet.columns[column].field_id.clone()))
            .collect(),
        ..TablePresentation::default()
    }
}

impl DesignerRuntime {
    pub(super) fn install_text_capacity_visibility(
        &mut self,
        workbook: &interop_adapter::SourceWorkbook,
        metadata: &crate::InteropMetadata,
    ) -> Result<(), DesignerError> {
        let mut store = PresentationStore::default();
        for (source, sheet) in workbook.sheets.iter().zip(&metadata.sheets) {
            let table = import_visibility(source, sheet);
            if table != TablePresentation::default() {
                store
                    .tables
                    .insert(SchemaId::from(sheet.schema_id.clone()), table);
            }
        }
        self.set_presentation(store)
    }

    pub(super) fn refresh_imported_table_with_presentation(
        &self,
        projection: &mut crate::interop_document::ImportedProjection,
    ) -> Result<(), DesignerError> {
        let limit = if self.profile == crate::SessionProfile::TextCapacity {
            if self.presentation.is_empty() {
                return Ok(());
            }
            crate::MAX_TEXT_CAPACITY_PROJECTION_BYTES
        } else {
            crate::MAX_PROJECTION_BYTES
        };
        projection.opened = crate::OpenedProjection {
            bootstrap: self.bootstrap_projection(),
            table: self.query_table(&self.default_collection)?,
        };
        crate::ensure_complete_projection_size("imported", projection, limit)
    }

    pub(super) fn export_native_visibility(
        &self,
        spec: &CollectionSpec,
        values_only: bool,
    ) -> interop_adapter::SourceLayout {
        export_visibility(
            self.presentation.tables.get(spec.summary.id.as_str()),
            spec.entities.iter().map(EntityId::as_str),
            spec.columns.iter().map(|column| column.id.as_str()),
            values_only,
        )
    }

    pub(super) fn export_imported_visibility(
        &self,
        sheet: &crate::interop_document::InteropSheetMetadata,
        document: &tachiko_workspace_engine::Document,
        values_only: bool,
    ) -> interop_adapter::SourceLayout {
        export_visibility(
            self.presentation.tables.get(sheet.schema_id.as_str()),
            sheet
                .rows
                .iter()
                .map(|row| row.entity_id.as_str())
                .filter(|id| document.entities.contains_key(*id)),
            sheet.columns.iter().map(|column| column.field_id.as_str()),
            values_only,
        )
    }
}

fn export_visibility<'a>(
    table: Option<&TablePresentation>,
    rows: impl Iterator<Item = &'a str>,
    fields: impl Iterator<Item = &'a str>,
    values_only: bool,
) -> interop_adapter::SourceLayout {
    let Some(table) = table.filter(|_| !values_only) else {
        return interop_adapter::SourceLayout::default();
    };
    interop_adapter::SourceLayout {
        hidden_rows: rows
            .enumerate()
            .filter_map(|(i, row)| table.hidden_rows.contains(row).then_some(i))
            .collect(),
        hidden_columns: fields
            .enumerate()
            .filter_map(|(i, field)| table.hidden_fields.contains(field).then_some(i))
            .collect(),
        ..interop_adapter::SourceLayout::default()
    }
}
