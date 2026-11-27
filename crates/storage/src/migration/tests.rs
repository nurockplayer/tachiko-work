use super::{LEGACY_V1_NAMESPACE, migration_id};

#[test]
fn migration_namespace_and_canonical_input_vectors_are_frozen() {
    assert_eq!(
        LEGACY_V1_NAMESPACE.to_string(),
        "7a199010-e2db-5f4f-a216-07ddb708f5ef"
    );
    assert_eq!(
        migration_id("document\0legacy-doc"),
        "1213a728-1f70-5425-a330-20a8797f5e82"
    );
    assert_eq!(
        migration_id("schema\0legacy-doc\0source"),
        "ff71fea8-d907-5234-a6be-819f6e6fdf07"
    );
    assert_eq!(
        migration_id("field\0legacy-doc\0source\0calc"),
        "32c7bf4d-e5e4-5ea0-ab43-0d42c6878cce"
    );
    assert_eq!(
        migration_id("entity\0legacy-doc\0source-entity"),
        "1832624c-a6ad-55fb-b96a-8617af123e7f"
    );
}
