use std::{collections::BTreeMap, panic::AssertUnwindSafe};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{from_bytes, from_str, to_canonical_string};

fn document(number: f64, title: String, text: String) -> Document {
    Document {
        id: DocumentId::from("audit-doc"),
        title,
        schemas: BTreeMap::from([(
            SchemaId::from("audit-schema"),
            Schema {
                id: SchemaId::from("audit-schema"),
                key: SchemaKey::from("audit-schema"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("number"),
                        FieldDefinition {
                            id: FieldId::from("number"),
                            key: FieldKey::from("number"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                    (
                        FieldId::from("text"),
                        FieldDefinition {
                            id: FieldId::from("text"),
                            key: FieldKey::from("text"),
                            field_type: FieldType::Text,
                            required: true,
                        },
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("audit-entity"),
            Entity {
                id: EntityId::from("audit-entity"),
                key: "audit-entity".into(),
                schema: SchemaId::from("audit-schema"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("number"),
                        Value::Number(Number::new(number).unwrap()),
                    ),
                    (FieldId::from("text"), Value::Text(text)),
                ]),
            },
        )]),
    }
}

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[test]
fn random_finite_binary64_values_round_trip_by_normalized_bits() {
    let mut state = 0x243f_6a88_85a3_08d3_u64;
    let mut checked = 0_usize;
    while checked < 50_000 {
        let value = f64::from_bits(next(&mut state));
        if !value.is_finite() {
            continue;
        }
        let expected = Number::new(value).unwrap();
        let document = document(value, "Number audit".to_owned(), String::new());
        let encoded = to_canonical_string(&document).unwrap();
        let decoded = from_str(&encoded).unwrap();
        let Value::Number(actual) = decoded.entities["audit-entity"].fields["number"] else {
            panic!("number field changed kind")
        };
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "input bits={:016x}",
            value.to_bits()
        );
        assert_eq!(to_canonical_string(&decoded).unwrap(), encoded);
        checked += 1;
    }
}

#[test]
fn generated_unicode_scalar_sequences_round_trip_without_normalization() {
    const SCALARS: &[char] = &[
        '\0',
        '\u{1}',
        '\n',
        '"',
        '\\',
        'a',
        'é',
        'e',
        '\u{301}',
        '漢',
        '𝄞',
        '🙂',
        '\u{2028}',
        '\u{10ffff}',
    ];
    let mut state = 0x1319_8a2e_0370_7344_u64;
    for case in 0..10_000 {
        let len = usize::try_from(next(&mut state) % 24).unwrap();
        let mut value = String::new();
        for _ in 0..len {
            let index =
                usize::try_from(next(&mut state) % u64::try_from(SCALARS.len()).unwrap()).unwrap();
            value.push(SCALARS[index]);
        }
        let title = format!("audit-{case}-{value}");
        let document = document(1.0, title.clone(), value.clone());
        let encoded = to_canonical_string(&document).unwrap();
        let decoded = from_str(&encoded).unwrap();
        assert_eq!(decoded.title, title);
        assert_eq!(
            decoded.entities["audit-entity"].fields["text"],
            Value::Text(value)
        );
        assert_eq!(to_canonical_string(&decoded).unwrap(), encoded);
    }
}

#[test]
fn deterministic_byte_mutations_never_panic_the_external_reader() {
    let seed = to_canonical_string(&document(
        f64::from_bits(1),
        "Cafe\u{301} 🙂".to_owned(),
        "quote \" slash \\ nul \0".to_owned(),
    ))
    .unwrap()
    .into_bytes();
    let mut state = 0xa409_3822_299f_31d0_u64;

    for case in 0..100_000 {
        let mut candidate = seed.clone();
        let edits = 1 + usize::try_from(next(&mut state) % 4).unwrap();
        for _ in 0..edits {
            let operation = next(&mut state) % 3;
            let position =
                usize::try_from(next(&mut state) % u64::try_from(candidate.len() + 1).unwrap())
                    .unwrap();
            let byte = next(&mut state).to_le_bytes()[0];
            match operation {
                0 => candidate.insert(position, byte),
                1 if !candidate.is_empty() => {
                    candidate.remove(position.min(candidate.len() - 1));
                }
                2 if !candidate.is_empty() => {
                    let index = position.min(candidate.len() - 1);
                    candidate[index] = byte;
                }
                _ => candidate.push(byte),
            }
        }

        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| from_bytes(&candidate)));
        assert!(outcome.is_ok(), "reader panicked for mutation case {case}");
    }
}
