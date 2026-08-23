//! Native/WASM conformance corpus for portable production semantics.

use std::collections::BTreeMap;

use tachiko_formula_engine::{
    CalculationError, CanonicalAuthoringProjectionError, calculate, project_expression,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{from_str as storage_from_str, to_canonical_string};

const CASE_COUNT: u32 = 21;
const VALUE: u32 = 0;
const DIVISION_BY_ZERO: u32 = 1;
const NON_FINITE: u32 = 2;
const CYCLE: u32 = 3;
const PROJECTION_FAILURE: u32 = 4;
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
        10 => storage_record("-0", 0x0000_0000_0000_0000, 0xdc22_5093_c082_d2dd),
        11 => storage_record("5e-324", 0x0000_0000_0000_0001, 0xc61d_92a8_30ba_56a5),
        12 => storage_record(
            "2.225073858507201e-308",
            0x000f_ffff_ffff_ffff,
            0x379f_2bcf_37b8_6303,
        ),
        13 => storage_record(
            "2.2250738585072014e-308",
            0x0010_0000_0000_0000,
            0xd790_d193_243b_4e31,
        ),
        14 => storage_record(
            "9.999999999999997e-7",
            0x3eb0_c6f7_a0b5_ed8c,
            0x705c_337d_c196_05e6,
        ),
        15 => storage_record(
            "0.000001",
            0x3eb0_c6f7_a0b5_ed8d,
            0x60c9_3bf3_ce1f_564a,
        ),
        16 => storage_record(
            "999999999999999900000",
            0x444b_1ae4_d6e2_ef4f,
            0x8d2b_a9d8_b52f_1f0d,
        ),
        17 => storage_record(
            "1e+21",
            0x444b_1ae4_d6e2_ef50,
            0xec85_c092_9a1f_5cc7,
        ),
        18 => storage_record(
            "9007199254740993",
            0x4340_0000_0000_0000,
            0xed75_8192_3be4_0cec,
        ),
        19 => storage_record(
            "9007199254740995",
            0x4340_0000_0000_0002,
            0xb6bf_79d9_b65d_91a0,
        ),
        20 => storage_record(
            "1.7976931348623157e308",
            0x7fef_ffff_ffff_ffff,
            0x6041_66b5_1a08_cee4,
        ),
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
