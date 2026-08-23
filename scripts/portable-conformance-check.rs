//! Native/WASM conformance corpus for portable production semantics.

use std::collections::BTreeMap;

use tachiko_ai_api::{explain_formula, suggest_field_change};
use tachiko_formula_engine::{
    CalculationError, CanonicalAuthoringProjectionError, calculate, project_expression,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{from_str as storage_from_str, to_canonical_string};
use tachiko_workspace_engine::calculate_fields;

const CASE_COUNT: u32 = 26;
const VALUE: u32 = 0;
const DIVISION_BY_ZERO: u32 = 1;
const NON_FINITE: u32 = 2;
const CYCLE: u32 = 3;
const PROJECTION_FAILURE: u32 = 4;
const CORPUS_DIGEST: u32 = 5;
const NON_NUMERIC_REFERENCE: u32 = 6;
const UNEXPECTED: u32 = 255;

#[derive(Clone, Copy)]
struct Record {
    class: u32,
    bits: u64,
    auxiliary: u64,
}

impl Record {
    fn value(number: Number, auxiliary: u64) -> Self {
        Self {
            class: VALUE,
            bits: number.to_bits(),
            auxiliary,
        }
    }

    const fn failure(class: u32, auxiliary: u64) -> Self {
        Self {
            class,
            bits: 0,
            auxiliary,
        }
    }
}

fn number(value: f64) -> Number {
    Number::new(value).expect("conformance constants are finite")
}

fn numeric(value: f64) -> Expression {
    Expression::Number(number(value))
}

fn field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Number,
        required: true,
    }
}

fn formula_document(input: Number, expression: Expression) -> Document {
    let schema_id = SchemaId::from("schema-stable");
    let entity_id = EntityId::from("entity-stable");
    Document {
        id: DocumentId::from("document-stable"),
        title: "Portable conformance".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("numbers"),
                fields: BTreeMap::from([
                    (FieldId::from("input-stable"), field("input-stable")),
                    (FieldId::from("output-stable"), field("output-stable")),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("source"),
                schema: schema_id,
                fields: BTreeMap::from([
                    (FieldId::from("input-stable"), Value::Number(input)),
                    (FieldId::from("output-stable"), Value::Formula(expression)),
                ]),
            },
        )]),
    }
}

fn input_reference() -> Expression {
    Expression::Reference(FieldRef::new("entity-stable", "input-stable"))
}

fn calculated_record(document: &Document) -> Record {
    match calculate(document) {
        Ok(calculation) => {
            let output = FieldRef::new("entity-stable", "output-stable");
            let Some(value) = calculation.value(&output) else {
                return Record::failure(UNEXPECTED, 0);
            };
            let dependencies = calculation
                .dependencies_of(&output)
                .map_or(0, |dependencies| dependencies.len() as u64);
            Record::value(value, dependencies)
        }
        Err(CalculationError::DivisionByZero { .. }) => Record::failure(DIVISION_BY_ZERO, 0),
        Err(CalculationError::NonFiniteResult { .. }) => Record::failure(NON_FINITE, 0),
        Err(CalculationError::Cycle { path }) => Record::failure(CYCLE, path.len() as u64),
        Err(_) => Record::failure(UNEXPECTED, 0),
    }
}

fn storage_number_source(number: &str) -> String {
    r#"{"format_version":2,"id":"doc","title":"Portable storage conformance","schemas":{"schema":{"id":"schema","key":"schema","fields":{"number":{"id":"number","key":"number","field_type":{"type":"number"},"required":true}}}},"entities":{"entity":{"id":"entity","key":"entity","schema":"schema","fields":{"number":{"kind":"number","value":NUMBER}}}}}"#
        .replace("NUMBER", number)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn mix_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn next_random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn adversarial_numeric_corpus_record() -> Record {
    const SAMPLE_COUNT: u64 = 4_096;

    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut sampled = 0;
    while sampled < SAMPLE_COUNT {
        let bits = next_random(&mut seed);
        let Ok(input) = Number::new(f64::from_bits(bits)) else {
            continue;
        };
        let document = formula_document(
            input,
            Expression::Multiply {
                left: Box::new(input_reference()),
                right: Box::new(numeric(0.5)),
            },
        );
        let calculated = calculated_record(&document);
        let Ok(canonical) = to_canonical_string(&document) else {
            return Record::failure(UNEXPECTED, sampled);
        };

        mix_hash(&mut hash, &calculated.class.to_le_bytes());
        mix_hash(&mut hash, &calculated.bits.to_le_bytes());
        mix_hash(&mut hash, &calculated.auxiliary.to_le_bytes());
        mix_hash(&mut hash, canonical.as_bytes());
        sampled += 1;
    }

    Record {
        class: CORPUS_DIGEST,
        bits: hash,
        auxiliary: SAMPLE_COUNT,
    }
}

fn schema_type_change_record() -> Record {
    let mut document = formula_document(number(42.0), input_reference());
    let schema = document.schemas.get_mut("schema-stable").unwrap();
    schema
        .fields
        .get_mut("input-stable")
        .unwrap()
        .field_type = FieldType::Text;
    let mut output_definition = schema.fields.remove("output-stable").unwrap();
    output_definition.id = FieldId::from("a-output-stable");
    schema
        .fields
        .insert(output_definition.id.clone(), output_definition);
    let entity = document.entities.get_mut("entity-stable").unwrap();
    let output_value = entity.fields.remove("output-stable").unwrap();
    entity
        .fields
        .insert(FieldId::from("a-output-stable"), output_value);

    match calculate(&document) {
        Err(CalculationError::NonNumericReference { reference })
            if reference == FieldRef::new("entity-stable", "input-stable") =>
        {
            Record::failure(NON_NUMERIC_REFERENCE, 1)
        }
        _ => Record::failure(UNEXPECTED, 25),
    }
}

fn storage_record(input: &str, expected_bits: u64, expected_fingerprint: u64) -> Record {
    let Ok(document) = storage_from_str(&storage_number_source(input)) else {
        return Record::failure(UNEXPECTED, 1);
    };
    let Value::Number(value) = document.entities["entity"].fields["number"] else {
        return Record::failure(UNEXPECTED, 2);
    };
    let Ok(canonical) = to_canonical_string(&document) else {
        return Record::failure(UNEXPECTED, 3);
    };
    let fingerprint = fnv1a64(canonical.as_bytes());
    if value.to_bits() != expected_bits || fingerprint != expected_fingerprint {
        return Record::failure(UNEXPECTED, fingerprint);
    }
    Record::value(value, fingerprint)
}

fn workspace_calculation_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let Ok(fields) = calculate_fields(&document) else {
        return Record::failure(UNEXPECTED, 21);
    };
    let Some(output) = fields
        .iter()
        .find(|field| field.field == FieldRef::new("entity-stable", "output-stable"))
    else {
        return Record::failure(UNEXPECTED, 22);
    };
    if output.address.to_string() != "source.output-stable"
        || output.value != number(42.0)
        || fields.len() != 2
    {
        return Record::failure(UNEXPECTED, fields.len() as u64);
    }
    Record::value(output.value, fields.len() as u64)
}

fn ai_formula_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let Ok(explanation) = explain_formula(
        &document,
        &FieldRef::new("entity-stable", "output-stable"),
    ) else {
        return Record::failure(UNEXPECTED, 23);
    };
    if explanation.value != number(42.0) || explanation.dependencies.len() != 1 {
        return Record::failure(UNEXPECTED, explanation.dependencies.len() as u64);
    }
    Record::value(explanation.value, explanation.dependencies.len() as u64)
}

fn ai_suggestion_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let original = document.clone();
    let proposed = number(2.0);
    let Ok(suggestion) = suggest_field_change(
        &document,
        FieldRef::new("entity-stable", "input-stable"),
        Value::Number(proposed),
    ) else {
        return Record::failure(UNEXPECTED, 24);
    };
    let evidence = u64::from(suggestion.requires_approval)
        | (u64::from(document == original) << 1)
        | (u64::from(suggestion.value == Value::Number(proposed)) << 2);
    if evidence != 7 {
        return Record::failure(UNEXPECTED, evidence);
    }
    Record::value(proposed, evidence)
}

fn case_record(index: u32) -> Record {
    match index {
        0 => Record::value(number(-0.0), 0),
        1 => calculated_record(&formula_document(
            number(f64::from_bits(1)),
            Expression::Add {
                left: Box::new(input_reference()),
                right: Box::new(numeric(0.0)),
            },
        )),
        2 => calculated_record(&formula_document(
            number(f64::from_bits(1)),
            Expression::Multiply {
                left: Box::new(input_reference()),
                right: Box::new(numeric(0.5)),
            },
        )),
        3 => calculated_record(&formula_document(
            number(f64::MAX),
            Expression::Multiply {
                left: Box::new(input_reference()),
                right: Box::new(numeric(2.0)),
            },
        )),
        4 => calculated_record(&formula_document(
            number(1.0),
            Expression::Divide {
                left: Box::new(input_reference()),
                right: Box::new(numeric(-0.0)),
            },
        )),
        5 => {
            let mut document = formula_document(number(42.0), input_reference());
            document.entities.get_mut("entity-stable").unwrap().key = "renamed".into();
            let mut record = calculated_record(&document);
            let Value::Formula(expression) =
                &document.entities["entity-stable"].fields["output-stable"]
            else {
                return Record::failure(UNEXPECTED, 0);
            };
            match project_expression(&document, expression) {
                Ok(source) => record.auxiliary = source.len() as u64,
                Err(_) => return Record::failure(UNEXPECTED, 0),
            }
            record
        }
        6 => calculated_record(&formula_document(number(7.0), input_reference())),
        7 => calculated_record(&formula_document(
            number(1.0),
            Expression::Reference(FieldRef::new("entity-stable", "output-stable")),
        )),
        8 => {
            let mut document = formula_document(number(1.0), input_reference());
            let Value::Formula(expression) =
                document.entities["entity-stable"].fields["output-stable"].clone()
            else {
                return Record::failure(UNEXPECTED, 0);
            };
            document.entities.remove("entity-stable");
            document.entities.insert(
                EntityId::from("replacement-stable"),
                Entity {
                    id: EntityId::from("replacement-stable"),
                    key: EntityKey::from("source"),
                    schema: SchemaId::from("schema-stable"),
                    fields: BTreeMap::new(),
                },
            );
            match project_expression(&document, &expression) {
                Err(CanonicalAuthoringProjectionError::UnresolvableBoundReferences { targets }) => {
                    Record::failure(PROJECTION_FAILURE, targets.len() as u64)
                }
                _ => Record::failure(UNEXPECTED, 0),
            }
        }
        9 => calculated_record(&formula_document(
            number(0.0),
            Expression::Add {
                left: Box::new(Expression::Add {
                    left: Box::new(numeric(10_000_000_000_000_000.0)),
                    right: Box::new(numeric(-10_000_000_000_000_000.0)),
                }),
                right: Box::new(numeric(1.0)),
            },
        )),
        10 => storage_record("-0", 0x0000_0000_0000_0000, 0xc59b_bd16_19ab_00f3),
        11 => storage_record("5e-324", 0x0000_0000_0000_0001, 0xe684_515a_9f24_2ae3),
        12 => storage_record(
            "2.225073858507201e-308",
            0x000f_ffff_ffff_ffff,
            0x8203_14af_e965_5aa1,
        ),
        13 => storage_record(
            "2.2250738585072014e-308",
            0x0010_0000_0000_0000,
            0x82d0_fe69_b407_06a7,
        ),
        14 => storage_record(
            "9.999999999999997e-7",
            0x3eb0_c6f7_a0b5_ed8c,
            0x854a_1541_0f9d_e70a,
        ),
        15 => storage_record(
            "0.000001",
            0x3eb0_c6f7_a0b5_ed8d,
            0x65c8_993e_f682_f106,
        ),
        16 => storage_record(
            "999999999999999900000",
            0x444b_1ae4_d6e2_ef4f,
            0x8546_e95c_6c56_79c3,
        ),
        17 => storage_record(
            "1e+21",
            0x444b_1ae4_d6e2_ef50,
            0xefcf_57a3_ad34_d99d,
        ),
        18 => storage_record(
            "9007199254740993",
            0x4340_0000_0000_0000,
            0xb994_92e2_c966_bd08,
        ),
        19 => storage_record(
            "9007199254740995",
            0x4340_0000_0000_0002,
            0x5b95_2044_ebb3_a1bc,
        ),
        20 => storage_record(
            "1.7976931348623157e308",
            0x7fef_ffff_ffff_ffff,
            0x27de_67c0_448f_8d78,
        ),
        21 => workspace_calculation_record(),
        22 => ai_formula_record(),
        23 => ai_suggestion_record(),
        24 => adversarial_numeric_corpus_record(),
        25 => schema_type_change_record(),
        _ => Record::failure(UNEXPECTED, 0),
    }
}

/// Return the number of production-semantic conformance records.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_count() -> u32 {
    CASE_COUNT
}

/// Return the typed outcome class for one conformance record.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_class(index: u32) -> u32 {
    case_record(index).class
}

/// Return normalized Number bits for one successful conformance record.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_bits(index: u32) -> u64 {
    case_record(index).bits
}

/// Return case-specific deterministic dependency/path/projection/storage evidence.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_auxiliary(index: u32) -> u64 {
    case_record(index).auxiliary
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    for index in 0..CASE_COUNT {
        let record = case_record(index);
        println!(
            "{index}|{}|{:016x}|{}",
            record.class, record.bits, record.auxiliary
        );
    }
}
