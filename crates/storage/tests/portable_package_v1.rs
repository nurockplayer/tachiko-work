use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use tachiko_semantic_core::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Schema,
    SchemaId, SchemaKey, Value,
};
use tachiko_storage::{
    FormatError, PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES, PortablePackageError,
    decode_portable_package_v1, encode_portable_package_v1, encode_roproj_v1, from_bytes,
    portable_package_payload_root, to_canonical_string,
};

const EMPTY_PACKAGE_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/research/fixtures/issue-43-portable-package-v1/empty-package-v1.hex"
));

fn decode_hex(source: &str) -> Vec<u8> {
    let source = source
        .strip_suffix('\n')
        .expect("golden hex must end in one LF");
    assert!(source.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(source.len() % 2, 0);
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).unwrap();
            let low = (pair[1] as char).to_digit(16).unwrap();
            u8::try_from((high << 4) | low).unwrap()
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("mutation target must exist")
}

#[derive(Clone, Debug)]
struct ZipRecord {
    name: String,
    local_offset: usize,
    local_end: usize,
    local_name_start: usize,
    data_start: usize,
    data_end: usize,
    central_offset: usize,
    central_end: usize,
    central_name_start: usize,
}

fn read_u16(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(source[offset..offset + 2].try_into().unwrap())
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap())
}

fn write_u16(destination: &mut [u8], offset: usize, value: u16) {
    destination[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(destination: &mut [u8], offset: usize, value: u32) {
    destination[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn push_u16(destination: &mut Vec<u8>, value: u16) {
    destination.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(destination: &mut Vec<u8>, value: u32) {
    destination.extend_from_slice(&value.to_le_bytes());
}

fn many_entry_future_package(entry_count: u16) -> Vec<u8> {
    assert!(entry_count > 0 && entry_count < u16::MAX);
    let manifest = b"{\"format\":\"tachiko.portable-package\",\"format_version\":2}";
    let mut archive = Vec::new();
    let mut local_offsets = Vec::with_capacity(usize::from(entry_count));

    for index in 0..entry_count {
        let (name, body) = if index == 0 {
            (b"package.json".as_slice(), manifest.as_slice())
        } else {
            (b"x".as_slice(), b"".as_slice())
        };
        local_offsets.push(u32::try_from(archive.len()).unwrap());
        push_u32(&mut archive, 0x0403_4b50);
        push_u16(&mut archive, 10);
        push_u16(&mut archive, 0x0800);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021);
        push_u32(&mut archive, crc32(body));
        push_u32(&mut archive, u32::try_from(body.len()).unwrap());
        push_u32(&mut archive, u32::try_from(body.len()).unwrap());
        push_u16(&mut archive, u16::try_from(name.len()).unwrap());
        push_u16(&mut archive, 0);
        archive.extend_from_slice(name);
        archive.extend_from_slice(body);
    }

    let central_offset = u32::try_from(archive.len()).unwrap();
    for (index, local_offset) in local_offsets.into_iter().enumerate() {
        let (name, body) = if index == 0 {
            (b"package.json".as_slice(), manifest.as_slice())
        } else {
            (b"x".as_slice(), b"".as_slice())
        };
        push_u32(&mut archive, 0x0201_4b50);
        push_u16(&mut archive, 20);
        push_u16(&mut archive, 10);
        push_u16(&mut archive, 0x0800);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0x0021);
        push_u32(&mut archive, crc32(body));
        push_u32(&mut archive, u32::try_from(body.len()).unwrap());
        push_u32(&mut archive, u32::try_from(body.len()).unwrap());
        push_u16(&mut archive, u16::try_from(name.len()).unwrap());
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u32(&mut archive, 0);
        push_u32(&mut archive, local_offset);
        archive.extend_from_slice(name);
    }
    let central_size = u32::try_from(archive.len()).unwrap() - central_offset;
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, entry_count);
    push_u16(&mut archive, entry_count);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    archive
}

fn zip_records(source: &[u8]) -> Vec<ZipRecord> {
    let end = source.len() - 22;
    let count = usize::from(read_u16(source, end + 10));
    let central_start = usize::try_from(read_u32(source, end + 16)).unwrap();
    let mut locals = Vec::with_capacity(count);
    let mut offset = 0;
    for _ in 0..count {
        assert_eq!(read_u32(source, offset), 0x0403_4b50);
        let name_length = usize::from(read_u16(source, offset + 26));
        let extra_length = usize::from(read_u16(source, offset + 28));
        let size = usize::try_from(read_u32(source, offset + 18)).unwrap();
        let name_start = offset + 30;
        let data_start = name_start + name_length + extra_length;
        let end = data_start + size;
        locals.push((offset, end, name_start, data_start, end));
        offset = end;
    }
    assert_eq!(offset, central_start);

    let mut records = Vec::with_capacity(count);
    let mut central_offset = central_start;
    for (local_offset, local_end, local_name_start, data_start, data_end) in locals {
        assert_eq!(read_u32(source, central_offset), 0x0201_4b50);
        let name_length = usize::from(read_u16(source, central_offset + 28));
        let extra_length = usize::from(read_u16(source, central_offset + 30));
        let comment_length = usize::from(read_u16(source, central_offset + 32));
        let central_name_start = central_offset + 46;
        let central_end = central_name_start + name_length + extra_length + comment_length;
        records.push(ZipRecord {
            name: std::str::from_utf8(&source[local_name_start..local_name_start + name_length])
                .unwrap()
                .to_owned(),
            local_offset,
            local_end,
            local_name_start,
            data_start,
            data_end,
            central_offset,
            central_end,
            central_name_start,
        });
        central_offset = central_end;
    }
    assert_eq!(central_offset, end);
    records
}

fn record(source: &[u8], name: &str) -> ZipRecord {
    zip_records(source)
        .into_iter()
        .find(|record| record.name == name)
        .unwrap_or_else(|| panic!("missing ZIP record {name}"))
}

fn crc32(source: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in source {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn refresh_record_crc(package: &mut [u8], name: &str) {
    let record = record(package, name);
    let crc = crc32(&package[record.data_start..record.data_end]);
    write_u32(package, record.local_offset + 14, crc);
    write_u32(package, record.central_offset + 16, crc);
}

fn shift_offset(offset: usize, old_length: usize, new_length: usize) -> usize {
    if new_length >= old_length {
        offset.checked_add(new_length - old_length).unwrap()
    } else {
        offset.checked_sub(old_length - new_length).unwrap()
    }
}

fn replace_entry_body(package: &[u8], name: &str, body: &[u8]) -> Vec<u8> {
    let records = zip_records(package);
    let target = records.iter().find(|record| record.name == name).unwrap();
    let old_length = target.data_end - target.data_start;
    let new_length = body.len();
    let size = u32::try_from(new_length).unwrap();
    let crc = crc32(body);
    let old_end = package.len() - 22;
    let old_central_offset = usize::try_from(read_u32(package, old_end + 16)).unwrap();
    let mut output = package.to_vec();
    output.splice(target.data_start..target.data_end, body.iter().copied());

    write_u32(&mut output, target.local_offset + 14, crc);
    write_u32(&mut output, target.local_offset + 18, size);
    write_u32(&mut output, target.local_offset + 22, size);
    for record in &records {
        let central_offset = shift_offset(record.central_offset, old_length, new_length);
        let local_offset = if record.local_offset > target.local_offset {
            shift_offset(record.local_offset, old_length, new_length)
        } else {
            record.local_offset
        };
        write_u32(
            &mut output,
            central_offset + 42,
            u32::try_from(local_offset).unwrap(),
        );
        if record.name == name {
            write_u32(&mut output, central_offset + 16, crc);
            write_u32(&mut output, central_offset + 20, size);
            write_u32(&mut output, central_offset + 24, size);
        }
    }
    let end = shift_offset(old_end, old_length, new_length);
    write_u32(
        &mut output,
        end + 16,
        u32::try_from(shift_offset(old_central_offset, old_length, new_length)).unwrap(),
    );
    output
}

fn package_payload_root(package: &[u8]) -> [u8; 32] {
    let records = zip_records(package);
    let mut root = Sha256::new();
    root.update(b"tachiko.portable-package/v1\0tachiko.roproj/v1\0");
    for path in tachiko_storage::ROPROJ_V1_PATHS {
        let record = records
            .iter()
            .find(|record| record.name == format!("payload/{path}"))
            .unwrap();
        let mut leaf = Sha256::new();
        leaf.update(path.as_bytes());
        leaf.update([0]);
        leaf.update(&package[record.data_start..record.data_end]);
        root.update(leaf.finalize());
    }
    root.finalize().into()
}

fn refresh_payload_integrity(package: &mut [u8], changed_entry: &str) {
    refresh_record_crc(package, changed_entry);
    let root = encode_hex(&package_payload_root(package));
    let manifest = record(package, "package.json");
    let relative = find_bytes(
        &package[manifest.data_start..manifest.data_end],
        b"\"payload_root_sha256\": \"",
    ) + b"\"payload_root_sha256\": \"".len();
    let root_start = manifest.data_start + relative;
    package[root_start..root_start + root.len()].copy_from_slice(root.as_bytes());
    refresh_record_crc(package, "package.json");
}

fn replace_entry_name(package: &mut [u8], old: &str, new: &str) {
    assert_eq!(old.len(), new.len());
    let record = record(package, old);
    package[record.local_name_start..record.local_name_start + old.len()]
        .copy_from_slice(new.as_bytes());
    package[record.central_name_start..record.central_name_start + old.len()]
        .copy_from_slice(new.as_bytes());
}

fn remove_last_entry(package: &[u8]) -> Vec<u8> {
    let target = record(package, "payload/entities/f.jsonl");
    let old_end = package.len() - 22;
    let old_central_offset = usize::try_from(read_u32(package, old_end + 16)).unwrap();
    let old_central_size = usize::try_from(read_u32(package, old_end + 12)).unwrap();
    let local_length = target.local_end - target.local_offset;
    let central_length = target.central_end - target.central_offset;
    let mut output = package.to_vec();
    output.drain(target.central_offset..target.central_end);
    output.drain(target.local_offset..target.local_end);
    let end = output.len() - 22;
    write_u16(&mut output, end + 8, 18);
    write_u16(&mut output, end + 10, 18);
    write_u32(
        &mut output,
        end + 12,
        u32::try_from(old_central_size - central_length).unwrap(),
    );
    write_u32(
        &mut output,
        end + 16,
        u32::try_from(old_central_offset - local_length).unwrap(),
    );
    output
}

fn duplicate_last_entry(package: &[u8]) -> Vec<u8> {
    let target = record(package, "payload/entities/f.jsonl");
    let old_end = package.len() - 22;
    let old_central_offset = usize::try_from(read_u32(package, old_end + 16)).unwrap();
    let old_central_size = usize::try_from(read_u32(package, old_end + 12)).unwrap();
    let local = package[target.local_offset..target.local_end].to_vec();
    let mut central = package[target.central_offset..target.central_end].to_vec();
    write_u32(&mut central, 42, u32::try_from(old_central_offset).unwrap());
    let mut output = Vec::with_capacity(package.len() + local.len() + central.len());
    output.extend_from_slice(&package[..old_central_offset]);
    output.extend_from_slice(&local);
    output.extend_from_slice(&package[old_central_offset..old_end]);
    output.extend_from_slice(&central);
    output.extend_from_slice(&package[old_end..]);
    let end = output.len() - 22;
    write_u16(&mut output, end + 8, 20);
    write_u16(&mut output, end + 10, 20);
    write_u32(
        &mut output,
        end + 12,
        u32::try_from(old_central_size + central.len()).unwrap(),
    );
    write_u32(
        &mut output,
        end + 16,
        u32::try_from(old_central_offset + local.len()).unwrap(),
    );
    output
}

fn add_last_file_comment(package: &[u8]) -> Vec<u8> {
    let target = record(package, "payload/entities/f.jsonl");
    let old_end = package.len() - 22;
    assert_eq!(target.central_end, old_end);
    let old_central_size = read_u32(package, old_end + 12);
    let mut output = package.to_vec();
    output.insert(old_end, b'x');
    write_u16(&mut output, target.central_offset + 32, 1);
    let end = output.len() - 22;
    write_u32(&mut output, end + 12, old_central_size + 1);
    output
}

fn add_last_data_descriptor(package: &[u8]) -> Vec<u8> {
    let target = record(package, "payload/entities/f.jsonl");
    let old_end = package.len() - 22;
    let old_central_offset = usize::try_from(read_u32(package, old_end + 16)).unwrap();
    assert_eq!(target.local_end, old_central_offset);
    let descriptor = [0x50, 0x4b, 0x07, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut output = Vec::with_capacity(package.len() + descriptor.len());
    output.extend_from_slice(&package[..old_central_offset]);
    output.extend_from_slice(&descriptor);
    output.extend_from_slice(&package[old_central_offset..]);
    write_u16(&mut output, target.local_offset + 6, 0x0808);
    write_u16(
        &mut output,
        target.central_offset + descriptor.len() + 8,
        0x0808,
    );
    let end = output.len() - 22;
    write_u32(
        &mut output,
        end + 16,
        u32::try_from(old_central_offset + descriptor.len()).unwrap(),
    );
    output
}

fn reference_document() -> Document {
    let target_schema = SchemaId::from("schema-target");
    let owner_schema = SchemaId::from("schema-owner");
    let reference_field = FieldId::from("field-reference");
    Document {
        id: "doc-reference".into(),
        title: "References".to_owned(),
        schemas: BTreeMap::from([
            (
                target_schema.clone(),
                Schema {
                    id: target_schema.clone(),
                    key: SchemaKey::from("target"),
                    fields: BTreeMap::new(),
                },
            ),
            (
                owner_schema.clone(),
                Schema {
                    id: owner_schema.clone(),
                    key: SchemaKey::from("owner"),
                    fields: BTreeMap::from([(
                        reference_field.clone(),
                        FieldDefinition {
                            id: reference_field.clone(),
                            key: FieldKey::from("reference"),
                            field_type: FieldType::Reference {
                                schema: target_schema.clone(),
                            },
                            required: true,
                        },
                    )]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("target-a"),
                Entity {
                    id: EntityId::from("target-a"),
                    key: EntityKey::from("target"),
                    schema: target_schema,
                    fields: BTreeMap::new(),
                },
            ),
            (
                EntityId::from("owner-aa"),
                Entity {
                    id: EntityId::from("owner-aa"),
                    key: EntityKey::from("owner"),
                    schema: owner_schema,
                    fields: BTreeMap::from([(
                        reference_field,
                        Value::Reference(EntityId::from("target-a")),
                    )]),
                },
            ),
        ]),
    }
}

#[test]
fn empty_project_matches_the_normative_package_vector() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    let package = encode_portable_package_v1(&tree).unwrap();
    let golden = decode_hex(EMPTY_PACKAGE_HEX);

    assert_eq!(package, golden);
    assert_eq!(package.len(), 2_692);
    assert_eq!(
        encode_hex(&Sha256::digest(&package)),
        "1368ebe38c86de28d2379ae6c0ca7a5ca8502543002fe084e33254ad1db4d7bc"
    );
    assert_eq!(
        encode_hex(&portable_package_payload_root(&tree)),
        "71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c"
    );
}

#[test]
fn package_round_trip_is_exact_in_both_directions() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    let first = encode_portable_package_v1(&tree).unwrap();
    let second = encode_portable_package_v1(&tree).unwrap();
    assert_eq!(first, second);

    let verified = decode_portable_package_v1(&first).unwrap();
    assert_eq!(verified.tree(), &tree);
    assert_eq!(
        verified.payload_root(),
        portable_package_payload_root(&tree)
    );
    assert_eq!(encode_portable_package_v1(verified.tree()).unwrap(), first);
}

#[test]
fn byte_reader_content_frames_packages_without_fallback() {
    let expected = Document::empty("doc-empty", "Empty");
    let package = decode_hex(EMPTY_PACKAGE_HEX);

    assert_eq!(from_bytes(&package).unwrap(), expected);
    assert!(matches!(
        from_bytes(b"PK\x03\x04"),
        Err(FormatError::PortablePackage(
            PortablePackageError::InvalidContainer { .. }
        ))
    ));

    let direct = to_canonical_string(&expected).unwrap();
    assert_eq!(from_bytes(direct.as_bytes()).unwrap(), expected);
}

#[test]
fn unsupported_package_version_wins_before_v1_crc_and_payload_checks() {
    let mut package = decode_hex(EMPTY_PACKAGE_HEX);
    let version = find_bytes(&package, b"\"format_version\": 1");
    package[version + b"\"format_version\": ".len()] = b'2';
    let payload = find_bytes(&package, b"\"id\": \"doc-empty\"");
    package[payload + b"\"id\": \"".len()] ^= 1;

    assert!(matches!(
        decode_portable_package_v1(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::UnsupportedVersion { ref found }
        )) if found == "2"
    ));
    assert!(matches!(
        from_bytes(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::UnsupportedVersion { ref found }
        )) if found == "2"
    ));
}

#[test]
fn unsupported_package_version_wins_over_future_owned_manifest_shape() {
    let package = replace_entry_body(
        &decode_hex(EMPTY_PACKAGE_HEX),
        "package.json",
        b"{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": 2,\n  \"future\": {\"x\": 1, \"x\": 2}\n}\n",
    );

    assert!(matches!(
        decode_portable_package_v1(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::UnsupportedVersion { ref found }
        )) if found == "2"
    ));
}

#[test]
fn unsupported_package_version_wins_over_deep_future_owned_values() {
    let nested = format!(
        "{{\"format\":\"tachiko.portable-package\",\"format_version\":2,\"future\":{}{}}}",
        "[".repeat(140),
        "]".repeat(140)
    );
    let package = replace_entry_body(
        &decode_hex(EMPTY_PACKAGE_HEX),
        "package.json",
        nested.as_bytes(),
    );

    assert!(matches!(
        decode_portable_package_v1(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::UnsupportedVersion { ref found }
        )) if found == "2"
    ));
}

#[test]
fn future_manifest_nesting_limit_is_explicit() {
    let nested = format!(
        "{{\"format\":\"tachiko.portable-package\",\"format_version\":2,\"future\":{}{}}}",
        "[".repeat(300),
        "]".repeat(300)
    );
    let package = replace_entry_body(
        &decode_hex(EMPTY_PACKAGE_HEX),
        "package.json",
        nested.as_bytes(),
    );

    assert!(matches!(
        decode_portable_package_v1(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::ResourceLimit {
                resource: "package.json nesting",
                ..
            }
        ))
    ));
}

#[test]
fn maximum_zip32_entry_count_dispatches_in_linear_work() {
    let package = many_entry_future_package(u16::MAX - 1);
    assert!(package.len() < PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES);

    assert!(matches!(
        decode_portable_package_v1(&package),
        Err(FormatError::PortablePackage(
            PortablePackageError::UnsupportedVersion { ref found }
        )) if found == "2"
    ));
}

#[test]
fn corruption_and_stale_integrity_have_distinct_failures() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);
    let payload = record(&golden, "payload/manifest.json");

    let mut corrupted = golden.clone();
    corrupted[payload.data_start + 1] ^= 1;
    assert!(matches!(
        decode_portable_package_v1(&corrupted),
        Err(FormatError::PortablePackage(
            PortablePackageError::CrcMismatch { .. }
        ))
    ));

    let mut stale_root = golden;
    stale_root[payload.data_start + 1] ^= 1;
    refresh_record_crc(&mut stale_root, "payload/manifest.json");
    assert!(matches!(
        decode_portable_package_v1(&stale_root),
        Err(FormatError::PortablePackage(
            PortablePackageError::IntegrityMismatch
        ))
    ));
}

#[test]
fn v1_manifest_metadata_variations_fail_closed() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);
    for body in [
        b"{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": 1,\n  \"payload_format\": \"tachiko.roproj\",\n  \"payload_format_version\": 1\n}\n".as_slice(),
        b"{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": 1,\n  \"payload_format\": \"tachiko.roproj\",\n  \"payload_format_version\": 1,\n  \"payload_root_sha256\": \"71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c\",\n  \"unknown\": true\n}\n".as_slice(),
        b"{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": \"1\",\n  \"payload_format\": \"tachiko.roproj\",\n  \"payload_format_version\": 1,\n  \"payload_root_sha256\": \"71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c\"\n}\n".as_slice(),
        b"{\n  \"format\": \"tachiko.portable-package\",\n  \"format_version\": 1,\n  \"format_version\": 1,\n  \"payload_format\": \"tachiko.roproj\",\n  \"payload_format_version\": 1,\n  \"payload_root_sha256\": \"71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c\"\n}\n".as_slice(),
    ] {
        let package = replace_entry_body(&golden, "package.json", body);
        assert!(matches!(
            decode_portable_package_v1(&package),
            Err(FormatError::PortablePackage(
                PortablePackageError::InvalidManifest { .. }
            ))
        ));
    }
}

#[test]
fn entry_set_variations_fail_closed() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);

    let mut unknown = golden.clone();
    replace_entry_name(
        &mut unknown,
        "payload/entities/f.jsonl",
        "payload/entities/x.jsonl",
    );
    let mut duplicate = golden.clone();
    replace_entry_name(
        &mut duplicate,
        "payload/entities/f.jsonl",
        "payload/entities/e.jsonl",
    );
    let mut aliased = golden.clone();
    replace_entry_name(
        &mut aliased,
        "payload/entities/f.jsonl",
        "payload\\entities/f.jsonl",
    );
    let mut explicit_directory = golden.clone();
    replace_entry_name(
        &mut explicit_directory,
        "payload/entities/f.jsonl",
        "payload/entities/folder/",
    );
    for package in [
        unknown,
        duplicate,
        aliased,
        explicit_directory,
        remove_last_entry(&golden),
        duplicate_last_entry(&golden),
    ] {
        assert!(matches!(
            decode_portable_package_v1(&package),
            Err(FormatError::PortablePackage(
                PortablePackageError::EntrySetMismatch { .. }
            ))
        ));
    }
}

#[test]
fn noncanonical_zip_metadata_order_and_record_disagreement_are_rejected() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);
    let payload = record(&golden, "payload/entities/0.jsonl");

    let mut metadata = golden.clone();
    write_u16(&mut metadata, payload.local_offset + 6, 0x0802);
    assert!(matches!(
        decode_portable_package_v1(&metadata),
        Err(FormatError::PortablePackage(
            PortablePackageError::NonCanonicalContainer { .. }
        ))
    ));

    let mut reordered = golden.clone();
    replace_entry_name(
        &mut reordered,
        "payload/entities/a.jsonl",
        "payload/entities/z.jsonl",
    );
    replace_entry_name(
        &mut reordered,
        "payload/entities/b.jsonl",
        "payload/entities/a.jsonl",
    );
    replace_entry_name(
        &mut reordered,
        "payload/entities/z.jsonl",
        "payload/entities/b.jsonl",
    );
    assert!(matches!(
        decode_portable_package_v1(&reordered),
        Err(FormatError::PortablePackage(
            PortablePackageError::NonCanonicalContainer { .. }
        ))
    ));

    let mut disagreement = golden.clone();
    let crc = read_u32(&disagreement, payload.central_offset + 16);
    write_u32(&mut disagreement, payload.central_offset + 16, crc ^ 1);
    let mut offset_disagreement = golden;
    write_u32(&mut offset_disagreement, payload.central_offset + 42, 0);
    let mut local_size_disagreement = decode_hex(EMPTY_PACKAGE_HEX);
    let final_shard = record(&local_size_disagreement, "payload/entities/f.jsonl");
    write_u32(
        &mut local_size_disagreement,
        final_shard.local_offset + 18,
        1,
    );
    let mut central_size_disagreement = decode_hex(EMPTY_PACKAGE_HEX);
    let final_shard = record(&central_size_disagreement, "payload/entities/f.jsonl");
    write_u32(
        &mut central_size_disagreement,
        final_shard.central_offset + 20,
        1,
    );
    let embedded_signature = b"xPK\x03\x04payload";
    let mut central_ambiguous = replace_entry_body(
        &decode_hex(EMPTY_PACKAGE_HEX),
        "payload/entities/0.jsonl",
        embedded_signature,
    );
    let shard = record(&central_ambiguous, "payload/entities/0.jsonl");
    write_u32(&mut central_ambiguous, shard.central_offset + 20, 1);
    let mut local_ambiguous = replace_entry_body(
        &decode_hex(EMPTY_PACKAGE_HEX),
        "payload/entities/0.jsonl",
        embedded_signature,
    );
    let shard = record(&local_ambiguous, "payload/entities/0.jsonl");
    write_u32(&mut local_ambiguous, shard.local_offset + 18, 1);
    for package in [
        disagreement,
        offset_disagreement,
        local_size_disagreement,
        central_size_disagreement,
        central_ambiguous,
        local_ambiguous,
    ] {
        assert!(matches!(
            decode_portable_package_v1(&package),
            Err(FormatError::PortablePackage(
                PortablePackageError::NonCanonicalContainer { .. }
            ))
        ));
    }
}

#[test]
fn structural_and_noncanonical_zip_variants_have_distinct_failures() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);
    let mut stubbed = b"stub".to_vec();
    stubbed.extend_from_slice(&golden);
    let mut trailed = golden.clone();
    trailed.push(0);
    let mut commented = golden.clone();
    let end = commented.len() - 22;
    write_u16(&mut commented, end + 20, 1);
    commented.push(b'x');
    let data_descriptor = add_last_data_descriptor(&golden);
    let file_comment = add_last_file_comment(&golden);
    let mut zip64 = golden;
    let end = zip64.len() - 22;
    write_u16(&mut zip64, end + 10, 0xffff);

    for package in [stubbed, trailed, zip64] {
        assert!(matches!(
            decode_portable_package_v1(&package),
            Err(FormatError::PortablePackage(
                PortablePackageError::InvalidContainer { .. }
            ))
        ));
    }

    for package in [commented, data_descriptor, file_comment] {
        assert!(matches!(
            decode_portable_package_v1(&package),
            Err(FormatError::PortablePackage(
                PortablePackageError::NonCanonicalContainer { .. }
            ))
        ));
    }
}

#[test]
fn payload_claim_canonicality_and_semantics_have_distinct_failures() {
    let golden = decode_hex(EMPTY_PACKAGE_HEX);
    let payload = record(&golden, "payload/manifest.json");

    let mut claim_mismatch = golden.clone();
    let format = find_bytes(
        &claim_mismatch[payload.data_start..payload.data_end],
        b"tachiko.roproj",
    );
    claim_mismatch[payload.data_start + format + b"tachiko.".len()] = b'x';
    refresh_payload_integrity(&mut claim_mismatch, "payload/manifest.json");
    assert!(matches!(
        decode_portable_package_v1(&claim_mismatch),
        Err(FormatError::PortablePackage(
            PortablePackageError::PayloadManifestMismatch
        ))
    ));

    let mut noncanonical = golden;
    let indentation = find_bytes(
        &noncanonical[payload.data_start..payload.data_end],
        b"  \"format\"",
    );
    noncanonical[payload.data_start + indentation] = b'\t';
    refresh_payload_integrity(&mut noncanonical, "payload/manifest.json");
    assert!(matches!(
        decode_portable_package_v1(&noncanonical),
        Err(FormatError::PortablePackage(
            PortablePackageError::NonCanonicalPayload { .. }
        ))
    ));

    let tree = encode_roproj_v1(&reference_document()).unwrap();
    let mut invalid_semantic = encode_portable_package_v1(&tree).unwrap();
    let reference = find_bytes(&invalid_semantic, b"\"value\":\"target-a\"");
    invalid_semantic[reference + b"\"value\":\"target-".len()] = b'z';
    let changed = zip_records(&invalid_semantic)
        .into_iter()
        .find(|record| {
            invalid_semantic[record.data_start..record.data_end]
                .windows(b"\"value\":\"target-z\"".len())
                .any(|window| window == b"\"value\":\"target-z\"")
        })
        .unwrap();
    let changed_name = changed.name;
    refresh_payload_integrity(&mut invalid_semantic, &changed_name);
    assert!(matches!(
        decode_portable_package_v1(&invalid_semantic),
        Err(FormatError::PortablePackage(
            PortablePackageError::InvalidSemanticPayload { .. }
        ))
    ));
}

#[test]
fn declared_archive_resource_limit_is_enforced_before_parsing() {
    let mut oversized = vec![0_u8; PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES + 1];
    oversized[..4].copy_from_slice(b"PK\x03\x04");

    assert!(matches!(
        decode_portable_package_v1(&oversized),
        Err(FormatError::PortablePackage(
            PortablePackageError::ResourceLimit {
                limit: PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES,
                ..
            }
        ))
    ));
}
