use super::{
    ColumnProjection, ConstraintFailureReason, DesignerError, DesignerRequest, DesignerResponse,
    DesignerRuntime, FieldConstraintInput, FieldConstraintProjection, FieldTarget,
    NewTableColumnInput, RowInitializer, ScalarEditInput, open_project,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000452";
const LITERALS: [&str; 3] = ["Doing", "Done", "Todo"];

fn columns() -> Vec<NewTableColumnInput> {
    vec![
        NewTableColumnInput {
            required: true,
            name: "Status".into(),
            field_type: "text".into(),
        },
        NewTableColumnInput {
            required: true,
            name: "Amount".into(),
            field_type: "number".into(),
        },
    ]
}

fn list(values: &[&str]) -> FieldConstraintInput {
    FieldConstraintInput::TextLiteralSet {
        values: values.iter().map(|value| (*value).to_owned()).collect(),
    }
}

fn range(min: f64, max: f64) -> FieldConstraintInput {
    FieldConstraintInput::NumberInclusiveRange { min, max }
}

fn published(reply: Result<DesignerResponse, DesignerError>) -> Result<String, DesignerError> {
    match reply? {
        DesignerResponse::Published(publication) => Ok(publication.resulting_revision),
        other => panic!("unexpected response {other:?}"),
    }
}

fn list_values(values: &[&str]) -> FieldConstraintProjection {
    FieldConstraintProjection::TextLiteralSet {
        values: values.iter().map(|value| (*value).to_owned()).collect(),
    }
}

struct Fixture {
    runtime: DesignerRuntime,
    collection: String,
    status: String,
    amount: String,
    row: String,
}

impl Fixture {
    fn new() -> Self {
        let mut runtime = DesignerRuntime::new_table(OCCURRENCE, "Tasks", &columns()).unwrap();
        let collection = runtime.collection_specs.keys().next().unwrap().clone();
        let table = runtime.query_table(&collection).unwrap();
        let column = |key: &str| {
            table
                .columns
                .iter()
                .find(|column| column.key == key)
                .unwrap()
                .id
                .clone()
        };
        let status = column("status");
        let amount = column("amount");
        let revision = runtime.current_revision().to_owned();
        runtime
            .insert_row(
                &revision,
                &collection,
                &[
                    RowInitializer {
                        field: status.clone(),
                        input: ScalarEditInput::Text {
                            value: "Todo".into(),
                        },
                    },
                    RowInitializer {
                        field: amount.clone(),
                        input: ScalarEditInput::Number { input: "50".into() },
                    },
                ],
            )
            .unwrap();
        let row = runtime.query_table(&collection).unwrap().rows[0].id.clone();
        Self {
            runtime,
            collection,
            status,
            amount,
            row,
        }
    }

    fn set(
        &mut self,
        field: &str,
        constraint: FieldConstraintInput,
    ) -> Result<String, DesignerError> {
        let expected_revision = self.runtime.current_revision().to_owned();
        published(self.runtime.handle(DesignerRequest::SetColumnConstraint {
            expected_revision,
            collection: self.collection.clone(),
            field: field.to_owned(),
            constraint,
        }))
    }

    fn edit(&mut self, field: &str, input: ScalarEditInput) -> Result<String, DesignerError> {
        let expected_revision = self.runtime.current_revision().to_owned();
        published(self.runtime.handle(DesignerRequest::EditScalar {
            expected_revision,
            target: FieldTarget {
                entity: self.row.clone(),
                field: field.to_owned(),
            },
            input,
        }))
    }

    fn undo(&mut self) -> Result<String, DesignerError> {
        let expected_revision = self.runtime.current_revision().to_owned();
        published(
            self.runtime
                .handle(DesignerRequest::Undo { expected_revision }),
        )
    }

    fn redo(&mut self) -> Result<String, DesignerError> {
        let expected_revision = self.runtime.current_revision().to_owned();
        published(
            self.runtime
                .handle(DesignerRequest::Redo { expected_revision }),
        )
    }

    fn column(&self, field: &str) -> ColumnProjection {
        self.runtime
            .query_table(&self.collection)
            .unwrap()
            .columns
            .into_iter()
            .find(|column| column.id == field)
            .unwrap()
    }

    fn status(&self) -> String {
        self.status.clone()
    }

    fn amount(&self) -> String {
        self.amount.clone()
    }

    fn add_row(&mut self, status: &str, amount: &str) -> String {
        let revision = self.runtime.current_revision().to_owned();
        published(self.runtime.handle(DesignerRequest::InsertRow {
            expected_revision: revision,
            collection: self.collection.clone(),
            initializers: vec![
                RowInitializer {
                    field: self.status.clone(),
                    input: ScalarEditInput::Text {
                        value: status.into(),
                    },
                },
                RowInitializer {
                    field: self.amount.clone(),
                    input: ScalarEditInput::Number {
                        input: amount.into(),
                    },
                },
            ],
        }))
        .unwrap();
        let rows = self.runtime.query_table(&self.collection).unwrap().rows;
        rows.last().unwrap().id.clone()
    }
}

#[test]
fn authoring_projects_lists_ranges_and_valid_edits() {
    let mut fixture = Fixture::new();
    let (status, amount) = (fixture.status(), fixture.amount());
    fixture.set(&status, list(&LITERALS)).unwrap();
    fixture.set(&amount, range(0.0, 100.0)).unwrap();

    let status_column = fixture.column(&status);
    assert_eq!(status_column.constraint, Some(list_values(&LITERALS)));
    assert_eq!(
        status_column.dropdown_options.as_deref(),
        Some(&["Doing".to_owned(), "Done".to_owned(), "Todo".to_owned()][..])
    );
    let amount_column = fixture.column(&amount);
    assert_eq!(
        amount_column.constraint,
        Some(FieldConstraintProjection::NumberInclusiveRange {
            min: 0.0,
            max: 100.0
        })
    );
    assert_eq!(amount_column.dropdown_options, None);

    // Both bounds are inclusive, and every declared literal is accepted.
    for value in ["0", "100"] {
        fixture
            .edit(
                &amount,
                ScalarEditInput::Number {
                    input: value.into(),
                },
            )
            .unwrap();
    }
    for value in LITERALS {
        fixture
            .edit(
                &status,
                ScalarEditInput::Text {
                    value: value.into(),
                },
            )
            .unwrap();
    }
}

#[test]
fn invalid_edits_return_typed_refusals_with_the_allowed_shape() {
    let mut fixture = Fixture::new();
    let (status, amount) = (fixture.status(), fixture.amount());
    fixture.set(&status, list(&LITERALS)).unwrap();
    fixture.set(&amount, range(0.0, 100.0)).unwrap();
    let before = fixture.runtime.current_revision().to_owned();

    let refusals = [
        (
            status.as_str(),
            ScalarEditInput::Text {
                value: "Blocked".into(),
            },
            list_values(&LITERALS),
        ),
        (
            amount.as_str(),
            ScalarEditInput::Number {
                input: "101".into(),
            },
            FieldConstraintProjection::NumberInclusiveRange {
                min: 0.0,
                max: 100.0,
            },
        ),
        (
            amount.as_str(),
            ScalarEditInput::Number { input: "-1".into() },
            FieldConstraintProjection::NumberInclusiveRange {
                min: 0.0,
                max: 100.0,
            },
        ),
    ];
    for (field, input, expected_constraint) in refusals {
        let field_id = field.to_owned();
        match fixture.edit(field, input).unwrap_err() {
            DesignerError::FieldConstraintViolation {
                reason,
                schema,
                field,
                constraint,
                offending_rows,
                message,
            } => {
                assert_eq!(reason, ConstraintFailureReason::FieldValue);
                assert_eq!(
                    schema.as_str(),
                    fixture
                        .runtime
                        .query_table(&fixture.collection)
                        .unwrap()
                        .collection
                        .id
                );
                assert_eq!(field.as_str(), field_id);
                assert_eq!(constraint, expected_constraint);
                assert_eq!(offending_rows, 1);
                assert!(message.contains("refuses this value"));
            }
            other => panic!("unexpected refusal {other:?}"),
        }
        // A refused edit publishes nothing, including its revision.
        assert_eq!(fixture.runtime.current_revision(), before);
    }
    let failure = DesignerError::FieldConstraintViolation {
        schema: fixture.collection.as_str().into(),
        field: status.as_str().into(),
        constraint: list_values(&LITERALS),
        offending_rows: 1,
        reason: ConstraintFailureReason::FieldValue,
        message: "message".into(),
    }
    .failure_projection(&before);
    assert_eq!(failure.code, "field_constraint_violation");
    let structured = failure.constraint_failure.unwrap();
    assert_eq!(structured.field, status);
    assert_eq!(structured.constraint, list_values(&LITERALS));
}

#[test]
fn clearing_a_constraint_restores_unconstrained_editing() {
    let mut fixture = Fixture::new();
    let status = fixture.status();
    fixture.set(&status, list(&LITERALS)).unwrap();
    fixture.set(&status, FieldConstraintInput::None).unwrap();
    let column = fixture.column(&status);
    assert_eq!(column.constraint, None);
    assert_eq!(column.dropdown_options, None);
    let wire = serde_json::to_value(&column).unwrap();
    assert!(wire.get("constraint").is_none());
    fixture
        .edit(
            &status,
            ScalarEditInput::Text {
                value: "Blocked".into(),
            },
        )
        .unwrap();
}

#[test]
fn constraint_replacements_undo_and_redo_exactly() {
    let mut fixture = Fixture::new();
    let status = fixture.status();
    fixture.set(&status, list(&LITERALS)).unwrap();
    fixture.set(&status, list(&["Doing", "Todo"])).unwrap();
    assert_eq!(
        fixture.column(&status).constraint,
        Some(list_values(&["Doing", "Todo"]))
    );

    fixture.undo().unwrap();
    assert_eq!(
        fixture.column(&status).constraint,
        Some(list_values(&LITERALS))
    );
    fixture.undo().unwrap();
    assert_eq!(fixture.column(&status).constraint, None);
    assert_eq!(fixture.column(&status).dropdown_options, None);

    fixture.redo().unwrap();
    assert_eq!(
        fixture.column(&status).constraint,
        Some(list_values(&LITERALS))
    );
    fixture.redo().unwrap();
    assert_eq!(
        fixture.column(&status).constraint,
        Some(list_values(&["Doing", "Todo"]))
    );
    // The narrowed declaration still refuses values outside the new set.
    assert!(
        fixture
            .edit(
                &status,
                ScalarEditInput::Text {
                    value: "Done".into(),
                },
            )
            .is_err()
    );
}

#[test]
fn declaring_a_constraint_refuses_existing_violations_atomically() {
    let mut fixture = Fixture::new();
    fixture.add_row("Blocked", "50");
    fixture.add_row("Blocked", "50");
    let status = fixture.status();
    let before = fixture.runtime.current_revision().to_owned();

    match fixture.set(&status, list(&LITERALS)).unwrap_err() {
        DesignerError::FieldConstraintViolation {
            reason,
            offending_rows,
            constraint,
            ..
        } => {
            assert_eq!(reason, ConstraintFailureReason::ExistingValues);
            assert_eq!(offending_rows, 2);
            assert_eq!(constraint, list_values(&LITERALS));
        }
        other => panic!("unexpected refusal {other:?}"),
    }
    assert_eq!(fixture.runtime.current_revision(), before);
    assert_eq!(fixture.column(&status).constraint, None);
}

#[test]
fn non_finite_bounds_are_refused_without_publication() {
    let mut fixture = Fixture::new();
    let amount = fixture.amount();
    let before = fixture.runtime.current_revision().to_owned();
    // Non-finite bounds are refused while decoding the declaration itself.
    for declaration in [range(f64::NAN, 100.0), range(0.0, f64::INFINITY)] {
        assert!(fixture.set(&amount, declaration).is_err());
        assert_eq!(fixture.runtime.current_revision(), before);
        assert_eq!(fixture.column(&amount).constraint, None);
    }
}

#[test]
fn unsorted_duplicate_text_values_are_normalised() {
    let mut fixture = Fixture::new();
    let status = fixture.status();
    fixture
        .set(&status, list(&["Todo", "Doing", "Todo", "Done"]))
        .unwrap();
    assert_eq!(
        fixture.column(&status).constraint,
        Some(list_values(&["Doing", "Done", "Todo"]))
    );
}

#[test]
fn invalid_declarations_are_typed_and_publish_nothing() {
    let mut fixture = Fixture::new();
    let (status, amount) = (fixture.status(), fixture.amount());
    let before = fixture.runtime.current_revision().to_owned();
    let declarations = [
        (amount.as_str(), range(10.0, 5.0), "max"),
        (status.as_str(), list(&[]), "at least one value"),
        (status.as_str(), range(0.0, 1.0), "Number columns"),
        (amount.as_str(), list(&["a"]), "Text columns"),
    ];
    for (field, declaration, expected) in declarations {
        match fixture.set(field, declaration).unwrap_err() {
            DesignerError::FieldConstraintViolation {
                reason,
                offending_rows,
                message,
                ..
            } => {
                assert_eq!(reason, ConstraintFailureReason::Declaration);
                assert_eq!(offending_rows, 0);
                assert!(message.contains(expected), "{message}");
            }
            other => panic!("unexpected declaration refusal {other:?}"),
        }
        // A refused declaration publishes nothing, including its revision.
        assert_eq!(fixture.runtime.current_revision(), before);
        assert_eq!(fixture.column(field).constraint, None);
    }
    let failure = DesignerError::FieldConstraintViolation {
        schema: fixture.collection.as_str().into(),
        field: status.as_str().into(),
        constraint: list_values(&["a"]),
        offending_rows: 0,
        reason: ConstraintFailureReason::Declaration,
        message: "message".into(),
    }
    .failure_projection(&before);
    assert_eq!(failure.code, "invalid_constraint_declaration");
    assert_eq!(
        failure
            .constraint_failure
            .map(|structured| structured.reason),
        Some(ConstraintFailureReason::Declaration)
    );
}

#[test]
fn constrained_documents_export_only_to_v3_and_reopen_equally() {
    let fixture = Fixture::new();
    let document = fixture.runtime.session.export_snapshot().into_document();
    let collection = fixture.collection.clone();
    let (status, amount) = (fixture.status(), fixture.amount());
    // Re-admit the unconstrained document so the runtime keeps the non-v3
    // export profile; legacy export must refuse only because of the constraint.
    let runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let row = runtime.query_table(&collection).unwrap().rows[0].id.clone();
    let mut non_v3 = Fixture {
        runtime,
        collection,
        status,
        amount,
        row,
    };
    non_v3.set(&non_v3.status(), list(&LITERALS)).unwrap();
    non_v3.set(&non_v3.amount(), range(0.0, 100.0)).unwrap();

    let revision = non_v3.runtime.current_revision().to_owned();
    match non_v3.runtime.export_project(&revision).unwrap_err() {
        DesignerError::LegacyExportUnsupported { .. } => {}
        other => panic!("unexpected legacy export result {other:?}"),
    }
    assert_eq!(
        non_v3
            .runtime
            .export_project(&revision)
            .unwrap_err()
            .failure_projection(&revision)
            .code,
        "legacy_export_unsupported"
    );

    let export = non_v3.runtime.export_project_v3(&revision).unwrap();
    let expected = non_v3.runtime.query_table(&non_v3.collection).unwrap();
    let mut reopened = None;
    open_project(&mut reopened, &export.bytes, OCCURRENCE).unwrap();
    let mut reopened = reopened.unwrap();
    let actual = reopened.query_table(&non_v3.collection).unwrap();
    // Reopening starts a fresh resident revision but must preserve every
    // declared constraint and every stored value.
    assert_eq!(actual.columns, expected.columns);
    assert_eq!(actual.rows, expected.rows);

    // The reopened constrained document keeps enforcing its declaration.
    let target = FieldTarget {
        entity: non_v3.row.clone(),
        field: non_v3.status.clone(),
    };
    let revision = reopened.current_revision().to_owned();
    let refusal = reopened
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision.clone(),
            target: target.clone(),
            input: ScalarEditInput::Text {
                value: "Blocked".into(),
            },
        })
        .unwrap_err();
    assert!(matches!(
        refusal,
        DesignerError::FieldConstraintViolation { .. }
    ));
    assert_eq!(reopened.current_revision(), revision);
}

#[test]
fn a_formula_outside_the_column_range_is_a_typed_constraint_refusal() {
    let mut fixture = Fixture::new();
    let amount = fixture.amount.clone();
    fixture.set(&amount, range(0.0, 100.0)).unwrap();
    let before = fixture.runtime.current_revision().to_owned();
    let target = FieldTarget {
        entity: fixture.row.clone(),
        field: amount.clone(),
    };
    let error = fixture
        .runtime
        .update_formula(&before, &target, "101")
        .unwrap_err();
    match error {
        DesignerError::FieldConstraintViolation {
            field,
            constraint,
            offending_rows,
            reason,
            ..
        } => {
            assert_eq!(field.as_str(), amount);
            assert_eq!(
                constraint,
                FieldConstraintProjection::NumberInclusiveRange {
                    min: 0.0,
                    max: 100.0
                }
            );
            assert_eq!(offending_rows, 1);
            assert_eq!(reason, ConstraintFailureReason::FieldValue);
        }
        other => panic!("unexpected refusal {other:?}"),
    }
    assert_eq!(fixture.runtime.current_revision(), before);
}
