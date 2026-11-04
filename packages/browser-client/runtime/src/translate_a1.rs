//! Read-only A1 authoring boundary over the import translator and formula admission.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tachiko_formula_engine::{bind_expression, parse_expression, project_expression};
use tachiko_workspace_engine::patch_lifecycle::{ProposalId, SemanticRevision, TrustedInstant};
use tachiko_workspace_engine::{Document, FieldRef, FieldType};

use crate::interop_adapter::SummaryAggregate;
use crate::interop_document::{A1Geometry, excel_extremum, parse_a1_address, translate_a1_with};
use crate::{
    DesignerError, DesignerRuntime, FormulaUpdateRequest, designer_lifecycle, designer_preview,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct A1SheetGeometry {
    pub collection_id: String,
    pub header_row: usize,
    pub rows: Vec<String>,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct A1GeometryInput {
    pub header_row: usize,
    pub rows: Vec<String>,
    pub columns: Vec<String>,
    #[serde(default)]
    pub sheets: BTreeMap<String, A1SheetGeometry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslateA1FormulaInput {
    pub expected_revision: String,
    pub collection_id: String,
    pub row_entity_id: String,
    pub target_field_id: String,
    pub text: String,
    pub geometry: A1GeometryInput,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct A1TokenSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum A1TranslationNote {
    RelativeCopySemanticsChanged,
    RangeTracksCollection,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum A1RefusalCode {
    UnknownSheet,
    OutOfBounds,
    HeaderReference,
    UnsupportedRange,
    UnsupportedFunction,
    UnsupportedOperator,
    FormulaSyntax,
    FormulaTypeMismatch,
    FormulaCycle,
    InvalidGeometry,
    StaleRevision,
}

impl A1RefusalCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UnknownSheet => "unknown_sheet",
            Self::OutOfBounds => "out_of_bounds",
            Self::HeaderReference => "header_reference",
            Self::UnsupportedRange => "unsupported_range",
            Self::UnsupportedFunction => "unsupported_function",
            Self::UnsupportedOperator => "unsupported_operator",
            Self::FormulaSyntax => "formula_syntax",
            Self::FormulaTypeMismatch => "formula_type_mismatch",
            Self::FormulaCycle => "formula_cycle",
            Self::InvalidGeometry => "invalid_geometry",
            Self::StaleRevision => "stale_revision",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct A1Translation {
    pub revision: String,
    pub source: String,
    pub notes: Vec<A1TranslationNote>,
}

fn refusal(code: A1RefusalCode) -> DesignerError {
    DesignerError::A1Translation {
        code,
        span: A1TokenSpan { start: 0, end: 0 },
    }
}

fn grid_address(address: &str) -> Result<(usize, usize), DesignerError> {
    parse_a1_address(address).map_err(|_| {
        let address = address.strip_prefix('$').unwrap_or(address);
        let letters = address.bytes().take_while(u8::is_ascii_alphabetic).count();
        let row = address[letters..]
            .strip_prefix('$')
            .unwrap_or(&address[letters..]);
        let code =
            if letters > 0 && !row.is_empty() && row.bytes().all(|byte| byte.is_ascii_digit()) {
                A1RefusalCode::OutOfBounds
            } else {
                A1RefusalCode::FormulaSyntax
            };
        refusal(code)
    })
}

fn validate_sheet(sheet: &A1SheetGeometry, document: &Document) -> Result<(), DesignerError> {
    let invalid = || refusal(A1RefusalCode::InvalidGeometry);
    let schema = document
        .schemas
        .get(sheet.collection_id.as_str())
        .ok_or_else(invalid)?;
    if sheet.header_row == 0
        || sheet.header_row.checked_add(sheet.rows.len()).is_none()
        || sheet.columns.len() != schema.fields.len()
        || sheet.columns.iter().collect::<BTreeSet<_>>().len() != sheet.columns.len()
        || sheet.rows.iter().collect::<BTreeSet<_>>().len() != sheet.rows.len()
        || sheet
            .columns
            .iter()
            .any(|id| !schema.fields.contains_key(id.as_str()))
        || sheet.rows.iter().any(|id| {
            document
                .entities
                .get(id.as_str())
                .is_none_or(|entity| entity.schema != schema.id)
        })
    {
        return Err(invalid());
    }
    Ok(())
}

struct GridGeometry<'a> {
    sheets: Vec<(&'a str, A1SheetGeometry)>,
    owner: &'a str,
    notes: BTreeSet<A1TranslationNote>,
    tokens: Vec<(A1TokenSpan, std::ops::Range<usize>)>,
}

impl GridGeometry<'_> {
    fn source_span(&self, position: usize) -> A1TokenSpan {
        self.tokens
            .iter()
            .find(|(_, output)| output.contains(&position))
            .or_else(|| self.tokens.last())
            .map_or(A1TokenSpan { start: 0, end: 0 }, |(span, _)| *span)
    }
}

impl A1Geometry for GridGeometry<'_> {
    fn sheet(&self, name: &str) -> Result<usize, DesignerError> {
        self.sheets
            .iter()
            .position(|(label, _)| label.to_lowercase() == name.to_lowercase())
            .ok_or_else(|| refusal(A1RefusalCode::UnknownSheet))
    }

    fn cell(
        &mut self,
        sheet: usize,
        address: &str,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError> {
        let (column, row) = grid_address(address)?;
        let grid = &self.sheets[sheet].1;
        if row < grid.header_row {
            return Err(refusal(A1RefusalCode::HeaderReference));
        }
        let entity_id = grid
            .rows
            .get(row - grid.header_row)
            .ok_or_else(|| refusal(A1RefusalCode::OutOfBounds))?;
        let field_id = grid
            .columns
            .get(column)
            .ok_or_else(|| refusal(A1RefusalCode::OutOfBounds))?;
        let entity = &document.entities[entity_id.as_str()];
        let field = &document.schemas[&entity.schema].fields[field_id.as_str()];
        let row_absolute = address
            .trim_start_matches('$')
            .chars()
            .find(|character| !character.is_ascii_alphabetic())
            == Some('$');
        let same_row = sheet == 0 && entity_id == self.owner && !row_absolute;
        if !same_row && !row_absolute {
            self.notes
                .insert(A1TranslationNote::RelativeCopySemanticsChanged);
        }
        references.insert(FieldRef::new(entity_id.clone(), field_id.clone()));
        Ok(format!(
            "[{}.{}]",
            if same_row { "" } else { entity.key.as_str() },
            field.key.as_str()
        ))
    }

    fn aggregate(
        &mut self,
        argument: &str,
        current: usize,
        aggregate: SummaryAggregate,
        document: &Document,
        references: &mut BTreeSet<FieldRef>,
    ) -> Result<String, DesignerError> {
        let invalid = || refusal(A1RefusalCode::UnsupportedRange);
        let argument = argument.trim();
        let (sheet, range) = if let Some((name, range)) = argument.rsplit_once('!') {
            (
                self.sheet(&name.trim_matches('\'').replace("''", "'"))?,
                range,
            )
        } else {
            (current, argument)
        };
        let (first, last) = range.split_once(':').ok_or_else(invalid)?;
        let (column, first) = parse_a1_address(first.trim()).map_err(|_| invalid())?;
        let (last_column, last) = parse_a1_address(last.trim()).map_err(|_| invalid())?;
        let grid = &self.sheets[sheet].1;
        let schema = &document.schemas[grid.collection_id.as_str()];
        let field_id = grid.columns.get(column).ok_or_else(invalid)?;
        let field = &schema.fields[field_id.as_str()];
        let whole = document
            .entities
            .values()
            .filter(|entity| entity.schema == schema.id)
            .count()
            == grid.rows.len();
        if column != last_column
            || grid.rows.is_empty()
            || !whole
            || !(first == grid.header_row || first == grid.header_row - 1)
            || last != grid.header_row + grid.rows.len() - 1
            || field.field_type != FieldType::Number
        {
            return Err(invalid());
        }
        for row in &grid.rows {
            references.insert(FieldRef::new(row.clone(), field_id.clone()));
        }
        self.notes.insert(A1TranslationNote::RangeTracksCollection);
        Ok(format!(
            "[{}.{}:{}]",
            schema.key.as_str(),
            field.key.as_str(),
            aggregate.suffix()
        ))
    }

    fn record_token(&mut self, start: usize, end: usize, output_start: usize, output_end: usize) {
        self.tokens
            .push((A1TokenSpan { start, end }, output_start..output_end));
    }
    fn unknown_function(&self) -> DesignerError {
        refusal(A1RefusalCode::UnsupportedFunction)
    }
    fn operator(&self, ch: char) -> Result<(), DesignerError> {
        if ch == ':' {
            return Err(refusal(A1RefusalCode::UnsupportedRange));
        }
        if matches!(ch, '&' | '^' | '@' | '#') {
            return Err(refusal(A1RefusalCode::UnsupportedOperator));
        }
        Ok(())
    }
    fn locate(&self, error: DesignerError, start: usize, end: usize) -> DesignerError {
        let code = if let DesignerError::A1Translation { code, .. } = error {
            code
        } else {
            A1RefusalCode::FormulaSyntax
        };
        DesignerError::A1Translation {
            code,
            span: A1TokenSpan { start, end },
        }
    }
}

impl DesignerRuntime {
    /// Translate explicit grid coordinates without changing the resident occurrence.
    ///
    /// # Errors
    /// Returns a typed, located refusal for invalid geometry, unsupported text,
    /// stale revisions, or failed formula admission.
    #[allow(clippy::too_many_lines)] // Keep the read-only admission sequence together.
    pub fn translate_a1_formula(
        &self,
        input: &TranslateA1FormulaInput,
    ) -> Result<A1Translation, DesignerError> {
        use A1RefusalCode::{FormulaSyntax, InvalidGeometry, StaleRevision};
        if self.current_revision() != input.expected_revision {
            return Err(refusal(StaleRevision));
        }
        let snapshot = self.session.export_snapshot();
        let document = snapshot.document();
        let current = A1SheetGeometry {
            collection_id: input.collection_id.clone(),
            header_row: input.geometry.header_row,
            rows: input.geometry.rows.clone(),
            columns: input.geometry.columns.clone(),
        };
        validate_sheet(&current, document)?;
        if !current.rows.contains(&input.row_entity_id)
            || !current.columns.contains(&input.target_field_id)
        {
            return Err(refusal(InvalidGeometry));
        }
        let mut geometry = GridGeometry {
            sheets: vec![("", current)],
            owner: &input.row_entity_id,
            notes: BTreeSet::new(),
            tokens: Vec::new(),
        };
        let mut names = BTreeSet::new();
        for (name, sheet) in &input.geometry.sheets {
            if name.is_empty() || !names.insert(name.to_lowercase()) {
                return Err(refusal(InvalidGeometry));
            }
            validate_sheet(sheet, document)?;
            geometry.sheets.push((name, sheet.clone()));
        }
        let (source, _) =
            translate_a1_with(&input.text, &mut geometry, 0, document).map_err(|error| {
                if matches!(error, DesignerError::A1Translation { .. }) {
                    error
                } else {
                    geometry.locate(error, 0, input.text.len())
                }
            })?;
        let target = FieldRef::new(input.row_entity_id.clone(), input.target_field_id.clone());
        let locate = |error: DesignerError| {
            let failure = error.failure_projection(self.current_revision());
            let nonnumeric_target = matches!(&error,
                DesignerError::Lifecycle(tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError::CommandRejected { source })
                if matches!(source.as_ref(), tachiko_workspace_engine::WorkspaceError::NonNumericFormulaField { .. }));
            let code = match failure.code.as_str() {
                "formula_cycle" => A1RefusalCode::FormulaCycle,
                "formula_type_mismatch" => A1RefusalCode::FormulaTypeMismatch,
                _ if nonnumeric_target => A1RefusalCode::FormulaTypeMismatch,
                _ => FormulaSyntax,
            };
            let reference = failure
                .diagnostics
                .iter()
                .flat_map(|diagnostic| &diagnostic.facts)
                .find(|fact| fact.name == "reference")
                .map(|fact| format!("[{}]", fact.value));
            let position = reference
                .as_ref()
                .and_then(|reference| {
                    source.find(reference).or_else(|| {
                        let prefix = format!(
                            "[{}.",
                            document.entities[input.row_entity_id.as_str()].key.as_str()
                        );
                        reference
                            .strip_prefix(&prefix)
                            .and_then(|field| source.find(&format!("[.{field}")))
                    })
                })
                .or_else(|| {
                    failure
                        .diagnostics
                        .iter()
                        .flat_map(|diagnostic| &diagnostic.subjects)
                        .filter_map(|subject| {
                            let tachiko_workspace_engine::SemanticSubject::EntityField(reference) =
                                subject
                            else {
                                return None;
                            };
                            let entity = document.entities.get(&reference.entity)?;
                            let field = document
                                .schemas
                                .get(&entity.schema)?
                                .fields
                                .get(&reference.field)?;
                            let fixed = format!("[{}.{}]", entity.key.as_str(), field.key.as_str());
                            source.find(&fixed).or_else(|| {
                                if reference.entity.as_str() == input.row_entity_id {
                                    source.find(&format!("[.{}]", field.key.as_str()))
                                } else {
                                    None
                                }
                            })
                        })
                        .min()
                })
                .or_else(|| {
                    source.find(&format!(
                        "[.{}]",
                        document.schemas[input.collection_id.as_str()].fields
                            [input.target_field_id.as_str()]
                        .key
                        .as_str()
                    ))
                })
                .unwrap_or(0);
            DesignerError::A1Translation {
                code,
                span: geometry.source_span(position),
            }
        };
        let unbound = parse_expression(&source).map_err(|error| DesignerError::A1Translation {
            code: FormulaSyntax,
            span: geometry.source_span(error.position),
        })?;
        let expression = bind_expression(document, &target, &unbound).map_err(|error| {
            locate(
                tachiko_workspace_engine::WorkspaceError::FormulaBinding {
                    field: target.clone(),
                    source: Box::new(error),
                }
                .into(),
            )
        })?;
        // Apply precisely the import's Excel blank-reference guard to scalar MIN/MAX.
        let source = if source.contains("min(") || source.contains("max(") {
            let position = source
                .find("min(")
                .into_iter()
                .chain(source.find("max("))
                .min()
                .unwrap_or(0);
            let guard_refusal = || DesignerError::A1Translation {
                code: FormulaSyntax,
                span: geometry.source_span(position),
            };
            let guarded =
                excel_extremum(expression, &target, document).map_err(|_| guard_refusal())?;
            project_expression(document, &target, &guarded).map_err(|_| guard_refusal())?
        } else {
            source.clone()
        };
        // Request-local proposal identity and time never advance the resident counters.
        let mut lifecycle =
            designer_lifecycle(snapshot.document_scope(), document, &self.principal)
                .map_err(locate)?;
        let proposal = ProposalId::from("translate-a1/dry-run");
        let preview = lifecycle
            .propose_formula_update(
                snapshot.document_scope(),
                document,
                snapshot.revision(),
                FormulaUpdateRequest::new(
                    proposal.clone(),
                    SemanticRevision::from(input.expected_revision.clone()),
                    target,
                    &source,
                    self.principal.clone(),
                ),
                TrustedInstant::new(1),
            )
            .map_err(DesignerError::from)
            .and_then(|_| {
                designer_preview(
                    &mut lifecycle,
                    &snapshot,
                    &proposal,
                    &self.principal,
                    TrustedInstant::new(2),
                )
            });
        match preview {
            Ok(_)
            | Err(DesignerError::Lifecycle(
                tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError::NoChange,
            )) => {}
            Err(error) => return Err(locate(error)),
        }
        Ok(A1Translation {
            revision: self.current_revision().to_owned(),
            source,
            notes: geometry.notes.into_iter().collect(),
        })
    }
}
