#[path = "../fixtures/product_gaps.rs"]
mod product_gaps;

use std::collections::BTreeMap;

use tachiko_designer_runtime::{DesignerRequest, DesignerResponse, DesignerRuntime};
use tachiko_storage::{decode_roproj_v1, encode_roproj_v1, read_canonical_roproj};
use tachiko_workspace_engine::{
    Document, FieldAddress, FieldType, Number, Value, calculate_fields, validation_report,
};

const PRODUCT_GAP_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000219";

#[test]
fn product_gap_fixture_is_deterministic_workspace_valid_evidence() {
    let document = product_gaps::document();
    let checked_in = read_canonical_roproj(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../dogfood/product-gaps.roproj"),
    )
    .expect("the repository-owned Product Gap project must be exact canonical .roproj/v1");

    assert_eq!(document, product_gaps::document());
    assert_eq!(
        checked_in,
        encode_roproj_v1(&document)
            .expect("the deterministic semantic constructor must encode canonically")
    );
    assert_eq!(
        decode_roproj_v1(&checked_in).expect("the checked-in project must decode semantically"),
        document
    );

    assert_product_gap_schema(&document);
    assert_product_gap_content(&document);
    assert_product_gap_priorities(&document);
}

fn assert_product_gap_schema(document: &Document) {
    assert_eq!(document.id.as_str(), "ba30fc0a-5b11-4dbf-9ef3-76f904315a4d");
    assert_eq!(document.title, "Tachiko Work Product Gaps");
    assert!(validation_report(document).is_valid());

    let schemas = document.schemas.values().collect::<Vec<_>>();
    let [schema] = schemas.as_slice() else {
        panic!("fixture should contain exactly one schema");
    };
    assert_eq!(schema.id.as_str(), "d8b3db6e-a2ca-48f1-82f5-4e44630418dc");
    assert_eq!(schema.key.as_str(), "product_gaps");
    assert_ne!(schema.id.as_str(), schema.key.as_str());

    let field_types = schema
        .fields
        .values()
        .map(|field| (field.key.as_str(), (&field.field_type, field.required)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        field_types,
        BTreeMap::from([
            ("area", (&FieldType::Text, true)),
            ("confirmed", (&FieldType::Boolean, true)),
            ("friction", (&FieldType::Number, true)),
            ("github_issue", (&FieldType::Text, true)),
            ("impact", (&FieldType::Number, true)),
            ("priority", (&FieldType::Number, true)),
            ("title", (&FieldType::Text, true)),
        ])
    );
    assert!(schema.fields.values().all(|field| {
        field.id.as_str() != field.key.as_str()
            && field.id.as_str().contains('-')
            && !field.id.as_str().contains(field.key.as_str())
    }));
}

fn assert_product_gap_content(document: &Document) {
    let schema = document
        .schemas
        .values()
        .next()
        .expect("fixture should contain a schema");
    let entity_keys = document
        .entities
        .values()
        .map(|entity| entity.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        entity_keys,
        [
            "designer_profile_bound",
            "schema_authoring_missing",
            "browser_save_as_only",
        ]
    );
    assert!(document.entities.values().all(|entity| {
        entity.id.as_str() != entity.key.as_str()
            && entity.id.as_str().contains('-')
            && !entity.id.as_str().contains(entity.key.as_str())
    }));

    let expected_content = BTreeMap::from([
        (
            "designer_profile_bound",
            (
                "Designer admission was bound to Moonfall",
                "Designer",
                5.0,
                5.0,
            ),
        ),
        (
            "schema_authoring_missing",
            (
                "Schema and field authoring is not exposed",
                "Authoring",
                5.0,
                4.0,
            ),
        ),
        (
            "browser_save_as_only",
            (
                "Browser persistence is create-only Save As",
                "Persistence",
                4.0,
                4.0,
            ),
        ),
    ]);
    for entity in document.entities.values() {
        let expected = expected_content[entity.key.as_str()];
        let fields = schema
            .fields
            .values()
            .map(|field| (field.key.as_str(), &entity.fields[&field.id]))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(fields["title"], &Value::Text(expected.0.to_owned()));
        assert_eq!(fields["area"], &Value::Text(expected.1.to_owned()));
        assert_eq!(
            fields["impact"],
            &Value::Number(Number::new(expected.2).unwrap())
        );
        assert_eq!(
            fields["friction"],
            &Value::Number(Number::new(expected.3).unwrap())
        );
        assert!(matches!(fields["priority"], Value::Formula(_)));
        assert_eq!(fields["confirmed"], &Value::Boolean(true));
        assert_eq!(
            fields["github_issue"],
            &Value::Text("https://github.com/nurockplayer/tachiko-work/issues/219".to_owned())
        );
    }
}

fn assert_product_gap_priorities(document: &Document) {
    let priorities = calculate_fields(document)
        .expect("fixture should calculate through workspace authority")
        .into_iter()
        .filter(|field| field.address.field.as_str() == "priority")
        .map(|field| (field.address, field.value))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        priorities,
        BTreeMap::from([
            (
                FieldAddress::new("browser_save_as_only", "priority"),
                Number::new(8.0).unwrap(),
            ),
            (
                FieldAddress::new("designer_profile_bound", "priority"),
                Number::new(10.0).unwrap(),
            ),
            (
                FieldAddress::new("schema_authoring_missing", "priority"),
                Number::new(9.0).unwrap(),
            ),
        ])
    );
}

#[test]
fn canonical_product_gap_project_is_admitted_without_moonfall_subjects() {
    let document = product_gaps::document();
    assert!(
        document
            .schemas
            .values()
            .all(|schema| schema.key.as_str() != "weapons")
    );
    assert!(
        document
            .entities
            .values()
            .all(|entity| entity.key.as_str() != "shop")
    );

    let mut runtime = DesignerRuntime::from_document(document, PRODUCT_GAP_OCCURRENCE)
        .expect("the canonical ordinary project should fit the bounded Designer profile");
    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: PRODUCT_GAP_OCCURRENCE.to_owned(),
        })
        .expect("Product Gap bootstrap should succeed")
    else {
        panic!("expected bootstrap response");
    };
    assert_eq!(bootstrap.title, "Tachiko Work Product Gaps");
    assert_eq!(bootstrap.default_collection, "product_gaps");
    assert_eq!(bootstrap.collections.len(), 1);

    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: bootstrap.default_collection,
        })
        .expect("Product Gap table should be queryable")
    else {
        panic!("expected table response");
    };
    assert_eq!(table.collection.key, "product_gaps");
    assert_eq!(table.rows.len(), 3);
    assert!(table.columns.iter().any(|column| column.key == "priority"));
    assert!(table.rows.iter().all(|row| {
        row.fields.iter().any(|field| {
            field.formula.is_some()
                && field
                    .calculated
                    .as_ref()
                    .and_then(tachiko_designer_runtime::CalculationProjection::number)
                    .is_some()
        })
    }));
}
