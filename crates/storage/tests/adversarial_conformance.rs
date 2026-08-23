use std::{collections::BTreeMap, panic::catch_unwind};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{FormatError, from_bytes, from_str, to_canonical_string};

fn number_document(number: Number) -> Document {
    let schema_id = SchemaId::from("schema");
    let field_id = FieldId::from("number");
    let entity_id = EntityId::from("entity");
    Document {
        id: DocumentId::from("document"),
        title: "Adversarial numeric conformance".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("schema"),
                fields: BTreeMap::from([(
                    field_id.clone(),
                    FieldDefinition {
                        id: field_id.clone(),
                        key: FieldKey::from("number"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("entity"),
                schema: schema_id,
                fields: BTreeMap::from([(field_id, Value::Number(number))]),
            },
        )]),
    }
}

fn number_source(token: &str) -> String {
    r#"{"format_version":2,"id":"document","title":"Adversarial numeric ingress","schemas":{"schema":{"id":"schema","key":"schema","fields":{"number":{"id":"number","key":"number","field_type":{"type":"number"},"required":true}}}},"entities":{"entity":{"id":"entity","key":"entity","schema":"schema","fields":{"number":{"kind":"number","value":NUMBER}}}}}"#
        .replace("NUMBER", token)
}

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn random_index(seed: &mut u64, length: usize) -> usize {
    let length = u64::try_from(length).unwrap();
    usize::try_from(next(seed) % length).unwrap()
}

#[test]
fn finite_binary64_values_survive_5000_canonical_round_trips_exactly() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut exercised = 0;
    while exercised < 5_000 {
        let bits = next(&mut seed);
        let value = f64::from_bits(bits);
        let Ok(number) = Number::new(value) else {
            continue;
        };
        let canonical = to_canonical_string(&number_document(number)).unwrap();
        let decoded = from_str(&canonical).unwrap();
        let Value::Number(actual) = decoded.entities["entity"].fields["number"] else {
            panic!("number field changed kind during round trip");
        };
        assert_eq!(actual.to_bits(), number.to_bits(), "input bits {bits:016x}");
        assert_eq!(to_canonical_string(&decoded).unwrap(), canonical);
        exercised += 1;
    }
}

#[test]
fn admitted_random_decimal_spellings_match_binary64_conversion_and_canonicalize() {
    let mut seed = 0xd1b5_4a32_d192_ed03_u64;
    for case in 0..5_000 {
        let sign = if next(&mut seed) & 1 == 0 { "" } else { "-" };
        let integer = next(&mut seed) % 1_000_000_000_000_000_000;
        let fraction = next(&mut seed) % 1_000_000_000;
        let exponent = (next(&mut seed) % 801) as i32 - 400;
        let token = format!("{sign}{integer}.{fraction:09}e{exponent:+}");
        let parsed = token.parse::<f64>().unwrap();
        match from_str(&number_source(&token)) {
            Ok(document) => {
                assert!(parsed.is_finite(), "case {case}: {token}");
                let expected = Number::new(parsed).unwrap();
                let Value::Number(actual) = document.entities["entity"].fields["number"] else {
                    panic!("number token changed kind");
                };
                assert_eq!(actual.to_bits(), expected.to_bits(), "case {case}: {token}");
                let canonical = to_canonical_string(&document).unwrap();
                assert_eq!(
                    to_canonical_string(&from_str(&canonical).unwrap()).unwrap(),
                    canonical
                );
            }
            Err(FormatError::InvalidRepresentation { .. }) => {
                assert!(
                    !parsed.is_finite(),
                    "finite token was rejected: case {case}: {token}"
                );
            }
            Err(error) => panic!("unexpected error for case {case} token {token}: {error:?}"),
        }
    }
}

#[test]
fn structured_and_mutated_external_inputs_never_unwind() {
    let canonical = to_canonical_string(&number_document(Number::new(1.0).unwrap())).unwrap();
    let alphabet = b"{}[],:\"\\u0123456789eE+-.truefalsenull abcdef";
    let mut seed = 0xa076_1d64_78bd_642f_u64;

    for case in 0..20_000 {
        let mut candidate = canonical.as_bytes().to_vec();
        for _ in 0..=next(&mut seed) % 8 {
            match next(&mut seed) % 4 {
                0 if !candidate.is_empty() => {
                    let index = random_index(&mut seed, candidate.len());
                    candidate[index] ^= 1 << (next(&mut seed) % 8);
                }
                1 => {
                    let index = random_index(&mut seed, candidate.len() + 1);
                    let byte = alphabet[random_index(&mut seed, alphabet.len())];
                    candidate.insert(index, byte);
                }
                2 if !candidate.is_empty() => {
                    let index = random_index(&mut seed, candidate.len());
                    candidate.remove(index);
                }
                _ if !candidate.is_empty() => {
                    let index = random_index(&mut seed, candidate.len());
                    candidate.insert(index, candidate[index]);
                }
                _ => {}
            }
        }
        assert!(
            catch_unwind(|| from_bytes(&candidate)).is_ok(),
            "external mutation {case} unwound"
        );
    }

    for depth in [63, 64, 65, 127, 128, 129, 256, 1_024] {
        let nested = format!(
            "{{\"format_version\":2,\"id\":\"document\",\"title\":\"x\",\"schemas\":{{}},\"entities\":{{}},\"unknown\":{}0{}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert!(
            catch_unwind(|| from_bytes(nested.as_bytes())).is_ok(),
            "nested external input at depth {depth} unwound"
        );
    }
}

#[test]
fn decoded_equivalent_member_names_are_rejected_for_non_ascii_and_control_escapes() {
    for source in [
        r#"{"format_version":2,"id":"document","title":"x","schemas":{},"\u0073chemas":{},"entities":{}}"#,
        "{\"format_version\":2,\"id\":\"document\",\"title\":\"x\",\"schemas\":{},\"entities\":{},\"𝄞\":1,\"\\uD834\\uDD1E\":2}",
        r#"{"format_version":2,"id":"document","title":"x","schemas":{},"entities":{},"\b":1,"\u0008":2}"#,
    ] {
        assert!(matches!(
            from_str(source),
            Err(FormatError::DuplicateMember { .. })
        ));
    }
}

#[test]
fn unicode_stable_ids_preserve_normalization_and_sort_by_utf8_bytes() {
    let decomposed = "e\u{301}";
    let composed = "é";
    let mut document = number_document(Number::new(1.0).unwrap());
    document.id = DocumentId::from(decomposed);
    let mut entity = document.entities.remove("entity").unwrap();
    entity.id = EntityId::from(decomposed);
    entity.key = "decomposed".into();
    let mut second = entity.clone();
    second.id = EntityId::from(composed);
    second.key = "composed".into();
    document.entities = BTreeMap::from([(entity.id.clone(), entity), (second.id.clone(), second)]);

    let canonical = to_canonical_string(&document).unwrap();
    let decoded = from_str(&canonical).unwrap();

    assert_eq!(decoded.id.as_str(), decomposed);
    assert!(decoded.entities.contains_key(decomposed));
    assert!(decoded.entities.contains_key(composed));
    assert!(
        canonical.find(&format!("\"{decomposed}\": {{")).unwrap()
            < canonical.find(&format!("\"{composed}\": {{")).unwrap()
    );
    assert_eq!(to_canonical_string(&decoded).unwrap(), canonical);
}
