//! Native/WASM conformance corpus for portable production semantics.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use sha2::{Digest, Sha256};
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
    FormatError as StorageFormatError, NORMAL_DIRECT_JSON_MAX_INPUT_BYTES, ROPROJ_V1_PATHS,
    V2_MAX_NUMBER_TOKEN_BYTES, decode_portable_package_v1, decode_roproj_v1,
    encode_portable_package_v1, encode_roproj_v1, from_bytes as storage_from_bytes,
    from_str as storage_from_str, portable_package_payload_root, to_canonical_string,
};
use tachiko_workspace_engine::{
    ValidationReport, calculate_fields, diagnostic_codes,
    analysis_operations::{
        AnalysisBucket, AnalysisCollectionKind, AnalysisDefinition, AnalysisDerivation,
        AnalysisFailure, AnalysisFieldRole, AnalysisGroup, AnalysisGroupKey, AnalysisLineage,
        AnalysisOperationError, AnalysisOutcome, AnalysisPredicate, AnalysisPredicateOperator,
        AnalysisProjection, AnalysisQueryResult, AnalysisResultRequest, AnalysisResultValue,
        AnalysisValueKind, MetricIncompleteReason, NumericAggregateOutcome,
        PairedAnalysisQueryResult, PredicateOperand,
    },
    formula_operations::{
        FormulaCalculationOutcome, FormulaReasoningOutcome, FormulaUpdateRequest, NumberOverride,
        ScenarioOutcome, ScenarioRequest, ScenarioTargetOutcome, ValidatorConfiguration,
    },
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        Grant, GrantId, GrantRequirement, MutationClass, OperationFamily, PatchLifecycle,
        PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind, ProposalId,
        ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validation_report,
};

const CASE_COUNT: u32 = 55;
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
const ROPROJ_V1_EXACT_TREE: u32 = 10;
const PORTABLE_PACKAGE_V1_EXACT_BYTES: u32 = 11;
const ANALYSIS_COMPLETE: u32 = 12;
const ANALYSIS_FAILURE: u32 = 13;
const ANALYSIS_PAIRED_AUTHORIZATION: u32 = 14;
const RESIDENT_SESSION: u32 = 15;
const UNEXPECTED: u32 = 255;

const VALIDATION_ACCUMULATION_COUNT: usize = 16;
const VALIDATION_ACCUMULATION_FINGERPRINT: u64 = 14_450_937_261_505_426_764;
const VALIDATION_RENAME_COUNT: usize = 2;
const VALIDATION_RENAME_FINGERPRINT: u64 = 17_244_910_774_212_126_556;
const VALIDATION_CYCLE_COUNT: usize = 6;
const VALIDATION_CYCLE_FINGERPRINT: u64 = 12_164_157_338_575_685_884;
const VALIDATION_DISJOINT_CYCLE_COUNT: usize = 4;
const VALIDATION_DISJOINT_CYCLE_FINGERPRINT: u64 = 18_384_632_427_777_425_720;
const ROPROJ_V1_EXACT_TREE_FINGERPRINT: u64 = 2_796_923_835_209_599_950;
const ROPROJ_V1_EXACT_TREE_FILE_COUNT: u64 = 18;
const PORTABLE_PACKAGE_V1_FINGERPRINT: u64 = 4_165_964_772_177_000_947;
const PORTABLE_PACKAGE_V1_LENGTH: u64 = 2_692;
const PORTABLE_PACKAGE_V1_SHA256: [u8; 32] = [
    0x13, 0x68, 0xeb, 0xe3, 0x8c, 0x86, 0xde, 0x28, 0xd2, 0x37, 0x9a, 0xe6, 0xc0, 0xca, 0x7a, 0x5c,
    0xa8, 0x50, 0x25, 0x43, 0x00, 0x2f, 0xe0, 0x84, 0xe3, 0x32, 0x54, 0xad, 0x1d, 0xb4, 0xd7, 0xbc,
];
const PORTABLE_PACKAGE_V1_PAYLOAD_ROOT: [u8; 32] = [
    0x71, 0xe2, 0xb1, 0x17, 0x0a, 0xe3, 0xb2, 0xc2, 0x25, 0x9c, 0xc0, 0xc9, 0x0c, 0x21, 0x73, 0x89,
    0xa1, 0xe5, 0x9c, 0x49, 0x0b, 0x5c, 0xcd, 0xe4, 0xc6, 0xfe, 0x2d, 0xad, 0xae, 0x1f, 0xed, 0x9c,
];

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

fn mix_framed(hash: &mut u64, domain: &[u8], payload: &[u8]) {
    mix_hash(hash, &(domain.len() as u64).to_le_bytes());
    mix_hash(hash, domain);
    mix_hash(hash, &(payload.len() as u64).to_le_bytes());
    mix_hash(hash, payload);
}

fn roproj_v1_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let Ok(tree) = encode_roproj_v1(&document) else {
        return Record::failure(UNEXPECTED, 46_u64 << 32);
    };
    if tree.files().len() != ROPROJ_V1_PATHS.len()
        || tree
            .files()
            .iter()
            .zip(ROPROJ_V1_PATHS)
            .any(|(file, expected_path)| file.path() != expected_path)
    {
        return Record::failure(UNEXPECTED, (46_u64 << 32) | tree.files().len() as u64);
    }

    let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
    mix_framed(
        &mut fingerprint,
        b"record-domain",
        b"tachiko.portable-conformance/roproj-v1-exact-tree/fnv1a64",
    );
    mix_framed(
        &mut fingerprint,
        b"file-count",
        &(tree.files().len() as u64).to_le_bytes(),
    );
    for file in tree.files() {
        mix_framed(&mut fingerprint, b"relative-path", file.path().as_bytes());
        mix_framed(&mut fingerprint, b"exact-body", file.bytes());
    }

    let Ok(decoded) = decode_roproj_v1(&tree) else {
        return Record::failure(UNEXPECTED, (46_u64 << 32) | 1);
    };
    if decoded != document {
        return Record::failure(UNEXPECTED, (46_u64 << 32) | 2);
    }
    let Ok(reencoded) = encode_roproj_v1(&decoded) else {
        return Record::failure(UNEXPECTED, (46_u64 << 32) | 3);
    };
    if reencoded != tree {
        return Record::failure(UNEXPECTED, (46_u64 << 32) | 4);
    }

    let record = Record {
        class: ROPROJ_V1_EXACT_TREE,
        bits: fingerprint,
        auxiliary: tree.files().len() as u64,
    };
    if record.class != ROPROJ_V1_EXACT_TREE
        || record.bits != ROPROJ_V1_EXACT_TREE_FINGERPRINT
        || record.auxiliary != ROPROJ_V1_EXACT_TREE_FILE_COUNT
    {
        return Record::failure(UNEXPECTED, fingerprint);
    }
    record
}

fn portable_package_v1_record() -> Record {
    let document = Document::empty("doc-empty", "Empty");
    let Ok(tree) = encode_roproj_v1(&document) else {
        return Record::failure(UNEXPECTED, 47_u64 << 32);
    };
    let root = portable_package_payload_root(&tree);
    if root != PORTABLE_PACKAGE_V1_PAYLOAD_ROOT {
        return Record::failure(
            UNEXPECTED,
            u64::from_le_bytes(root[..8].try_into().unwrap()),
        );
    }
    let Ok(package) = encode_portable_package_v1(&tree) else {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 1);
    };
    let fingerprint = fnv1a64(&package);
    if package.len() as u64 != PORTABLE_PACKAGE_V1_LENGTH
        || fingerprint != PORTABLE_PACKAGE_V1_FINGERPRINT
    {
        return Record::failure(UNEXPECTED, fingerprint);
    }
    let package_digest: [u8; 32] = Sha256::digest(&package).into();
    if package_digest != PORTABLE_PACKAGE_V1_SHA256 {
        return Record::failure(
            UNEXPECTED,
            u64::from_le_bytes(package_digest[..8].try_into().unwrap()),
        );
    }
    let Ok(verified) = decode_portable_package_v1(&package) else {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 2);
    };
    if verified.tree() != &tree || verified.payload_root() != root {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 3);
    }
    let Ok(decoded_tree) = decode_roproj_v1(verified.tree()) else {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 4);
    };
    if decoded_tree != document {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 5);
    }
    let Ok(decoded_package) = storage_from_bytes(&package) else {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 6);
    };
    if decoded_package != document {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 7);
    }
    let Ok(reencoded) = encode_portable_package_v1(verified.tree()) else {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 8);
    };
    if reencoded != package {
        return Record::failure(UNEXPECTED, (47_u64 << 32) | 9);
    }
    Record {
        class: PORTABLE_PACKAGE_V1_EXACT_BYTES,
        bits: fingerprint,
        auxiliary: package.len() as u64,
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
    schema.fields.get_mut("input-stable").unwrap().field_type = FieldType::Text;
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
    let reference = |field| Expression::Reference(FieldRef::new("oracle-entity-stable", field));
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
                dependencies: BTreeSet::from([node("cycle-a"), node("evaluation-failure")]),
            },
        ),
        (
            node("evaluation-failure"),
            CalculationFailure::DivisionByZero,
        ),
        (
            node("missing-failure"),
            CalculationFailure::InvalidReferences {
                targets: BTreeMap::from([(node("missing-target"), ReferenceFailure::Missing)]),
            },
        ),
        (
            node("type-failure"),
            CalculationFailure::InvalidReferences {
                targets: BTreeMap::from([(node("text-target"), ReferenceFailure::NonNumeric)]),
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
        0 => br#"{"format_version":1,"id":"doc","title":"x","schemas":{},"entities":{}}"#.to_vec(),
        1 => br#"{"format_version":2,"id":"doc","title":"x","schemas":{},"entities":{}}"#.to_vec(),
        2 => padded_direct_json(r#"{"format_version":2,"future":""#, "", oversized).into_bytes(),
        3 => padded_direct_json(
            r#"{"format_version":3,"padding":""#,
            r#"","future":{"a":1,"\u0061":2}}"#,
            oversized,
        )
        .into_bytes(),
        4 => padded_direct_json(r#"{"future":""#, r#""}"#, oversized).into_bytes(),
        5 => padded_direct_json(r#"{"format_version":"2","future":""#, r#""}"#, oversized)
            .into_bytes(),
        6 => {
            padded_direct_json(r#"{"format_version":3,"future":""#, r#""}"#, oversized).into_bytes()
        }
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
    let Ok(explanation) =
        explain_formula(&document, &FieldRef::new("entity-stable", "output-stable"))
    else {
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

fn fixed_validation_record(
    report: &ValidationReport,
    expected_count: usize,
    expected_fingerprint: u64,
    failure_marker: u64,
) -> Record {
    let actual_count = report.diagnostics().len();
    let actual_fingerprint = validation_fingerprint(report);
    if actual_count != expected_count {
        return Record::failure(UNEXPECTED, failure_marker | actual_count as u64);
    }
    if actual_fingerprint != expected_fingerprint {
        return Record::failure(UNEXPECTED, actual_fingerprint);
    }
    Record {
        class: VALIDATION_REPORT,
        bits: actual_count as u64,
        auxiliary: actual_fingerprint,
    }
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
    entity
        .fields
        .insert(FieldId::from("text"), Value::Number(number(7.0)));
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
        Value::Formula(Expression::Reference(FieldRef::new(
            "entity",
            "typed-input",
        ))),
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
            fields: BTreeMap::from([(FieldId::from("unknown"), Value::Formula(numeric(1.0)))]),
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
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity", "text",
                    ))]
        })
        && report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity", "binding",
                    ))]
                && diagnostic.related_subjects
                    == [
                        SemanticSubject::EntityField(FieldRef::new("entity", "missing-binding")),
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
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "ghost", "value",
                    ))]
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
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity", "zero",
                    ))]
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
    fixed_validation_record(
        &report,
        VALIDATION_ACCUMULATION_COUNT,
        VALIDATION_ACCUMULATION_FINGERPRINT,
        31_u64 << 32,
    )
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
    fixed_validation_record(
        &forward,
        VALIDATION_DISJOINT_CYCLE_COUNT,
        VALIDATION_DISJOINT_CYCLE_FINGERPRINT,
        34_u64 << 32,
    )
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
                (FieldId::from("input-stable"), Value::Number(number(2.0))),
                (FieldId::from("output-stable"), Value::Number(number(3.0))),
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
    fixed_validation_record(
        &before,
        VALIDATION_RENAME_COUNT,
        VALIDATION_RENAME_FINGERPRINT,
        32_u64 << 32,
    )
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
                    "entity",
                    "dependent",
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
        && dependency
            .is_some_and(|diagnostic| diagnostic.related_subjects == expected_dependencies);
    if !exact {
        return Record::failure(UNEXPECTED, 33);
    }
    fixed_validation_record(
        &report,
        VALIDATION_CYCLE_COUNT,
        VALIDATION_CYCLE_FINGERPRINT,
        33_u64 << 32,
    )
}

fn analysis_text_field(id: &str, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Text,
        required,
    }
}

fn analysis_number_field(id: &str, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Number,
        required,
    }
}

fn analysis_weapon(
    entity: &str,
    category: &str,
    tier: Option<f64>,
    damage: f64,
    interval: f64,
) -> Entity {
    let mut fields = BTreeMap::from([
        (FieldId::from("category"), Value::Text(category.to_owned())),
        (FieldId::from("damage"), Value::Number(number(damage))),
        (
            FieldId::from("attack_interval"),
            Value::Number(number(interval)),
        ),
        (
            FieldId::from("dps"),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Reference(FieldRef::new(entity, "damage"))),
                right: Box::new(Expression::Reference(FieldRef::new(
                    entity,
                    "attack_interval",
                ))),
            }),
        ),
    ]);
    if let Some(tier) = tier {
        fields.insert(FieldId::from("tier"), Value::Number(number(tier)));
    }
    Entity {
        id: EntityId::from(entity),
        key: EntityKey::from(entity),
        schema: SchemaId::from("analysis-weapons"),
        fields,
    }
}

fn analysis_document() -> Document {
    let schema = SchemaId::from("analysis-weapons");
    Document {
        id: DocumentId::from("portable-analysis-document"),
        title: "Portable analysis conformance".to_owned(),
        schemas: BTreeMap::from([(
            schema.clone(),
            Schema {
                id: schema,
                key: SchemaKey::from("analysis-weapons"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("category"),
                        analysis_text_field("category", true),
                    ),
                    (FieldId::from("tier"), analysis_number_field("tier", false)),
                    (
                        FieldId::from("damage"),
                        analysis_number_field("damage", true),
                    ),
                    (
                        FieldId::from("attack_interval"),
                        analysis_number_field("attack_interval", true),
                    ),
                    (FieldId::from("dps"), analysis_number_field("dps", true)),
                ]),
            },
        )]),
        entities: BTreeMap::from([
            (
                EntityId::from("alpha"),
                analysis_weapon("alpha", "melee", Some(2.0), 50.0, 1.0),
            ),
            (
                EntityId::from("beta"),
                analysis_weapon("beta", "ranged", None, 30.0, 2.0),
            ),
            (
                EntityId::from("gamma"),
                analysis_weapon("gamma", "melee", Some(3.0), 60.0, 1.5),
            ),
        ]),
    }
}

fn analysis_operation_lifecycle(
    document: &Document,
    family: OperationFamily,
) -> Result<(PatchLifecycle, DocumentScopeId, PrincipalId), ()> {
    analysis_operation_lifecycle_named(document, family, "primary")
}

fn analysis_operation_lifecycle_named(
    document: &Document,
    family: OperationFamily,
    name: &str,
) -> Result<(PatchLifecycle, DocumentScopeId, PrincipalId), ()> {
    let scope = DocumentScopeId::from(format!("portable-analysis-{name}-occurrence"));
    let principal = PrincipalId::from(format!("portable-analysis-{name}-principal"));
    let document_subject =
        ScopedSemanticSubject::new(scope.clone(), document.id.clone(), SemanticScope::Document);
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from(format!("portable-analysis-{name}-domain")),
        scope.clone(),
        document.id.clone(),
        SemanticApiContract::from("portable-analysis-api"),
        AuthorizationPolicyVersion::from("portable-analysis-policy-v1"),
        PolicyMeaningId::from("portable-analysis-policy-meaning-v1"),
    );
    lifecycle
        .register_principal(principal.clone(), PrincipalKind::Human)
        .map_err(|_| ())?;
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from(format!("portable-analysis-{name}-query-grant")),
            principal.clone(),
            principal.clone(),
            vec![GrantRequirement::query(family, document_subject)],
            None,
        ))
        .map_err(|_| ())?;
    Ok((lifecycle, scope, principal))
}

fn analysis_success_definition() -> AnalysisDefinition {
    AnalysisDefinition::new(
        SchemaId::from("analysis-weapons"),
        Some(vec![EntityId::from("beta"), EntityId::from("alpha")]),
        vec![AnalysisPredicate::new(
            FieldId::from("tier"),
            AnalysisPredicateOperator::GreaterThanOrEqual,
            PredicateOperand::Number(number(2.0)),
        )],
        None,
        vec![
            AnalysisResultRequest::Observations(FieldId::from("dps")),
            AnalysisResultRequest::Maximum(FieldId::from("dps")),
            AnalysisResultRequest::Minimum(FieldId::from("damage")),
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Membership,
        ],
    )
}

fn analysis_success_record() -> Record {
    let document = analysis_document();
    let definition = analysis_success_definition();
    let Ok(expected_definition) = definition.admit_envelope() else {
        return Record::failure(UNEXPECTED, 51_u64 << 32);
    };
    let Ok((lifecycle, scope, principal)) =
        analysis_operation_lifecycle(&document, OperationFamily::AnalysisQuery)
    else {
        return Record::failure(UNEXPECTED, (51_u64 << 32) | 1);
    };
    let Ok(result) = lifecycle.query_analysis(
        &scope,
        &document,
        (
            &SemanticRevision::from("portable-analysis-r1"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &definition,
        &principal,
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (51_u64 << 32) | 2);
    };
    let expected_values = vec![
        AnalysisResultValue::Membership(vec![EntityId::from("alpha")]),
        AnalysisResultValue::Count(1),
        AnalysisResultValue::Minimum {
            field: FieldId::from("damage"),
            outcome: NumericAggregateOutcome::Value(number(50.0)),
        },
        AnalysisResultValue::Maximum {
            field: FieldId::from("dps"),
            outcome: NumericAggregateOutcome::Value(number(50.0)),
        },
        AnalysisResultValue::Observations {
            field: FieldId::from("dps"),
            values: vec![(EntityId::from("alpha"), number(50.0))],
        },
    ];
    let expected_derivations = vec![
        AnalysisDerivation::Predicate(FieldId::from("tier")),
        AnalysisDerivation::Membership,
        AnalysisDerivation::Count,
        AnalysisDerivation::Minimum(FieldId::from("damage")),
        AnalysisDerivation::Maximum(FieldId::from("dps")),
        AnalysisDerivation::Observations(FieldId::from("dps")),
    ];
    let exact = result.lineage.sources.len() == 1
        && result.lineage.sources[0].document == document.id
        && result.lineage.sources[0].source_revision == SemanticRevision::from("portable-analysis-r1")
        && result.lineage.sources[0].validator_configuration == ValidatorConfiguration::WorkspaceFull
        && result.lineage.normalized_definition == expected_definition
        && result.lineage.formula_calculation_used
        && result.lineage.derivations == expected_derivations
        && matches!(
            &result.outcome,
            AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(AnalysisBucket { values }))
                if *values == expected_values
        );
    if !exact {
        return Record::failure(UNEXPECTED, (51_u64 << 32) | 3);
    }
    Record {
        class: ANALYSIS_COMPLETE,
        bits: number(50.0).to_bits(),
        auxiliary: analysis_result_fingerprint(&result),
    }
}

fn analysis_failure_record() -> Record {
    let mut document = analysis_document();
    document
        .entities
        .get_mut("beta")
        .expect("portable analysis fixture has beta")
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(number(0.0)));
    let definition = AnalysisDefinition::new(
        SchemaId::from("analysis-weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("dps"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(10.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let Ok((lifecycle, scope, principal)) =
        analysis_operation_lifecycle(&document, OperationFamily::AnalysisQuery)
    else {
        return Record::failure(UNEXPECTED, 52_u64 << 32);
    };
    let Ok(result) = lifecycle.query_analysis(
        &scope,
        &document,
        (
            &SemanticRevision::from("portable-analysis-failure-r1"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &definition,
        &principal,
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (52_u64 << 32) | 1);
    };
    let exact = result.lineage.formula_calculation_used
        && matches!(
            &result.outcome,
            AnalysisOutcome::Failure(AnalysisFailure::CalculationFailed {
                field,
                failure: Some(CalculationFailure::DivisionByZero),
            }) if *field == FieldRef::new("beta", "dps")
        );
    if !exact {
        return Record::failure(UNEXPECTED, (52_u64 << 32) | 2);
    }
    Record {
        class: ANALYSIS_FAILURE,
        bits: 0,
        auxiliary: analysis_result_fingerprint(&result),
    }
}

fn analysis_paired_authorization_record() -> Record {
    let mut first = analysis_document();
    first
        .entities
        .retain(|entity, _| entity == &EntityId::from("alpha"));
    let mut second = first.clone();
    second.id = DocumentId::from("comparison-analysis-document");
    second
        .entities
        .get_mut("alpha")
        .expect("portable analysis fixture has alpha")
        .fields
        .insert(FieldId::from("damage"), Value::Number(number(80.0)));
    let definition = AnalysisDefinition::new(
        SchemaId::from("analysis-weapons"),
        None,
        vec![],
        None,
        vec![
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Maximum(FieldId::from("dps")),
        ],
    );
    let Ok(expected_definition) = definition.admit_envelope() else {
        return Record::failure(UNEXPECTED, 53_u64 << 32);
    };
    let Ok((lifecycle, scope, principal)) =
        analysis_operation_lifecycle(&first, OperationFamily::AnalysisQuery)
    else {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 1);
    };
    let Ok((second_lifecycle, second_scope, second_principal)) =
        analysis_operation_lifecycle_named(&second, OperationFamily::AnalysisQuery, "comparison")
    else {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 2);
    };
    let Ok(result) = lifecycle.query_analysis_pair(
        &scope,
        &first,
        (
            &SemanticRevision::from("portable-analysis-pair-r1"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &second_lifecycle,
        &second_scope,
        &second,
        (
            &SemanticRevision::from("portable-analysis-pair-r2"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &definition,
        &principal,
        &second_principal,
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 3);
    };
    let first_outcome = AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(AnalysisBucket {
        values: vec![
            AnalysisResultValue::Count(1),
            AnalysisResultValue::Maximum {
                field: FieldId::from("dps"),
                outcome: NumericAggregateOutcome::Value(number(50.0)),
            },
        ],
    }));
    let second_outcome = AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(AnalysisBucket {
        values: vec![
            AnalysisResultValue::Count(1),
            AnalysisResultValue::Maximum {
                field: FieldId::from("dps"),
                outcome: NumericAggregateOutcome::Value(number(80.0)),
            },
        ],
    }));
    let exact = result.lineage.sources.len() == 2
        && result.lineage.sources[0].document == first.id
        && result.lineage.sources[0].source_revision
            == SemanticRevision::from("portable-analysis-pair-r1")
        && result.lineage.sources[1].document == second.id
        && result.lineage.sources[1].source_revision
            == SemanticRevision::from("portable-analysis-pair-r2")
        && result.lineage.normalized_definition == expected_definition
        && result.lineage.formula_calculation_used
        && result.first == first_outcome
        && result.second == second_outcome
        && result.first != result.second;
    if !exact {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 4);
    }

    let Ok((wrong_family, wrong_scope, wrong_principal)) = analysis_operation_lifecycle_named(
        &second,
        OperationFamily::FormulaReasoning,
        "comparison-denied",
    )
    else {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 5);
    };
    if !matches!(
        lifecycle.query_analysis_pair(
            &scope,
            &first,
            (
                &SemanticRevision::from("portable-analysis-pair-r1"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &wrong_family,
            &wrong_scope,
            &second,
            (
                &SemanticRevision::from("portable-analysis-pair-r2"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &definition,
            &principal,
            &wrong_principal,
            TrustedInstant::new(1),
        ),
        Err(AnalysisOperationError::Lifecycle(PatchLifecycleError::DisclosureDenied))
    ) {
        return Record::failure(UNEXPECTED, (53_u64 << 32) | 6);
    }
    Record {
        class: ANALYSIS_PAIRED_AUTHORIZATION,
        bits: number(80.0).to_bits(),
        auxiliary: analysis_pair_fingerprint(&result),
    }
}

fn analysis_hash_text(hash: &mut u64, value: &str) {
    mix_framed(hash, b"text", value.as_bytes());
}

fn analysis_hash_number(hash: &mut u64, value: Number) {
    mix_framed(hash, b"number", &value.to_bits().to_le_bytes());
}

fn analysis_hash_field_ref(hash: &mut u64, value: &FieldRef) {
    mix_framed(hash, b"field-ref", value.entity.as_str().as_bytes());
    mix_framed(hash, b"field-ref", value.field.as_str().as_bytes());
}

fn analysis_hash_predicate(hash: &mut u64, predicate: &AnalysisPredicate) {
    analysis_hash_text(hash, predicate.field.as_str());
    analysis_hash_text(
        hash,
        match predicate.operator {
            AnalysisPredicateOperator::Equal => "equal",
            AnalysisPredicateOperator::NotEqual => "not-equal",
            AnalysisPredicateOperator::LessThan => "less-than",
            AnalysisPredicateOperator::LessThanOrEqual => "less-than-or-equal",
            AnalysisPredicateOperator::GreaterThan => "greater-than",
            AnalysisPredicateOperator::GreaterThanOrEqual => "greater-than-or-equal",
        },
    );
    match &predicate.operand {
        PredicateOperand::Number(value) => analysis_hash_number(hash, *value),
        PredicateOperand::Text(value) => analysis_hash_text(hash, value),
        PredicateOperand::Boolean(value) => mix_framed(hash, b"boolean", &[*value as u8]),
        PredicateOperand::Reference(value) => analysis_hash_text(hash, value.as_str()),
    }
}

fn analysis_hash_result_request(hash: &mut u64, request: &AnalysisResultRequest) {
    match request {
        AnalysisResultRequest::Membership => analysis_hash_text(hash, "membership"),
        AnalysisResultRequest::Count => analysis_hash_text(hash, "count"),
        AnalysisResultRequest::Minimum(field) => {
            analysis_hash_text(hash, "minimum");
            analysis_hash_text(hash, field.as_str());
        }
        AnalysisResultRequest::Maximum(field) => {
            analysis_hash_text(hash, "maximum");
            analysis_hash_text(hash, field.as_str());
        }
        AnalysisResultRequest::Observations(field) => {
            analysis_hash_text(hash, "observations");
            analysis_hash_text(hash, field.as_str());
        }
    }
}

fn analysis_hash_lineage(hash: &mut u64, lineage: &AnalysisLineage) {
    for source in &lineage.sources {
        analysis_hash_text(hash, source.document.as_str());
        analysis_hash_text(hash, source.source_revision.as_str());
        analysis_hash_text(
            hash,
            match source.validator_configuration {
                ValidatorConfiguration::WorkspaceFull => "workspace-full",
            },
        );
    }
    analysis_hash_text(hash, lineage.normalized_definition.schema.as_str());
    match &lineage.normalized_definition.narrowing {
        Some(entities) => {
            analysis_hash_text(hash, "explicit-narrowing");
            for entity in entities {
                analysis_hash_text(hash, entity.as_str());
            }
        }
        None => analysis_hash_text(hash, "unbounded-domain"),
    }
    for predicate in &lineage.normalized_definition.predicates {
        analysis_hash_predicate(hash, predicate);
    }
    match &lineage.normalized_definition.group_by {
        Some(field) => analysis_hash_text(hash, field.as_str()),
        None => analysis_hash_text(hash, "ungrouped"),
    }
    for request in &lineage.normalized_definition.results {
        analysis_hash_result_request(hash, request);
    }
    mix_framed(hash, b"formula-calculation-used", &[lineage.formula_calculation_used as u8]);
    for derivation in &lineage.derivations {
        match derivation {
            AnalysisDerivation::Predicate(field) => {
                analysis_hash_text(hash, "predicate");
                analysis_hash_text(hash, field.as_str());
            }
            AnalysisDerivation::GroupedBy(field) => {
                analysis_hash_text(hash, "grouped-by");
                analysis_hash_text(hash, field.as_str());
            }
            AnalysisDerivation::Membership => analysis_hash_text(hash, "membership"),
            AnalysisDerivation::Count => analysis_hash_text(hash, "count"),
            AnalysisDerivation::Minimum(field) => {
                analysis_hash_text(hash, "minimum");
                analysis_hash_text(hash, field.as_str());
            }
            AnalysisDerivation::Maximum(field) => {
                analysis_hash_text(hash, "maximum");
                analysis_hash_text(hash, field.as_str());
            }
            AnalysisDerivation::Observations(field) => {
                analysis_hash_text(hash, "observations");
                analysis_hash_text(hash, field.as_str());
            }
        }
    }
}

fn analysis_hash_numeric_aggregate(hash: &mut u64, outcome: &NumericAggregateOutcome) {
    match outcome {
        NumericAggregateOutcome::Value(value) => analysis_hash_number(hash, *value),
        NumericAggregateOutcome::Empty => analysis_hash_text(hash, "empty"),
    }
}

fn analysis_hash_bucket(hash: &mut u64, bucket: &AnalysisBucket) {
    for value in &bucket.values {
        match value {
            AnalysisResultValue::Membership(members) => {
                analysis_hash_text(hash, "membership");
                for member in members {
                    analysis_hash_text(hash, member.as_str());
                }
            }
            AnalysisResultValue::Count(count) => {
                analysis_hash_text(hash, "count");
                mix_framed(hash, b"count", &count.to_le_bytes());
            }
            AnalysisResultValue::Minimum { field, outcome } => {
                analysis_hash_text(hash, "minimum");
                analysis_hash_text(hash, field.as_str());
                analysis_hash_numeric_aggregate(hash, outcome);
            }
            AnalysisResultValue::Maximum { field, outcome } => {
                analysis_hash_text(hash, "maximum");
                analysis_hash_text(hash, field.as_str());
                analysis_hash_numeric_aggregate(hash, outcome);
            }
            AnalysisResultValue::Observations { field, values } => {
                analysis_hash_text(hash, "observations");
                analysis_hash_text(hash, field.as_str());
                for (entity, value) in values {
                    analysis_hash_text(hash, entity.as_str());
                    analysis_hash_number(hash, *value);
                }
            }
        }
    }
}

fn analysis_hash_calculation_failure(hash: &mut u64, failure: &CalculationFailure) {
    match failure {
        CalculationFailure::InvalidExpression { error } => {
            analysis_hash_text(hash, "invalid-expression");
            analysis_hash_text(hash, &error.to_string());
        }
        CalculationFailure::InvalidReferences { targets } => {
            analysis_hash_text(hash, "invalid-references");
            for (field, failure) in targets {
                analysis_hash_field_ref(hash, field);
                analysis_hash_text(
                    hash,
                    match failure {
                        ReferenceFailure::Missing => "missing",
                        ReferenceFailure::NonNumeric => "non-numeric",
                    },
                );
            }
        }
        CalculationFailure::Cycle { members } => {
            analysis_hash_text(hash, "cycle");
            for field in members {
                analysis_hash_field_ref(hash, field);
            }
        }
        CalculationFailure::FailedDependencies { dependencies } => {
            analysis_hash_text(hash, "failed-dependencies");
            for field in dependencies {
                analysis_hash_field_ref(hash, field);
            }
        }
        CalculationFailure::DivisionByZero => analysis_hash_text(hash, "division-by-zero"),
        CalculationFailure::NonFiniteResult => analysis_hash_text(hash, "non-finite-result"),
    }
}

fn analysis_hash_outcome(hash: &mut u64, outcome: &AnalysisOutcome) {
    match outcome {
        AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(bucket)) => {
            analysis_hash_text(hash, "complete-ungrouped");
            analysis_hash_bucket(hash, bucket);
        }
        AnalysisOutcome::Complete(AnalysisProjection::Grouped(groups)) => {
            analysis_hash_text(hash, "complete-grouped");
            for AnalysisGroup { key, bucket } in groups {
                match key {
                    AnalysisGroupKey::Number(value) => analysis_hash_number(hash, *value),
                    AnalysisGroupKey::Text(value) => analysis_hash_text(hash, value),
                    AnalysisGroupKey::Boolean(value) => {
                        mix_framed(hash, b"boolean", &[*value as u8]);
                    }
                    AnalysisGroupKey::Reference(value) => analysis_hash_text(hash, value.as_str()),
                }
                analysis_hash_bucket(hash, bucket);
            }
        }
        AnalysisOutcome::Failure(failure) => analysis_hash_failure(hash, failure),
    }
}

fn analysis_hash_failure(hash: &mut u64, failure: &AnalysisFailure) {
    match failure {
        AnalysisFailure::UnresolvedSchema { schema } => {
            analysis_hash_text(hash, "unresolved-schema");
            analysis_hash_text(hash, schema.as_str());
        }
        AnalysisFailure::UnresolvedField { role, field } => {
            analysis_hash_text(hash, "unresolved-field");
            analysis_hash_text(
                hash,
                match role {
                    AnalysisFieldRole::Predicate => "predicate",
                    AnalysisFieldRole::Group => "group",
                    AnalysisFieldRole::Metric => "metric",
                },
            );
            analysis_hash_text(hash, field.as_str());
        }
        AnalysisFailure::UnresolvedNarrowingEntity { entity } => {
            analysis_hash_text(hash, "unresolved-narrowing-entity");
            analysis_hash_text(hash, entity.as_str());
        }
        AnalysisFailure::WrongDomainNarrowingEntity {
            entity,
            expected,
            actual,
        } => {
            analysis_hash_text(hash, "wrong-domain-narrowing-entity");
            analysis_hash_text(hash, entity.as_str());
            analysis_hash_text(hash, expected.as_str());
            analysis_hash_text(hash, actual.as_str());
        }
        AnalysisFailure::IncoherentCandidateIdentity { key, entity } => {
            analysis_hash_text(hash, "incoherent-candidate-identity");
            analysis_hash_text(hash, key.as_str());
            analysis_hash_text(hash, entity.as_str());
        }
        AnalysisFailure::InvalidPredicateType { field, declared }
        | AnalysisFailure::InvalidMetricType { field, declared } => {
            analysis_hash_text(hash, "invalid-declared-type");
            analysis_hash_text(hash, field.as_str());
            analysis_hash_text(
                hash,
                match declared {
                    FieldType::Number => "number",
                    FieldType::Text => "text",
                    FieldType::Boolean => "boolean",
                    FieldType::Reference { schema } => schema.as_str(),
                },
            );
        }
        AnalysisFailure::InvalidPredicateValue {
            entity,
            field,
            actual,
        }
        | AnalysisFailure::InvalidGroupValue {
            entity,
            field,
            actual,
        } => {
            analysis_hash_text(hash, "invalid-value");
            analysis_hash_text(hash, entity.as_str());
            analysis_hash_text(hash, field.as_str());
            analysis_hash_value_kind(hash, actual);
        }
        AnalysisFailure::MissingGroupValue { entity, field } => {
            analysis_hash_text(hash, "missing-group-value");
            analysis_hash_text(hash, entity.as_str());
            analysis_hash_text(hash, field.as_str());
        }
        AnalysisFailure::FormulaGroupingUnsupported { field } => {
            analysis_hash_text(hash, "formula-grouping-unsupported");
            analysis_hash_text(hash, field.as_str());
        }
        AnalysisFailure::CalculationFailed { field, failure } => {
            analysis_hash_text(hash, "calculation-failed");
            analysis_hash_field_ref(hash, field);
            match failure {
                Some(failure) => analysis_hash_calculation_failure(hash, failure),
                None => analysis_hash_text(hash, "unavailable"),
            }
        }
        AnalysisFailure::MetricIncomplete {
            entity,
            field,
            reason,
        } => {
            analysis_hash_text(hash, "metric-incomplete");
            analysis_hash_text(hash, entity.as_str());
            analysis_hash_text(hash, field.as_str());
            match reason {
                MetricIncompleteReason::Missing => analysis_hash_text(hash, "missing"),
                MetricIncompleteReason::WrongKind(kind) => analysis_hash_value_kind(hash, kind),
            }
        }
        AnalysisFailure::ResultTooLarge { collection, limit } => {
            analysis_hash_text(hash, "result-too-large");
            analysis_hash_text(
                hash,
                match collection {
                    AnalysisCollectionKind::Membership => "membership",
                    AnalysisCollectionKind::Groups => "groups",
                    AnalysisCollectionKind::Observations => "observations",
                },
            );
            mix_framed(hash, b"limit", &limit.to_le_bytes());
        }
    }
}

fn analysis_hash_value_kind(hash: &mut u64, kind: &AnalysisValueKind) {
    analysis_hash_text(
        hash,
        match kind {
            AnalysisValueKind::Number => "number",
            AnalysisValueKind::Formula => "formula",
            AnalysisValueKind::Text => "text",
            AnalysisValueKind::Boolean => "boolean",
            AnalysisValueKind::Reference => "reference",
        },
    );
}

fn analysis_result_fingerprint(result: &AnalysisQueryResult) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    mix_framed(
        &mut hash,
        b"record-domain",
        b"tachiko.portable-conformance/analysis-result/fnv1a64",
    );
    analysis_hash_lineage(&mut hash, &result.lineage);
    analysis_hash_outcome(&mut hash, &result.outcome);
    hash
}

fn analysis_pair_fingerprint(result: &PairedAnalysisQueryResult) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    mix_framed(
        &mut hash,
        b"record-domain",
        b"tachiko.portable-conformance/analysis-pair/fnv1a64",
    );
    analysis_hash_lineage(&mut hash, &result.lineage);
    analysis_hash_outcome(&mut hash, &result.first);
    analysis_hash_outcome(&mut hash, &result.second);
    analysis_hash_text(
        &mut hash,
        "independent-second-family-denied-analysis-pair",
    );
    hash
}

fn formula_operation_lifecycle(
    document: &Document,
    family: OperationFamily,
    mutation_class: MutationClass,
    mutation_actions: &[AuthorizationAction],
) -> Result<(PatchLifecycle, DocumentScopeId, PrincipalId), ()> {
    let scope = DocumentScopeId::from("portable-document-occurrence");
    let principal = PrincipalId::from("portable-human");
    let document_subject =
        ScopedSemanticSubject::new(scope.clone(), document.id.clone(), SemanticScope::Document);
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from("portable-domain"),
        scope.clone(),
        document.id.clone(),
        SemanticApiContract::from("portable-semantic-api"),
        AuthorizationPolicyVersion::from("portable-policy-v1"),
        PolicyMeaningId::from("portable-policy-meaning-v1"),
    );
    lifecycle
        .register_principal(principal.clone(), PrincipalKind::Human)
        .map_err(|_| ())?;
    let mut requirements = vec![GrantRequirement::query(family, document_subject.clone())];
    for action in mutation_actions {
        requirements.push(
            GrantRequirement::mutation(
                *action,
                family,
                mutation_class,
                document_subject.clone(),
            )
            .map_err(|_| ())?,
        );
    }
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("portable-formula-grant"),
            principal.clone(),
            principal.clone(),
            requirements,
            None,
        ))
        .map_err(|_| ())?;
    Ok((lifecycle, scope, principal))
}

fn formula_query_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let Ok((lifecycle, scope, principal)) =
        formula_operation_lifecycle(
            &document,
            OperationFamily::FormulaReasoning,
            MutationClass::Formula,
            &[],
        )
    else {
        return Record::failure(UNEXPECTED, 48_u64 << 32);
    };
    let Ok(result) = lifecycle.query_formula_reasoning(
        &scope,
        &document,
        (
            &SemanticRevision::from("portable-r1"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &FieldRef::new("entity-stable", "output-stable"),
        &principal,
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (48_u64 << 32) | 1);
    };
    let FormulaReasoningOutcome::Formula(facts) = result.outcome else {
        return Record::failure(UNEXPECTED, (48_u64 << 32) | 2);
    };
    let FormulaCalculationOutcome::Value(value) = facts.calculation else {
        return Record::failure(UNEXPECTED, (48_u64 << 32) | 3);
    };
    if facts.expression != input_reference()
        || facts.direct_inputs != [FieldRef::new("entity-stable", "input-stable")]
        || !facts.direct_dependents.is_empty()
        || !facts.affected_subjects.is_empty()
        || facts
            .validation_report
            .is_none_or(|report| !report.is_valid())
    {
        return Record::failure(UNEXPECTED, (48_u64 << 32) | 4);
    }
    Record::value(value, facts.direct_inputs.len() as u64)
}

fn formula_scenario_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let original = document.clone();
    let Ok((lifecycle, scope, principal)) =
        formula_operation_lifecycle(
            &document,
            OperationFamily::NumberOverrideScenario,
            MutationClass::Formula,
            &[],
        )
    else {
        return Record::failure(UNEXPECTED, 49_u64 << 32);
    };
    let request = ScenarioRequest::new(
        vec![NumberOverride::new(
            FieldRef::new("entity-stable", "input-stable"),
            5.0,
        )],
        vec![FieldRef::new("entity-stable", "output-stable")],
    );
    let Ok(result) = lifecycle.query_number_override_scenario(
        &scope,
        &document,
        (
            &SemanticRevision::from("portable-r1"),
            ValidatorConfiguration::WorkspaceFull,
        ),
        &request,
        &principal,
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 1);
    };
    let ScenarioOutcome::Evaluated(evaluation) = result.outcome else {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 2);
    };
    let Some(target) = evaluation.targets.first() else {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 3);
    };
    let ScenarioTargetOutcome::Formula(comparison) = &target.outcome else {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 4);
    };
    let (FormulaCalculationOutcome::Value(baseline), FormulaCalculationOutcome::Value(candidate)) =
        (&comparison.baseline, &comparison.candidate)
    else {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 5);
    };
    let reports_valid = evaluation
        .baseline_validation
        .as_ref()
        .is_some_and(ValidationReport::is_valid)
        && evaluation
            .candidate_validation
            .as_ref()
            .is_some_and(ValidationReport::is_valid);
    if document != original
        || *baseline != number(42.0)
        || *candidate != number(5.0)
        || result.normalized_overrides.len() != 1
        || !reports_valid
    {
        return Record::failure(UNEXPECTED, (49_u64 << 32) | 6);
    }
    Record::value(*candidate, baseline.to_bits())
}

struct PortablePublication {
    scope: DocumentScopeId,
    document: Document,
    revision: SemanticRevision,
}

impl SemanticPublicationAuthority for PortablePublication {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        (
            self.scope.clone(),
            self.document.clone(),
            self.revision.clone(),
        )
    }

    fn publish_if_current<Authorization>(
        &mut self,
        expected_document_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<
        (DocumentScopeId, Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    > {
        if expected_document_scope != &self.scope {
            return Err(SemanticPublicationError::DocumentScopeMismatch);
        }
        if expected_revision != &self.revision {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(3))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.document = candidate;
        self.revision = SemanticRevision::from("portable-r2");
        Ok((
            self.scope.clone(),
            self.document.clone(),
            self.revision.clone(),
            authorization,
        ))
    }
}

fn formula_update_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let original = document.clone();
    let Ok((mut lifecycle, scope, principal)) = formula_operation_lifecycle(
        &document,
        OperationFamily::FormulaUpdate,
        MutationClass::Formula,
        &[AuthorizationAction::Propose, AuthorizationAction::Execute],
    ) else {
        return Record::failure(UNEXPECTED, 50_u64 << 32);
    };
    let revision = SemanticRevision::from("portable-r1");
    let Ok(patch) = lifecycle.propose_formula_update(
        &scope,
        &document,
        &revision,
        FormulaUpdateRequest::new(
            ProposalId::from("portable-formula-update"),
            revision.clone(),
            FieldRef::new("entity-stable", "output-stable"),
            "[source.input-stable] + 1",
            principal.clone(),
        ),
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 1);
    };
    let SemanticPatchBody::Command(SemanticCommand::FormulaUpdate(command)) =
        patch.exact_change().body()
    else {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 2);
    };
    if command.target() != &FieldRef::new("entity-stable", "output-stable")
        || command.references() != &BTreeSet::from([FieldRef::new("entity-stable", "input-stable")])
    {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 3);
    }
    let mut publication = PortablePublication {
        scope,
        document: document.clone(),
        revision,
    };
    let Ok(receipt) = lifecycle.execute(
        patch.id(),
        None,
        &principal,
        &mut publication,
        TrustedInstant::new(2),
    ) else {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 4);
    };
    let outcome = calculate_complete(&publication.document);
    let CalculationOutcome::Complete(calculation) = outcome else {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 5);
    };
    let Some(value) = calculation.value(&FieldRef::new("entity-stable", "output-stable")) else {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 6);
    };
    if document != original || !receipt.verified || value != number(43.0) {
        return Record::failure(UNEXPECTED, (50_u64 << 32) | 7);
    }
    Record::value(value, receipt.semantic_changes.len() as u64)
}

struct PortableTrustedTime {
    calls: u64,
}

impl TrustedPublicationTimeSource for PortableTrustedTime {
    fn now(&mut self) -> TrustedInstant {
        self.calls += 1;
        TrustedInstant::new(self.calls + 2)
    }
}

fn resident_session_record() -> Record {
    let document = formula_document(number(42.0), input_reference());
    let Ok((mut lifecycle, scope, principal)) = formula_operation_lifecycle(
        &document,
        OperationFamily::SetFieldValue,
        MutationClass::Value,
        &[AuthorizationAction::Propose, AuthorizationAction::Execute],
    ) else {
        return Record::failure(UNEXPECTED, 54_u64 << 32);
    };
    let mut session = ResidentWorkspaceSession::new(scope, document);
    let initial = session.export_snapshot();
    let input = FieldRef::new("entity-stable", "input-stable");
    let output = FieldRef::new("entity-stable", "output-stable");
    let Ok(query) = session.query_fields(&[output.clone(), input.clone()]) else {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 1);
    };
    let [input_projection, output_projection] = query.value().as_slice() else {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 2);
    };
    if query.revision() != initial.revision()
        || session.revision() != initial.revision()
        || input_projection.field != input
        || input_projection.stored_value != Some(Value::Number(number(42.0)))
        || input_projection.formula_definition.is_some()
        || input_projection.calculated_value.is_some()
        || output_projection.field != output
        || output_projection.stored_value.is_some()
        || output_projection.formula_definition.as_ref() != Some(&input_reference())
        || output_projection.calculated_value
            != Some(FormulaCalculationOutcome::Value(number(42.0)))
        || !input_projection.diagnostics.is_empty()
        || !output_projection.diagnostics.is_empty()
    {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 3);
    }
    let proposal = ProposalId::from("resident-set-input");
    let Ok(patch) = lifecycle.propose(
        initial.document_scope(),
        initial.document(),
        initial.revision(),
        ProposalRequest::new(
            proposal.clone(),
            initial.revision().clone(),
            SemanticPatchBody::command(SemanticCommand::set_field_value(
                input.clone(),
                Value::Number(number(43.0)),
            )),
            principal.clone(),
        ),
        TrustedInstant::new(1),
    ) else {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 4);
    };
    if lifecycle
        .preview(
            initial.document_scope(),
            initial.document(),
            initial.revision(),
            &proposal,
            &principal,
            TrustedInstant::new(2),
        )
        .is_err()
    {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 5);
    };
    let mut time = PortableTrustedTime { calls: 0 };
    let receipt = {
        let mut publication = session.publication_authority(&mut time);
        let Ok(receipt) = lifecycle.execute(
            patch.id(),
            None,
            &principal,
            &mut publication,
            TrustedInstant::new(3),
        ) else {
            return Record::failure(UNEXPECTED, (54_u64 << 32) | 6);
        };
        receipt
    };
    let invalidation = session.projection_invalidation(std::slice::from_ref(&input));
    if invalidation.revision() != &receipt.resulting_revision
        || invalidation.value().changed_fields != [input.clone()]
        || invalidation.value().affected_calculations != [output.clone()]
    {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 7);
    }
    let installed = session.export_snapshot();
    let mut stale_candidate = installed.document().clone();
    stale_candidate.title = "must not install".to_owned();
    let stale = session
        .publication_authority(&mut time)
        .publish_if_current(
            installed.document_scope(),
            initial.revision(),
            stale_candidate,
            |_| Some(()),
        );
    if !matches!(stale, Err(SemanticPublicationError::Stale))
        || session.export_snapshot() != installed
        || receipt.base_revision != *initial.revision()
        || receipt.resulting_revision != *installed.revision()
        || initial.revision() == installed.revision()
        || time.calls != 2
    {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 8);
    }
    let Ok(current) = session.query_fields(std::slice::from_ref(&output)) else {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 9);
    };
    let Some(FormulaCalculationOutcome::Value(value)) = current.value()[0].calculated_value else {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 10);
    };
    if value != number(43.0) || !receipt.verified {
        return Record::failure(UNEXPECTED, (54_u64 << 32) | 11);
    }
    Record {
        class: RESIDENT_SESSION,
        bits: value.to_bits(),
        auxiliary: (time.calls << 32)
            | ((invalidation.value().affected_calculations.len() as u64) << 16)
            | receipt.semantic_changes.len() as u64,
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
        15 => storage_record("0.000001", 0x3eb0_c6f7_a0b5_ed8d, 0x65c8_993e_f682_f106),
        16 => storage_record(
            "999999999999999900000",
            0x444b_1ae4_d6e2_ef4f,
            0x8546_e95c_6c56_79c3,
        ),
        17 => storage_record("1e+21", 0x444b_1ae4_d6e2_ef50, 0xefcf_57a3_ad34_d99d),
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
        46 => roproj_v1_record(),
        47 => portable_package_v1_record(),
        48 => formula_query_record(),
        49 => formula_scenario_record(),
        50 => formula_update_record(),
        51 => analysis_success_record(),
        52 => analysis_failure_record(),
        53 => analysis_paired_authorization_record(),
        54 => resident_session_record(),
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
