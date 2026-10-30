//! Work #488 item (e): one closure test that saves a table carrying every
//! supported v3 feature, reopens it in a fresh runtime and proves the reopened
//! meaning is equal — Dates, explicit display names, required and optional
//! fields, blank cells, formulas, a duplicated table and a saved
//! keyed-grouped-sum definition — then does the same through a portable `.ro`.
//! The document also carries a Text literal-set and a Number range
//! constraint, which v3 and the portable `.ro` must keep; the legacy writer
//! refuses them instead of dropping them. The reopened project must also leave
//! as CSV/XLSX with truthful disclosure.

use std::collections::BTreeMap;
use tachiko_designer_runtime::interop_adapter::{FidelityCategory, SourceValue, SourceWorkbook};
use tachiko_designer_runtime::{
    BootstrapProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    FieldConstraintInput, FieldConstraintProjection, KeyedGroupedSumProjection, ScalarEditInput,
    StoredValueProjection, TableProjection, open_portable_ro, open_project, verify_portable_ro,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000700";
const REOPENED: &str = "00000000-0000-4000-8000-000000000701";
const PORTABLE: &str = "00000000-0000-4000-8000-000000000702";
const DEFINITION: &str = "revenue-by-category";

fn field(id: &str, name: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: Some(name.to_owned()),
        field_type,
        required,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn product_total(order: &str) -> Value {
    Value::Formula(Expression::Multiply {
        left: Box::new(Expression::Reference(FieldRef::new(order, "quantity"))),
        right: Box::new(Expression::Reference(FieldRef::new(order, "unit_price"))),
    })
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("fixture dates are valid"))
}

fn entity(
    document: &mut Document,
    schema: &SchemaId,
    id: &str,
    fields: Vec<(&str, Value)>,
) -> EntityId {
    let entity_id = EntityId::from(id);
    document.entities.insert(
        entity_id.clone(),
        Entity {
            id: entity_id.clone(),
            key: EntityKey::from(id),
            schema: schema.clone(),
            fields: fields
                .into_iter()
                .map(|(field, value)| (FieldId::from(field), value))
                .collect(),
        },
    );
    entity_id
}

fn schema(document: &mut Document, id: &str, name: &str, fields: Vec<FieldDefinition>) -> SchemaId {
    let schema_id = SchemaId::from(id);
    document.schemas.insert(
        schema_id.clone(),
        Schema {
            field_order: None,
            id: schema_id.clone(),
            key: SchemaKey::from(id),
            display_name: Some(name.to_owned()),
            fields: fields
                .into_iter()
                .map(|field| (field.id.clone(), field))
                .collect(),
        },
    );
    schema_id
}

fn constrained(mut field: FieldDefinition, constraint: FieldConstraint) -> FieldDefinition {
    field.constraint = constraint;
    field
}

fn category_values() -> Vec<String> {
    vec!["Paper".to_owned(), "Stationery".to_owned()]
}

fn price_range() -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: Number::new(0.0).expect("fixture bounds are finite"),
        max: Number::new(1000.0).expect("fixture bounds are finite"),
    }
}

/// Orders columns: every scalar type, required and optional, and both
/// constraint shapes.
fn order_fields() -> Vec<FieldDefinition> {
    vec![
        field("item", "Item", FieldType::Text, true),
        constrained(
            field("category", "分類 Category", FieldType::Text, true),
            FieldConstraint::TextLiteralSet {
                values: category_values(),
            },
        ),
        field("quantity", "Quantity", FieldType::Number, true),
        constrained(
            field("unit_price", "Unit price", FieldType::Number, true),
            price_range(),
        ),
        field("total", "Total", FieldType::Number, false),
        field("ordered_on", "Ordered on", FieldType::Date, true),
        field("note", "Note", FieldType::Text, false),
        field("shipped", "Shipped", FieldType::Boolean, false),
        field("paid", "Paid", FieldType::Boolean, true),
        field("delivered_on", "Delivered on", FieldType::Date, false),
    ]
}

/// The feature matrix of item (e) in one document: unicode display names,
/// required/optional columns, a present-empty Text versus an absent one, Date,
/// a per-row formula, a Text literal-set and a Number range constraint, a
/// duplicated schema pair for the grouped sum, and one saved definition whose
/// product-side category and price differ from the order-side columns.
fn kitchen_sink_document() -> Document {
    let mut document = Document::empty("kitchen_sink", "Kitchen sink 全部");
    let orders = schema(&mut document, "orders", "Orders 注文", order_fields());
    for (id, item, quantity, price, day, note, shipped) in [
        (
            "order-pen",
            "PEN",
            4.0,
            3.0,
            "2026-09-28",
            Some(Value::Text("urgent".to_owned())),
            Some(Value::Boolean(true)),
        ),
        ("order-note", "NOTE", 5.0, 4.0, "2026-09-27", None, None),
        (
            "order-spare",
            "PEN",
            2.0,
            3.0,
            "2026-09-26",
            Some(Value::Text(String::new())),
            None,
        ),
    ] {
        let mut fields = vec![
            ("item", Value::Text(item.to_owned())),
            (
                "category",
                // Deliberately unlike the product-side category, so a grouped
                // sum that read the orders column would give other groups.
                Value::Text(if item == "PEN" { "Paper" } else { "Stationery" }.to_owned()),
            ),
            ("quantity", number(quantity)),
            ("unit_price", number(price)),
            ("total", product_total(id)),
            ("ordered_on", date(day)),
            ("paid", Value::Boolean(id != "order-note")),
        ];
        fields.extend(note.map(|note| ("note", note)));
        fields.extend(shipped.map(|shipped| ("shipped", shipped)));
        if id == "order-pen" {
            fields.push(("delivered_on", date("2026-09-30")));
        }
        entity(&mut document, &orders, id, fields);
    }

    let products = schema(
        &mut document,
        "products",
        "Products 商品",
        vec![
            field("product_key", "Product key", FieldType::Text, true),
            field(
                "product_category",
                "Product category",
                FieldType::Text,
                true,
            ),
            field("product_price", "Product price", FieldType::Number, true),
        ],
    );
    for (id, key, category, price) in [
        ("product-pen", "PEN", "Stationery", 5.0),
        ("product-note", "NOTE", "Paper", 7.0),
    ] {
        entity(
            &mut document,
            &products,
            id,
            vec![
                ("product_key", Value::Text(key.to_owned())),
                ("product_category", Value::Text(category.to_owned())),
                ("product_price", number(price)),
            ],
        );
    }

    document.keyed_grouped_sum_definitions.insert(
        KeyedGroupedSumDefinitionId::from(DEFINITION),
        KeyedGroupedSumDefinition {
            id: KeyedGroupedSumDefinitionId::from(DEFINITION),
            orders: KeyedGroupedSumOrdersBinding {
                schema: orders,
                lookup_key_field: FieldId::from("item"),
                quantity_field: FieldId::from("quantity"),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: products,
                key_field: FieldId::from("product_key"),
                category_field: FieldId::from("product_category"),
                price_field: FieldId::from("product_price"),
            },
        },
    );
    document
}

fn handle(runtime: &mut DesignerRuntime, request: DesignerRequest) -> DesignerResponse {
    runtime
        .handle(request)
        .expect("the fixture request is accepted")
}

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = handle(
        runtime,
        DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        },
    ) else {
        panic!("expected a table projection");
    };
    table
}

/// Revision spellings are per-occurrence history, not persisted meaning.
fn normalized(mut table: TableProjection) -> TableProjection {
    table.revision = String::from("normalized");
    table
}

fn bootstrap(runtime: &mut DesignerRuntime, occurrence: &str) -> BootstrapProjection {
    let DesignerResponse::Bootstrap(mut bootstrap) = handle(
        runtime,
        DesignerRequest::Bootstrap {
            occurrence_id: occurrence.to_owned(),
        },
    ) else {
        panic!("expected a bootstrap projection");
    };
    bootstrap.revision = String::from("normalized");
    bootstrap
}

fn grouped_sum(runtime: &mut DesignerRuntime) -> KeyedGroupedSumProjection {
    let DesignerResponse::KeyedGroupedSum(mut sum) = handle(
        runtime,
        DesignerRequest::QueryKeyedGroupedSum {
            definition_id: DEFINITION.to_owned(),
        },
    ) else {
        panic!("expected a keyed grouped-sum projection");
    };
    sum.revision = String::from("normalized");
    sum
}

fn cell<'a>(
    table: &'a TableProjection,
    entity: &str,
    field: &str,
) -> &'a tachiko_designer_runtime::FieldProjection {
    let row = table
        .rows
        .iter()
        .find(|row| row.id == entity)
        .unwrap_or_else(|| panic!("no row '{entity}'"));
    row.fields
        .iter()
        .find(|candidate| candidate.target.field == field)
        .unwrap_or_else(|| panic!("row '{entity}' has no '{field}' value"))
}

fn calculated(field: &tachiko_designer_runtime::FieldProjection) -> Option<f64> {
    field
        .calculated
        .as_ref()
        .and_then(tachiko_designer_runtime::CalculationProjection::number)
}

fn duplicate(runtime: &mut DesignerRuntime, expected: &str) -> String {
    let DesignerResponse::Duplicated(duplicated) = handle(
        runtime,
        DesignerRequest::DuplicateCollection {
            expected_revision: expected.to_owned(),
            collection: "orders".to_owned(),
            name: "Orders copy".to_owned(),
 column_names: BTreeMap::new(),
        },
    ) else {
        panic!("expected a duplicate projection");
    };
    assert_eq!(duplicated.publication.resulting_revision, "resident/1");
    assert_ne!(duplicated.collection.key, "orders");
    duplicated.collection.key
}

fn open_saved(bytes: &[u8], occurrence: &str) -> DesignerRuntime {
    let mut reopened = None;
    open_project(&mut reopened, bytes, occurrence).expect("the saved v3 project reopens");
    reopened.expect("a successful open installs the occurrence")
}

fn column_constraint(table: &TableProjection, column: &str) -> Option<FieldConstraintProjection> {
    table
        .columns
        .iter()
        .find(|candidate| candidate.id == column)
        .unwrap_or_else(|| panic!("no column '{column}'"))
        .constraint
        .clone()
}

/// The duplicated collection carries the original declarations, checked
/// against the fixture rather than against another projection of the copy.
fn assert_copy_keeps_declarations(runtime: &mut DesignerRuntime, copy_key: &str) {
    let declared = |table: &TableProjection| {
        table
            .columns
            .iter()
            .map(|column| {
                (
                    column.display_name.clone(),
                    column.required,
                    column.constraint.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    let expected = order_fields()
        .into_iter()
        .map(|field| {
            (
                field.display_name.expect("fixture names are explicit"),
                field.required,
                constraint_projection(&field.constraint),
            )
        })
        .collect::<Vec<_>>();
    let mut copy = declared(&table(runtime, copy_key));
    let mut original = declared(&table(runtime, "orders"));
    let mut expected = expected;
    for list in [&mut copy, &mut original, &mut expected] {
        list.sort_by(|left, right| left.0.cmp(&right.0));
    }
    assert_eq!(original, expected, "the reopened orders match the fixture");
    assert_eq!(
        copy, expected,
        "the duplicate keeps requiredness and constraints"
    );
}

fn constraint_projection(constraint: &FieldConstraint) -> Option<FieldConstraintProjection> {
    match constraint {
        FieldConstraint::None => None,
        FieldConstraint::TextLiteralSet { values } => {
            Some(FieldConstraintProjection::TextLiteralSet {
                values: values.clone(),
            })
        }
        FieldConstraint::NumberInclusiveRange { min, max } => {
            Some(FieldConstraintProjection::NumberInclusiveRange {
                min: min.get(),
                max: max.get(),
            })
        }
    }
}

/// Spot-check the features that a scalar-only projection assertion could hide.
fn assert_spot_checked_meaning(runtime: &mut DesignerRuntime) {
    let orders = table(runtime, "orders");
    let ordered = cell(&orders, "order-note", "ordered_on");
    assert_eq!(
        ordered.stored,
        Some(StoredValueProjection::Date {
            value: Date::parse("2026-09-27").unwrap()
        })
    );
    let blank_row = orders
        .rows
        .iter()
        .find(|row| row.id == "order-note")
        .expect("the blank row reopens");
    let mut blank_fields = blank_row.blank_fields.clone();
    blank_fields.sort();
    assert_eq!(
        blank_fields,
        ["delivered_on", "note", "shipped"],
        "optional fields with no value stay absent, not placeholdered"
    );
    assert!(blank_row.fields.iter().all(|field| !matches!(
        field.target.field.as_str(),
        "note" | "shipped" | "delivered_on"
    )));
    assert_eq!(
        cell(&orders, "order-note", "paid").stored,
        Some(StoredValueProjection::Boolean { value: false }),
        "a required Boolean keeps a false value"
    );
    assert_eq!(
        cell(&orders, "order-pen", "delivered_on").stored,
        Some(StoredValueProjection::Date {
            value: Date::parse("2026-09-30").unwrap()
        }),
        "an optional Date keeps its value where present"
    );
    let empty_note = cell(&orders, "order-spare", "note");
    assert_eq!(
        empty_note.stored,
        Some(StoredValueProjection::Text {
            value: String::new()
        }),
        "a present empty Text stays distinguishable from an absent one"
    );
    assert_eq!(calculated(cell(&orders, "order-pen", "total")), Some(12.0));
    assert_eq!(
        column_constraint(&orders, "category"),
        Some(FieldConstraintProjection::TextLiteralSet {
            values: category_values()
        }),
        "a Text literal-set constraint survives with its values"
    );
    assert_eq!(
        column_constraint(&orders, "unit_price"),
        Some(FieldConstraintProjection::NumberInclusiveRange {
            min: 0.0,
            max: 1000.0
        }),
        "a Number range constraint survives with its bounds"
    );
    assert_eq!(column_constraint(&orders, "quantity"), None);
    assert_eq!(
        grouped_sum(runtime).groups,
        vec![
            tachiko_designer_runtime::KeyedGroupedSumGroupProjection {
                category: "Paper".to_owned(),
                value: 35.0,
            },
            tachiko_designer_runtime::KeyedGroupedSumGroupProjection {
                category: "Stationery".to_owned(),
                value: 30.0,
            },
        ]
    );
}

#[test]
fn v3_save_reopen_preserves_every_supported_feature_and_reexports_identically() {
    let source = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let mut source = source;
    let copy_key = duplicate(&mut source, "resident/0");
    let saved = source
        .export_project_v3("resident/1")
        .expect("the kitchen-sink document exports as v3");
    assert_eq!(saved.revision, "resident/1");

    let reopened = open_saved(&saved.bytes, REOPENED);
    let mut reopened = reopened;
    assert_eq!(
        bootstrap(&mut reopened, REOPENED),
        bootstrap(&mut source, OCCURRENCE),
        "bootstrap collections, title and saved definitions survive the round trip"
    );
    assert_eq!(
        bootstrap(&mut reopened, REOPENED).keyed_grouped_sum_definition_ids,
        vec![DEFINITION.to_owned()]
    );

    // Semantic equality per collection: stable IDs, keys, display names,
    // requiredness, presence/absence, Dates, formula sources and calculated
    // values are all carried by the projection itself.
    for collection in ["orders", "products", copy_key.as_str()] {
        assert_eq!(
            normalized(table(&mut reopened, collection)),
            normalized(table(&mut source, collection)),
            "collection '{collection}' reopens with identical meaning"
        );
    }
    assert_eq!(
        grouped_sum(&mut reopened),
        grouped_sum(&mut source),
        "the saved grouped-sum definition and its results survive"
    );

    assert_spot_checked_meaning(&mut reopened);
    assert_copy_keeps_declarations(&mut reopened, &copy_key);

    // Re-saving the reopened occurrence is byte-identical: the round trip
    // loses nothing that the canonical v3 encoding can express.
    assert_eq!(
        reopened
            .export_project_v3("resident/0")
            .expect("the reopened project exports again")
            .bytes,
        saved.bytes
    );

    // No stored result is trusted on reopen: an edited input recomputes.
    let quantity = cell(&table(&mut reopened, "orders"), "order-pen", "quantity")
        .target
        .clone();
    let DesignerResponse::Published(publication) = handle(
        &mut reopened,
        DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: quantity,
            input: ScalarEditInput::Number {
                input: "10".to_owned(),
            },
        },
    ) else {
        panic!("expected a publication");
    };
    assert_eq!(publication.resulting_revision, "resident/1");
    let recomputed = table(&mut reopened, "orders");
    assert_eq!(
        calculated(cell(&recomputed, "order-pen", "total")),
        Some(30.0)
    );
    assert_eq!(
        calculated(cell(&recomputed, "order-note", "total")),
        Some(20.0)
    );
}

#[test]
fn v3_saved_project_round_trips_through_a_portable_ro() {
    let source = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let mut source = source;
    let copy_key = duplicate(&mut source, "resident/0");
    let saved = source
        .export_project_v3("resident/1")
        .expect("the kitchen-sink document exports as v3");
    let reopened = open_saved(&saved.bytes, REOPENED);
    let mut reopened = reopened;

    let portable = reopened
        .export_portable_ro("resident/0")
        .expect("Date, formulas and saved definitions export as portable .ro");
    verify_portable_ro(&portable.bytes).expect("the portable artifact verifies");

    let mut portable_runtime = None;
    open_portable_ro(&mut portable_runtime, &portable.bytes, PORTABLE)
        .expect("the portable artifact opens");
    let mut portable_runtime = portable_runtime.expect("a successful open installs the occurrence");

    assert_eq!(
        bootstrap(&mut portable_runtime, PORTABLE),
        bootstrap(&mut reopened, REOPENED)
    );
    for collection in ["orders", "products", copy_key.as_str()] {
        assert_eq!(
            normalized(table(&mut portable_runtime, collection)),
            normalized(table(&mut reopened, collection)),
            "collection '{collection}' survives the portable artifact"
        );
    }
    assert_eq!(
        grouped_sum(&mut portable_runtime),
        grouped_sum(&mut reopened)
    );
    let portable_saved = portable_runtime
        .export_project_v3("resident/0")
        .expect("the portable-opened project exports as v3");
    assert_eq!(
        portable_saved.bytes, saved.bytes,
        "the portable artifact and the v3 project carry the same document"
    );
}

/// Formula cells in the exported workbook, as the computed Number each
/// `Total` column carries. Two collections (orders and its copy) have one.
fn formula_totals(workbook: &SourceWorkbook) -> Vec<Vec<f64>> {
    workbook
        .sheets
        .iter()
        .filter_map(|sheet| {
            let index = sheet
                .columns
                .iter()
                .position(|column| column.name == "Total")?;
            Some(
                sheet
                    .rows
                    .iter()
                    .map(|row| match &row[index].value {
                        SourceValue::Number { value } => *value,
                        other => panic!("a computed formula must export as Number, got {other:?}"),
                    })
                    .collect(),
            )
        })
        .collect()
}

fn sorted(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    values
}

fn disclosed_codes(workbook: &SourceWorkbook, code: &str) -> usize {
    workbook
        .ledger
        .iter()
        .filter(|finding| {
            finding.code == code && finding.category == FidelityCategory::LossyOnExport
        })
        .count()
}

#[test]
fn reopened_kitchen_sink_exports_formula_values_to_csv_and_xlsx() {
    let mut source = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let copy_key = duplicate(&mut source, "resident/0");
    let saved = source
        .export_project_v3("resident/1")
        .expect("the kitchen-sink document exports as v3");
    let mut reopened = open_saved(&saved.bytes, REOPENED);

    let workbook = reopened
        .export_native_tables_workbook("resident/0", "orders")
        .expect("the reopened project exports as XLSX");
    assert_eq!(
        formula_totals(&workbook)
            .into_iter()
            .map(sorted)
            .collect::<Vec<_>>(),
        vec![vec![6.0, 12.0, 20.0]; 2],
        "formula cells export their current computed value, not a blank or a source"
    );
    assert_eq!(
        disclosed_codes(&workbook, "native_tables_formula_values_only"),
        2,
        "both formula-bearing collections disclose that definitions stay behind"
    );
    assert_eq!(
        disclosed_codes(&workbook, "native_tables_saved_grouped_sum_excluded"),
        1
    );
    assert!(
        !workbook.ledger.iter().any(|finding| finding.blocking),
        "{:?}",
        workbook.ledger
    );
    assert_eq!(workbook.sheets.len(), 3);
    assert!(
        workbook
            .sheets
            .iter()
            .any(|sheet| sheet.name == "Orders copy"),
        "the duplicated collection {copy_key} leaves as its own sheet"
    );

    // CSV keeps the same computed values for the selected collection only.
    let csv = reopened
        .export_native_tables_csv_workbook("resident/0", "orders")
        .expect("the reopened formula table exports as CSV");
    assert_eq!(csv.sheets.len(), 1);
    assert_eq!(
        sorted(formula_totals(&csv).remove(0)),
        vec![6.0, 12.0, 20.0]
    );
    assert_eq!(
        disclosed_codes(&csv, "native_tables_csv_collection_excluded"),
        2,
        "CSV excludes the other two collections by name"
    );
    assert_eq!(
        disclosed_codes(&csv, "native_tables_formula_values_only"),
        1,
        "CSV discloses that the selected formulas leave as values"
    );
    assert_eq!(
        disclosed_codes(&csv, "native_tables_field_constraint_excluded"),
        2,
        "CSV discloses both constraints of the selected collection"
    );

    // Export is read-only: the reopened occurrence keeps its revision and its
    // formulas, and still saves back to the identical v3 bytes.
    assert_eq!(reopened.observe_occurrence().revision, "resident/0");
    assert_eq!(
        calculated(cell(&table(&mut reopened, "orders"), "order-pen", "total")),
        Some(12.0)
    );
    assert_eq!(
        reopened
            .export_project_v3("resident/0")
            .expect("the reopened project still exports")
            .bytes,
        saved.bytes
    );
}

fn text_column(workbook: &SourceWorkbook, column_name: &str) -> Vec<String> {
    let sheet = workbook
        .sheets
        .iter()
        .find(|sheet| {
            sheet
                .columns
                .iter()
                .any(|column| column.name == column_name)
        })
        .unwrap_or_else(|| panic!("no sheet carries '{column_name}'"));
    let index = sheet
        .columns
        .iter()
        .position(|column| column.name == column_name)
        .expect("the column was just found");
    sheet
        .rows
        .iter()
        .map(|row| match &row[index].value {
            SourceValue::Text { value } => value.clone(),
            other => panic!("'{column_name}' must export as Text, got {other:?}"),
        })
        .collect()
}

#[test]
fn constrained_columns_export_with_disclosure_and_refuse_only_the_legacy_writer() {
    let mut runtime = open_saved(
        &DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
            .expect("the kitchen-sink document is admissible")
            .export_project_v3("resident/0")
            .expect("constraints are stored in .roproj/v3")
            .bytes,
        REOPENED,
    );

    let workbook = runtime
        .export_native_tables_workbook("resident/0", "orders")
        .expect("constrained columns export their values");
    let disclosures = workbook
        .ledger
        .iter()
        .filter(|finding| finding.code == "native_tables_field_constraint_excluded")
        .map(|finding| finding.message.as_str())
        .collect::<Vec<_>>();
    for requirement in [
        "allowed values: Paper, Stationery",
        "allowed range: 0 to 1000",
    ] {
        assert!(
            disclosures
                .iter()
                .any(|message| message.contains(requirement)),
            "{requirement}: {disclosures:?}"
        );
    }
    assert_eq!(
        sorted_text(text_column(&workbook, "分類 Category")),
        vec!["Paper", "Paper", "Stationery"],
        "a constrained column exports its values unchanged"
    );
    let csv = runtime
        .export_native_tables_csv_workbook("resident/0", "orders")
        .expect("CSV still exports the constrained collection");
    assert_eq!(
        sorted_text(text_column(&csv, "分類 Category")),
        vec!["Paper", "Paper", "Stationery"]
    );

    // Only the legacy writer has no representation for the rules, so it
    // refuses instead of silently dropping them.
    let error = runtime
        .export_project("resident/0")
        .expect_err("the legacy writer cannot carry constraints");
    assert!(
        matches!(error, DesignerError::LegacyExportUnsupported { .. }),
        "{error}"
    );
    assert_eq!(
        error.failure_projection("resident/0").code,
        "legacy_export_unsupported"
    );

    // The reopened rules are enforced, not merely projected.
    let category = cell(&table(&mut runtime, "orders"), "order-pen", "category")
        .target
        .clone();
    let refused = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: category,
            input: ScalarEditInput::Text {
                value: "Food".to_owned(),
            },
        })
        .expect_err("a value outside the reopened literal set is refused");
    assert_eq!(
        refused.failure_projection("resident/0").code,
        "field_constraint_violation"
    );
    assert_eq!(runtime.observe_occurrence().revision, "resident/0");
    assert_range_enforced(&mut runtime, "orders", "order-pen", "unit_price", 1000.0);
}

/// Inclusive bounds: one past the maximum is refused without publication,
/// the maximum itself publishes.
fn assert_range_enforced(
    runtime: &mut DesignerRuntime,
    collection: &str,
    row: &str,
    field: &str,
    max: f64,
) {
    let target = cell(&table(runtime, collection), row, field).target.clone();
    let revision = runtime.observe_occurrence().revision;
    let edit = |runtime: &mut DesignerRuntime, value: f64| {
        runtime.handle(DesignerRequest::EditScalar {
            expected_revision: revision.clone(),
            target: target.clone(),
            input: ScalarEditInput::Number {
                input: value.to_string(),
            },
        })
    };
    let refused = edit(runtime, max + 1.0).expect_err("a value above the range is refused");
    assert_eq!(
        refused.failure_projection(&revision).code,
        "field_constraint_violation"
    );
    assert_eq!(runtime.observe_occurrence().revision, revision);
    assert!(
        matches!(edit(runtime, max), Ok(DesignerResponse::Published(_))),
        "the inclusive maximum is accepted"
    );
}

#[test]
fn authored_constraint_round_trips_through_v3_and_portable_ro() {
    let mut runtime = DesignerRuntime::from_document(kitchen_sink_document(), OCCURRENCE)
        .expect("the kitchen-sink document is admissible");
    let DesignerResponse::Published(publication) = handle(
        &mut runtime,
        DesignerRequest::SetColumnConstraint {
            expected_revision: "resident/0".to_owned(),
            collection: "products".to_owned(),
            field: "product_price".to_owned(),
            constraint: FieldConstraintInput::NumberInclusiveRange {
                min: 1.0,
                max: 50.0,
            },
        },
    ) else {
        panic!("expected a publication");
    };
    assert_eq!(publication.resulting_revision, "resident/1");
    let authored = table(&mut runtime, "products");
    let expected = Some(FieldConstraintProjection::NumberInclusiveRange {
        min: 1.0,
        max: 50.0,
    });
    assert_eq!(column_constraint(&authored, "product_price"), expected);

    let saved = runtime
        .export_project_v3("resident/1")
        .expect("an authored constraint saves as v3");
    let mut reopened = open_saved(&saved.bytes, REOPENED);
    assert_eq!(
        normalized(table(&mut reopened, "products")),
        normalized(authored),
        "the authored constraint reopens with identical meaning"
    );
    let portable = reopened
        .export_portable_ro("resident/0")
        .expect("an authored constraint exports as portable .ro");
    verify_portable_ro(&portable.bytes).expect("the portable artifact verifies");
    let mut portable_runtime = None;
    open_portable_ro(&mut portable_runtime, &portable.bytes, PORTABLE)
        .expect("the portable artifact opens");
    let mut portable_runtime = portable_runtime.expect("a successful open installs the occurrence");
    assert_eq!(
        column_constraint(&table(&mut portable_runtime, "products"), "product_price"),
        expected
    );
    assert_eq!(
        portable_runtime
            .export_project_v3("resident/0")
            .expect("the portable-opened project exports as v3")
            .bytes,
        saved.bytes
    );
    assert_range_enforced(
        &mut portable_runtime,
        "products",
        "product-pen",
        "product_price",
        50.0,
    );
}

fn sorted_text(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}
