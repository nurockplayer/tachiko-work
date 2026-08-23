//! Native/WASM conformance corpus for portable production semantics.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use tachiko_ai_api::{explain_formula, suggest_field_change};
use tachiko_formula_engine::{
    CalculationError, CalculationFailure, CalculationOutcome, CanonicalAuthoringProjectionError,
    ReferenceFailure, calculate, calculate_complete, project_expression,
};
use tachiko_semantic_core::{
    DiagnosticCode, DiagnosticSeverity, Document, DocumentId, Entity, EntityId, EntityKey,
    Expression, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId,
    SchemaKey, SemanticSubject, Value,
};
use tachiko_storage::{
    FormatError as StorageFormatError, NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
    V2_MAX_NUMBER_TOKEN_BYTES, from_bytes as storage_from_bytes, from_str as storage_from_str,
    to_canonical_string,
};
use tachiko_workspace_engine::{
    ValidationReport, calculate_fields, diagnostic_codes, validation_report,
};

const CASE_COUNT: u32 = 46;
const VALUE: u32 = 0;
const DIVISION_BY_ZERO: u32 = 1;
const NON_FINITE: u32 = 2;
const CYCLE: u32 = 3;
const PROJECTION_FAILURE: u32 = 4;
const CORPUS_DIGEST: u32 = 5;
const NON_NUMERIC_REFERENCE: u32 = 6;
const COMPLETE_ORACLE: u32 = 7;
const DIRECT_JSON_ENVELOPE: u32 = 8;
const VALIDATION_REPORT: u32 = 9;
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

fn mix_field_ref(hash: &mut u64, field: &FieldRef) {
    mix_hash(hash, field.entity.as_str().as_bytes());
    mix_hash(hash, &[0]);
    mix_hash(hash, field.field.as_str().as_bytes());
    mix_hash(hash, &[0xff]);
}

fn mix_calculation_failure(hash: &mut u64, failure: &CalculationFailure) {
    match failure {
        CalculationFailure::InvalidExpression { .. } => mix_hash(hash, &[0]),
        CalculationFailure::InvalidReferences { targets } => {
            mix_hash(hash, &[1]);
            for (target, failure) in targets {
                mix_field_ref(hash, target);
                mix_hash(
                    hash,
                    &[match failure {
                        ReferenceFailure::Missing => 0,
                        ReferenceFailure::NonNumeric => 1,
                    }],
                );
            }
        }
        CalculationFailure::Cycle { members } => {
            mix_hash(hash, &[2]);
            for member in members {
                mix_field_ref(hash, member);
            }
        }
        CalculationFailure::FailedDependencies { dependencies } => {
            mix_hash(hash, &[3]);
            for dependency in dependencies {
                mix_field_ref(hash, dependency);
            }
        }
        CalculationFailure::DivisionByZero => mix_hash(hash, &[4]),
        CalculationFailure::NonFiniteResult => mix_hash(hash, &[5]),
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

fn complete_oracle_record() -> Record {
    let schema_id = SchemaId::from("oracle-schema-stable");
    let entity_id = EntityId::from("oracle-entity-stable");
    let formula = |expression| Value::Formula(expression);
    let reference = |field| {
        Expression::Reference(FieldRef::new("oracle-entity-stable", field))
    };
    let fields = [
        "cycle-a",
        "cycle-b",
        "missing-failure",
        "type-failure",
        "evaluation-failure",
        "downstream-failure",
        "successful-independent",
    ];
    let mut definitions = fields
        .into_iter()
        .map(|field_id| (FieldId::from(field_id), field(field_id)))
        .collect::<BTreeMap<_, _>>();
    definitions.insert(
        FieldId::from("text-target"),
        FieldDefinition {
            id: FieldId::from("text-target"),
            key: FieldKey::from("text-target"),
            field_type: FieldType::Text,
            required: true,
        },
    );
    let values = BTreeMap::from([
        (FieldId::from("cycle-a"), formula(reference("cycle-b"))),
        (FieldId::from("cycle-b"), formula(reference("cycle-a"))),
        (
            FieldId::from("missing-failure"),
            formula(reference("missing-target")),
        ),
        (
            FieldId::from("type-failure"),
            formula(reference("text-target")),
        ),
        (
            FieldId::from("evaluation-failure"),
            formula(Expression::Divide {
                left: Box::new(numeric(1.0)),
                right: Box::new(numeric(0.0)),
            }),
        ),
        (
            FieldId::from("downstream-failure"),
            formula(Expression::Add {
                left: Box::new(reference("cycle-a")),
                right: Box::new(reference("evaluation-failure")),
            }),
        ),
        (
            FieldId::from("successful-independent"),
            formula(numeric(2.0)),
        ),
        (
            FieldId::from("text-target"),
            Value::Text("not numeric".to_owned()),
        ),
    ]);
    let document = Document {
        id: DocumentId::from("oracle-document-stable"),
        title: "Complete oracle parity".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("oracle-schema"),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("oracle-entity"),
                schema: schema_id,
                fields: values,
            },
        )]),
    };
    let node = |field| FieldRef::new("oracle-entity-stable", field);
    let cycle = BTreeSet::from([node("cycle-a"), node("cycle-b")]);
    let expected_failures = BTreeMap::from([
        (
            node("cycle-a"),
            CalculationFailure::Cycle {
                members: cycle.clone(),
            },
        ),
        (
            node("cycle-b"),
            CalculationFailure::Cycle { members: cycle },
        ),
        (
            node("downstream-failure"),
            CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([
                    node("cycle-a"),
                    node("evaluation-failure"),
                ]),
            },
        ),
        (
            node("evaluation-failure"),
            CalculationFailure::DivisionByZero,
        ),
        (
            node("missing-failure"),
            CalculationFailure::InvalidReferences {
                targets: BTreeMap::from([(
                    node("missing-target"),
                    ReferenceFailure::Missing,
                )]),
            },
        ),
        (
            node("type-failure"),
            CalculationFailure::InvalidReferences {
                targets: BTreeMap::from([(
                    node("text-target"),
                    ReferenceFailure::NonNumeric,
                )]),
            },
        ),
    ]);
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        return Record::failure(UNEXPECTED, 26);
    };
    if failures.failures() != &expected_failures || failures.dependencies().len() != fields.len() {
        return Record::failure(UNEXPECTED, failures.failures().len() as u64);
    }

    let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
    for (subject, failure) in failures.failures() {
        mix_field_ref(&mut fingerprint, subject);
        mix_calculation_failure(&mut fingerprint, failure);
    }
    for (subject, targets) in failures.dependencies() {
        mix_field_ref(&mut fingerprint, subject);
        for target in targets {
            mix_field_ref(&mut fingerprint, target);
        }
    }
    Record {
        class: COMPLETE_ORACLE,
        bits: fingerprint,
        auxiliary: failures.failures().len() as u64,
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

fn padded_direct_json(prefix: &str, suffix: &str, target: usize) -> String {
    let filler = target
        .checked_sub(prefix.len() + suffix.len())
        .expect("portable direct-JSON fixture framing fits its target");
    let mut input = String::with_capacity(target);
    input.push_str(prefix);
    input.extend(std::iter::repeat_n('x', filler));
    input.push_str(suffix);
    input
}

fn direct_json_envelope_input(index: u32) -> Vec<u8> {
    let oversized = NORMAL_DIRECT_JSON_MAX_INPUT_BYTES + 1;
    match index {
        0 => br#"{"format_version":1,"id":"doc","title":"x","schemas":{},"entities":{}}"#
            .to_vec(),
        1 => br#"{"format_version":2,"id":"doc","title":"x","schemas":{},"entities":{}}"#
            .to_vec(),
        2 => padded_direct_json(r#"{"format_version":2,"future":""#, "", oversized).into_bytes(),
        3 => padded_direct_json(
            r#"{"format_version":3,"padding":""#,
            r#"","future":{"a":1,"\u0061":2}}"#,
            oversized,
        )
        .into_bytes(),
        4 => padded_direct_json(r#"{"future":""#, r#""}"#, oversized).into_bytes(),
        5 => padded_direct_json(
            r#"{"format_version":"2","future":""#,
            r#""}"#,
            oversized,
        )
        .into_bytes(),
        6 => padded_direct_json(
            r#"{"format_version":3,"future":""#,
            r#""}"#,
            oversized,
        )
        .into_bytes(),
        7 => {
            let mut input = vec![b' '; oversized];
            input[oversized - 1] = 0xff;
            input
        }
        8 => padded_direct_json(
            r#"{"format_version":1,"id":"doc","title":""#,
            r#"","schemas":{},"entities":{}}"#,
            oversized,
        )
        .into_bytes(),
        9 => padded_direct_json(
            r#"{"format_version":2,"id":"doc","title":""#,
            r#"","schemas":{},"entities":{}}"#,
            NORMAL_DIRECT_JSON_MAX_INPUT_BYTES,
        )
        .into_bytes(),
        10 => padded_direct_json(
            r#"{"format_version":2,"id":"doc","title":""#,
            r#"","schemas":{},"entities":{}}"#,
            oversized,
        )
        .into_bytes(),
        11 => {
            let token = format!("1{}", "0".repeat(V2_MAX_NUMBER_TOKEN_BYTES));
            format!(r#"{{"format_version":2,"future":{token}}}"#).into_bytes()
        }
        12 => {
            let token = format!("1{}", "0".repeat(V2_MAX_NUMBER_TOKEN_BYTES));
            format!(r#"{{"format_version":3,"future":{token}}}"#).into_bytes()
        }
        13 => padded_direct_json(
            r#"{"format_version":3,"a":1,"\u0061":2,"future":""#,
            "",
            oversized,
        )
        .into_bytes(),
        14 => format!(
            "{{\"format_version\":3,\"future\":{}0{}}}",
            "[".repeat(1_024),
            "]".repeat(1_024)
        )
        .into_bytes(),
        _ => Vec::new(),
    }
}

fn direct_json_result_class(result: Result<Document, StorageFormatError>) -> u64 {
    match result {
        Ok(_) => 0,
        Err(StorageFormatError::InvalidUtf8 { .. }) => 1,
        Err(StorageFormatError::InvalidJson { .. }) => 2,
        Err(StorageFormatError::DuplicateMember { .. }) => 3,
        Err(StorageFormatError::VersionMissing) => 4,
        Err(StorageFormatError::VersionMalformed) => 5,
        Err(StorageFormatError::UnsupportedVersion { .. }) => 6,
        Err(StorageFormatError::ResourceLimit {
            resource: "input", ..
        }) => 7,
        Err(StorageFormatError::ResourceLimit {
            resource: "number token",
            ..
        }) => 8,
        Err(_) => u64::from(UNEXPECTED),
    }
}

fn direct_json_envelope_record(index: u32) -> Record {
    const EXPECTED: [u64; 15] = [0, 0, 7, 7, 7, 7, 7, 7, 7, 0, 7, 8, 6, 7, 2];

    let input = direct_json_envelope_input(index);
    let actual = direct_json_result_class(storage_from_bytes(&input));
    let Some(&expected) = EXPECTED.get(index as usize) else {
        return Record::failure(UNEXPECTED, u64::from(index));
    };
    if actual != expected {
        return Record {
            class: UNEXPECTED,
            bits: actual,
            auxiliary: expected,
        };
    }
    Record {
        class: DIRECT_JSON_ENVELOPE,
        bits: expected,
        auxiliary: input.len() as u64,
    }
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

fn append_token(bytes: &mut Vec<u8>, token: &str) {
    bytes.extend_from_slice(&(token.len() as u64).to_le_bytes());
    bytes.extend_from_slice(token.as_bytes());
}

fn append_field_ref(bytes: &mut Vec<u8>, field: &FieldRef) {
    append_token(bytes, field.entity.as_str());
    append_token(bytes, field.field.as_str());
}

fn append_subject(bytes: &mut Vec<u8>, subject: &SemanticSubject) {
    match subject {
        SemanticSubject::Document(document) => {
            bytes.push(0);
            append_token(bytes, document.as_str());
        }
        SemanticSubject::Schema(schema) => {
            bytes.push(1);
            append_token(bytes, schema.as_str());
        }
        SemanticSubject::SchemaField { schema, field } => {
            bytes.push(2);
            append_token(bytes, schema.as_str());
            append_token(bytes, field.as_str());
        }
        SemanticSubject::Entity(entity) => {
            bytes.push(3);
            append_token(bytes, entity.as_str());
        }
        SemanticSubject::EntityField(field) => {
            bytes.push(4);
            append_field_ref(bytes, field);
        }
    }
}

fn validation_fingerprint(report: &ValidationReport) -> u64 {
    let mut bytes = Vec::new();
    for observation in report.stable_observations() {
        append_token(&mut bytes, observation.code.as_str());
        bytes.push(match observation.severity {
            DiagnosticSeverity::Error => 0,
        });
        bytes.extend_from_slice(&(observation.subjects.len() as u64).to_le_bytes());
        for subject in &observation.subjects {
            append_subject(&mut bytes, subject);
        }
        bytes.extend_from_slice(&(observation.related_subjects.len() as u64).to_le_bytes());
        for subject in &observation.related_subjects {
            append_subject(&mut bytes, subject);
        }
        bytes.extend_from_slice(&(observation.facts.len() as u64).to_le_bytes());
        for fact in &observation.facts {
            append_token(&mut bytes, fact.name);
            append_token(&mut bytes, &fact.value);
        }
        append_token(&mut bytes, observation.provider.as_str());
    }
    fnv1a64(&bytes)
}

fn oracle_document() -> Document {
    let formula_ids = [
        "structural",
        "binding",
        "cycle-a",
        "cycle-b",
        "dependent",
        "zero",
        "depends-zero",
        "independent",
    ];
    let mut definitions: BTreeMap<_, _> = formula_ids
        .into_iter()
        .map(|id| (FieldId::from(id), field(id)))
        .collect();
    definitions.insert(
        FieldId::from("text"),
        FieldDefinition {
            id: "text".into(),
            key: "text".into(),
            field_type: FieldType::Text,
            required: true,
        },
    );

    let reference = |field| Expression::Reference(FieldRef::new("entity", field));
    let mut structural = reference("missing-structural");
    for _ in 0..65 {
        structural = Expression::Add {
            left: Box::new(structural),
            right: Box::new(numeric(1.0)),
        };
    }
    let fields = BTreeMap::from([
        (FieldId::from("structural"), Value::Formula(structural)),
        (
            FieldId::from("binding"),
            Value::Formula(Expression::Add {
                left: Box::new(reference("binding")),
                right: Box::new(Expression::Add {
                    left: Box::new(reference("missing-binding")),
                    right: Box::new(reference("text")),
                }),
            }),
        ),
        (
            FieldId::from("cycle-a"),
            Value::Formula(reference("cycle-b")),
        ),
        (
            FieldId::from("cycle-b"),
            Value::Formula(Expression::Divide {
                left: Box::new(reference("cycle-a")),
                right: Box::new(numeric(0.0)),
            }),
        ),
        (
            FieldId::from("dependent"),
            Value::Formula(Expression::Add {
                left: Box::new(reference("cycle-a")),
                right: Box::new(reference("binding")),
            }),
        ),
        (
            FieldId::from("zero"),
            Value::Formula(Expression::Divide {
                left: Box::new(numeric(1.0)),
                right: Box::new(numeric(0.0)),
            }),
        ),
        (
            FieldId::from("depends-zero"),
            Value::Formula(reference("zero")),
        ),
        (
            FieldId::from("independent"),
            Value::Formula(Expression::Add {
                left: Box::new(numeric(2.0)),
                right: Box::new(numeric(3.0)),
            }),
        ),
        (FieldId::from("text"), Value::Text("text".to_owned())),
    ]);
    Document {
        id: "oracle-document".into(),
        title: "Full formula oracle".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema"),
            Schema {
                id: "schema".into(),
                key: "schema".into(),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity"),
            Entity {
                id: "entity".into(),
                key: "entity".into(),
                schema: "schema".into(),
                fields,
            },
        )]),
    }
}

fn validation_accumulation_record() -> Record {
    let mut document = oracle_document();
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .insert(FieldId::from("required"), field("required"));
    for id in [
        "aggregate-dependent",
        "blocked-owner",
        "local-first",
        "mixed-binding",
        "reference-type-error",
        "required-dependent",
        "typed-dependent",
        "typed-input",
    ] {
        document
            .schemas
            .get_mut("schema")
            .unwrap()
            .fields
            .insert(FieldId::from(id), field(id));
    }
    document.schemas.get_mut("schema").unwrap().fields.insert(
        FieldId::from("reference-target"),
        FieldDefinition {
            id: "reference-target".into(),
            key: "reference-target".into(),
            field_type: FieldType::Reference {
                schema: "missing-target-schema".into(),
            },
            required: false,
        },
    );
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("blocked-owner")
        .unwrap()
        .id = "different-stable-id".into();
    let entity = document.entities.get_mut("entity").unwrap();
    entity.fields.insert(
        FieldId::from("text"),
        Value::Number(number(7.0)),
    );
    entity.fields.insert(
        FieldId::from("cycle-a"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("entity", "cycle-b"))),
            right: Box::new(numeric(1.0)),
        }),
    );
    entity.fields.insert(
        FieldId::from("required-dependent"),
        Value::Formula(Expression::Reference(FieldRef::new("entity", "required"))),
    );
    entity.fields.insert(
        FieldId::from("mixed-binding"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("orphan", "unknown"))),
            right: Box::new(Expression::Reference(FieldRef::new("ghost", "value"))),
        }),
    );
    entity.fields.insert(
        FieldId::from("reference-type-error"),
        Value::Formula(Expression::Reference(FieldRef::new(
            "entity",
            "reference-target",
        ))),
    );
    entity.fields.insert(
        FieldId::from("typed-input"),
        Value::Text("not numeric".to_owned()),
    );
    entity.fields.insert(
        FieldId::from("typed-dependent"),
        Value::Formula(Expression::Reference(FieldRef::new("entity", "typed-input"))),
    );
    entity.fields.insert(
        FieldId::from("blocked-owner"),
        Value::Formula(Expression::Divide {
            left: Box::new(numeric(1.0)),
            right: Box::new(numeric(0.0)),
        }),
    );
    entity.fields.insert(
        FieldId::from("aggregate-dependent"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new(
                "entity",
                "blocked-owner",
            ))),
            right: Box::new(Expression::Reference(FieldRef::new("entity", "zero"))),
        }),
    );
    entity.fields.insert(
        FieldId::from("local-first"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(numeric(1.0)),
                right: Box::new(numeric(0.0)),
            }),
            right: Box::new(numeric(1.0)),
        }),
    );
    document.entities.insert(
        EntityId::from("orphan"),
        Entity {
            id: "orphan".into(),
            key: "orphan".into(),
            schema: "missing-schema".into(),
            fields: BTreeMap::from([(
                FieldId::from("unknown"),
                Value::Formula(numeric(1.0)),
            )]),
        },
    );
    let report = validation_report(&document);
    let codes: BTreeSet<_> = report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    let exact = codes.contains(&DiagnosticCode::MISSING_REQUIRED_FIELD)
        && codes.contains(&DiagnosticCode::MISSING_SCHEMA)
        && codes.contains(&diagnostic_codes::FORMULA_INVALID_REFERENCES)
        && codes.contains(&diagnostic_codes::FORMULA_CYCLE)
        && codes.contains(&diagnostic_codes::FORMULA_FAILED_DEPENDENCY)
        && codes.contains(&diagnostic_codes::FORMULA_DIVISION_BY_ZERO)
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::TYPE_MISMATCH
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new("entity", "text"))]
        })
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity", "binding",
                    ))]
                && diagnostic.related_subjects
                    == [
                        SemanticSubject::EntityField(FieldRef::new(
                            "entity",
                            "missing-binding",
                        )),
                        SemanticSubject::EntityField(FieldRef::new("entity", "text")),
                    ]
        })
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "mixed-binding",
                    ))]
                && diagnostic.related_subjects
                    == [SemanticSubject::EntityField(FieldRef::new("ghost", "value"))]
                && diagnostic.facts.len() == 1
                && diagnostic.facts[0].name == "missing_target"
                && diagnostic.facts[0].value == "5:ghost5:value"
        })
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "reference-type-error",
                    ))]
                && diagnostic.related_subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "reference-target",
                    ))]
                && diagnostic.facts.len() == 1
                && diagnostic.facts[0].name == "non_numeric_target"
                && diagnostic.facts[0].value == "6:entity16:reference-target"
        })
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_FAILED_DEPENDENCY
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "aggregate-dependent",
                    ))]
                && diagnostic.related_subjects
                    == [SemanticSubject::EntityField(FieldRef::new("entity", "zero"))]
        })
        && !report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_DIVISION_BY_ZERO
                && diagnostic
                    .subjects
                    .contains(&SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "blocked-owner",
                    )))
        })
        && !report.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .subjects
                .contains(&SemanticSubject::EntityField(FieldRef::new(
                    "orphan", "unknown",
                )))
        });
    if !exact {
        return Record::failure(UNEXPECTED, 31);
    }
    Record {
        class: VALIDATION_REPORT,
        bits: report.diagnostics().len() as u64,
        auxiliary: validation_fingerprint(&report),
    }
}

fn disjoint_cycle_document(reverse_insertion: bool) -> Document {
    let mut formulas = vec![
        ("a-cycle-1", "a-cycle-2"),
        ("a-cycle-2", "a-cycle-1"),
        ("b-cycle-1", "b-cycle-2"),
        ("b-cycle-2", "b-cycle-3"),
        ("b-cycle-3", "b-cycle-1"),
        ("depends-a", "a-cycle-1"),
        ("depends-b", "b-cycle-2"),
    ];
    if reverse_insertion {
        formulas.reverse();
    }
    let mut definitions = BTreeMap::new();
    let mut fields = BTreeMap::new();
    for (formula, dependency) in formulas {
        definitions.insert(FieldId::from(formula), field(formula));
        fields.insert(
            FieldId::from(formula),
            Value::Formula(Expression::Reference(FieldRef::new("entity", dependency))),
        );
    }
    Document {
        id: "disjoint-cycles".into(),
        title: "Disjoint cycles".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema"),
            Schema {
                id: "schema".into(),
                key: "schema".into(),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity"),
            Entity {
                id: "entity".into(),
                key: "entity".into(),
                schema: "schema".into(),
                fields,
            },
        )]),
    }
}

fn disjoint_cycle_record() -> Record {
    let forward = validation_report(&disjoint_cycle_document(false));
    let reversed = validation_report(&disjoint_cycle_document(true));
    let repeated = validation_report(&disjoint_cycle_document(false));
    let subjects = |fields: &[&str]| {
        fields
            .iter()
            .map(|field| SemanticSubject::EntityField(FieldRef::new("entity", *field)))
            .collect::<Vec<_>>()
    };
    let cycles = forward
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
        .collect::<Vec<_>>();
    let dependency = |formula: &str, failed: &str| {
        forward.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_FAILED_DEPENDENCY
                && diagnostic.subjects == subjects(&[formula])
                && diagnostic.related_subjects == subjects(&[failed])
        })
    };
    let exact = forward.stable_observations() == reversed.stable_observations()
        && repeated == forward
        && cycles.len() == 2
        && cycles[0].subjects == subjects(&["a-cycle-1", "a-cycle-2"])
        && cycles[1].subjects == subjects(&["b-cycle-1", "b-cycle-2", "b-cycle-3"])
        && dependency("depends-a", "a-cycle-1")
        && dependency("depends-b", "b-cycle-2");
    if !exact {
        return Record::failure(UNEXPECTED, forward.diagnostics().len() as u64);
    }
    Record {
        class: VALIDATION_REPORT,
        bits: forward.diagnostics().len() as u64,
        auxiliary: validation_fingerprint(&forward),
    }
}

fn rename_stability_record() -> Record {
    let mut before = formula_document(
        number(1.0),
        Expression::Reference(FieldRef::new("entity-stable", "missing-stable")),
    );
    before.entities.insert(
        EntityId::from("second-stable"),
        Entity {
            id: "second-stable".into(),
            key: "source".into(),
            schema: "schema-stable".into(),
            fields: BTreeMap::from([
                (
                    FieldId::from("input-stable"),
                    Value::Number(number(2.0)),
                ),
                (
                    FieldId::from("output-stable"),
                    Value::Number(number(3.0)),
                ),
            ]),
        },
    );
    let mut after = before.clone();
    after.schemas.get_mut("schema-stable").unwrap().key = "renamed-schema".into();
    for definition in after
        .schemas
        .get_mut("schema-stable")
        .unwrap()
        .fields
        .values_mut()
    {
        definition.key = format!("renamed-{}", definition.key).into();
    }
    after.entities.get_mut("entity-stable").unwrap().key = "renamed-source".into();
    after.entities.get_mut("second-stable").unwrap().key = "renamed-source".into();

    let before = validation_report(&before);
    let after = validation_report(&after);
    let duplicate_subjects = vec![
        SemanticSubject::Entity(EntityId::from("entity-stable")),
        SemanticSubject::Entity(EntityId::from("second-stable")),
    ];
    let exact = before.stable_observations() == after.stable_observations()
        && before.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::DUPLICATE_KEY
                && diagnostic.subjects == duplicate_subjects
        })
        && before.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.related_subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity-stable",
                        "missing-stable",
                    ))]
        });
    if !exact {
        return Record::failure(UNEXPECTED, 32);
    }
    Record {
        class: VALIDATION_REPORT,
        bits: before.diagnostics().len() as u64,
        auxiliary: validation_fingerprint(&before),
    }
}

fn validation_cycle_record() -> Record {
    let report = validation_report(&oracle_document());
    let cycle = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
        .collect::<Vec<_>>();
    let dependency = report.diagnostics().iter().find(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_FAILED_DEPENDENCY
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity", "dependent",
                ))]
    });
    let expected_cycle = vec![
        SemanticSubject::EntityField(FieldRef::new("entity", "cycle-a")),
        SemanticSubject::EntityField(FieldRef::new("entity", "cycle-b")),
    ];
    let expected_dependencies = vec![
        SemanticSubject::EntityField(FieldRef::new("entity", "binding")),
        SemanticSubject::EntityField(FieldRef::new("entity", "cycle-a")),
    ];
    let exact = cycle.len() == 1
        && cycle[0].subjects == expected_cycle
        && cycle[0].provider.as_str() == "tachiko.formula-engine"
        && dependency.is_some_and(|diagnostic| {
            diagnostic.related_subjects == expected_dependencies
        });
    if !exact {
        return Record::failure(UNEXPECTED, 33);
    }
    Record {
        class: VALIDATION_REPORT,
        bits: report.diagnostics().len() as u64,
        auxiliary: validation_fingerprint(&report),
    }
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
        26 => complete_oracle_record(),
        27..=41 => direct_json_envelope_record(index - 27),
        42 => validation_accumulation_record(),
        43 => rename_stability_record(),
        44 => validation_cycle_record(),
        45 => disjoint_cycle_record(),
        _ => Record::failure(UNEXPECTED, 0),
    }
}

fn conformance_records() -> &'static [Record] {
    static RECORDS: OnceLock<Vec<Record>> = OnceLock::new();
    RECORDS.get_or_init(|| (0..CASE_COUNT).map(case_record).collect())
}

fn cached_case_record(index: u32) -> Record {
    conformance_records()
        .get(index as usize)
        .copied()
        .unwrap_or_else(|| Record::failure(UNEXPECTED, 0))
}

/// Return the number of production-semantic conformance records.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_count() -> u32 {
    CASE_COUNT
}

/// Return the typed outcome class for one conformance record.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_class(index: u32) -> u32 {
    cached_case_record(index).class
}

/// Return normalized Number bits for one successful conformance record.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_bits(index: u32) -> u64 {
    cached_case_record(index).bits
}

/// Return case-specific deterministic semantic/projection/storage evidence.
#[unsafe(no_mangle)]
pub extern "C" fn tachiko_case_auxiliary(index: u32) -> u64 {
    cached_case_record(index).auxiliary
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    for (index, record) in conformance_records().iter().enumerate() {
        println!(
            "{index}|{}|{:016x}|{}",
            record.class, record.bits, record.auxiliary
        );
    }
}
