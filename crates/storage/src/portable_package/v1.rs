//! Pure canonical portable-package/v1 ZIP32 codec.

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::PortablePackageError;
use crate::{
    FormatError,
    roproj::{CanonicalRoProjectV1, ROPROJ_V1_PATHS},
    strict_json::{FrontendError, inspect},
};

const PACKAGE_FORMAT: &str = "tachiko.portable-package";
const PACKAGE_FORMAT_VERSION: u32 = 1;
const PAYLOAD_FORMAT: &str = "tachiko.roproj";
const PAYLOAD_FORMAT_VERSION: u32 = 1;
const PAYLOAD_DOMAIN: &[u8] = b"tachiko.portable-package/v1\0tachiko.roproj/v1\0";

const ZIP_LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const ZIP_CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const ZIP_END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP_VERSION_MADE_BY: u16 = 20;
const ZIP_VERSION_NEEDED: u16 = 10;
const ZIP_UTF8_FLAG: u16 = 0x0800;
const ZIP_STORE_METHOD: u16 = 0;
const ZIP_DOS_TIME: u16 = 0;
const ZIP_DOS_DATE: u16 = 0x0021;
const ZIP32_U16_SENTINEL: u16 = 0xffff;
const ZIP32_U32_SENTINEL: u32 = 0xffff_ffff;
const ZIP32_MAX_ORDINARY_U32: u64 = 0xffff_fffe;
const LOCAL_HEADER_LENGTH: u64 = 30;
const CENTRAL_HEADER_LENGTH: u64 = 46;
const END_RECORD_LENGTH: usize = 22;

/// Finite in-memory admission bound for one package-v1 artifact.
pub const PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

const PACKAGE_PATHS: [&str; 19] = [
    "package.json",
    "payload/manifest.json",
    "payload/schemas.json",
    "payload/entities/0.jsonl",
    "payload/entities/1.jsonl",
    "payload/entities/2.jsonl",
    "payload/entities/3.jsonl",
    "payload/entities/4.jsonl",
    "payload/entities/5.jsonl",
    "payload/entities/6.jsonl",
    "payload/entities/7.jsonl",
    "payload/entities/8.jsonl",
    "payload/entities/9.jsonl",
    "payload/entities/a.jsonl",
    "payload/entities/b.jsonl",
    "payload/entities/c.jsonl",
    "payload/entities/d.jsonl",
    "payload/entities/e.jsonl",
    "payload/entities/f.jsonl",
];
const PACKAGE_ENTRY_COUNT: u16 = 19;
const MANIFEST_KEYS: [&str; 5] = [
    "format",
    "format_version",
    "payload_format",
    "payload_format_version",
    "payload_root_sha256",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPortablePackageV1 {
    tree: CanonicalRoProjectV1,
    payload_root: [u8; 32],
}

impl VerifiedPortablePackageV1 {
    #[must_use]
    pub fn tree(&self) -> &CanonicalRoProjectV1 {
        &self.tree
    }

    #[must_use]
    pub fn into_tree(self) -> CanonicalRoProjectV1 {
        self.tree
    }

    #[must_use]
    pub fn payload_root(&self) -> [u8; 32] {
        self.payload_root
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageManifestV1 {
    format: String,
    format_version: u32,
    payload_format: String,
    payload_format_version: u32,
    payload_root_sha256: String,
}

struct Entry<'a> {
    name: &'static str,
    body: &'a [u8],
}

struct LocalRecord<'a> {
    offset: usize,
    version_needed: u16,
    flags: u16,
    method: u16,
    dos_time: u16,
    dos_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    name: &'a [u8],
    extra_length: u16,
    data: &'a [u8],
    end: usize,
}

struct CentralRecord<'a> {
    version_made_by: u16,
    version_needed: u16,
    flags: u16,
    method: u16,
    dos_time: u16,
    dos_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    name: &'a [u8],
    extra_length: u16,
    comment_length: u16,
    disk_number_start: u16,
    internal_attributes: u16,
    external_attributes: u32,
    local_offset: u32,
    end: usize,
}

struct EndRecord {
    disk_number: u16,
    central_disk_number: u16,
    entries_on_disk: u16,
    total_entries: u16,
    central_size: u32,
    central_offset: u32,
}

struct Container<'a> {
    end: EndRecord,
    local_records: Vec<LocalRecord<'a>>,
    central_records: Vec<CentralRecord<'a>>,
}

/// Calculate the Accepted exact-payload root for a canonical `.roproj/v1` tree.
#[must_use]
pub fn payload_root(tree: &CanonicalRoProjectV1) -> [u8; 32] {
    let mut root = Sha256::new();
    root.update(PAYLOAD_DOMAIN);
    for file in tree.files() {
        let mut leaf = Sha256::new();
        leaf.update(file.path().as_bytes());
        leaf.update([0]);
        leaf.update(file.bytes());
        root.update(leaf.finalize());
    }
    root.finalize().into()
}

/// Encode an exact canonical `.roproj/v1` tree as canonical package-v1 bytes.
///
/// # Errors
///
/// Returns a package capacity or declared resource-limit error before allocation.
pub fn encode(tree: &CanonicalRoProjectV1) -> Result<Vec<u8>, FormatError> {
    let root = payload_root(tree);
    let manifest = render_manifest(&root);
    let mut entries = Vec::with_capacity(PACKAGE_PATHS.len());
    entries.push(Entry {
        name: PACKAGE_PATHS[0],
        body: manifest.as_bytes(),
    });
    for (index, file) in tree.files().iter().enumerate() {
        entries.push(Entry {
            name: PACKAGE_PATHS[index + 1],
            body: file.bytes(),
        });
    }
    build_zip(&entries).map_err(FormatError::from)
}

/// Decode and completely verify canonical package-v1 bytes.
///
/// # Errors
///
/// Returns the first dependency-ordered stable portable-package failure.
pub fn decode(source: &[u8]) -> Result<VerifiedPortablePackageV1, FormatError> {
    check_input_capacity(source)?;
    let container = parse_container(source)?;
    let manifest_record = package_manifest_record(&container)?;
    let manifest = parse_manifest(manifest_record.data)?;
    validate_canonical_profile(&container)?;
    validate_crc_and_sizes(&container)?;

    let files = container
        .local_records
        .iter()
        .skip(1)
        .zip(ROPROJ_V1_PATHS)
        .map(|(record, path)| (path.to_owned(), record.data.to_vec()))
        .collect::<Vec<_>>();
    let found_root = payload_root_from_files(&files);
    if found_root != manifest.payload_root {
        return Err(PortablePackageError::IntegrityMismatch.into());
    }

    crate::roproj::v1::dispatch_manifest(&files[0].1)
        .map_err(|_| PortablePackageError::PayloadManifestMismatch)?;

    let tree = CanonicalRoProjectV1::try_from_files(files).map_err(map_payload_error)?;
    Ok(VerifiedPortablePackageV1 {
        tree,
        payload_root: found_root,
    })
}

fn render_manifest(root: &[u8; 32]) -> String {
    format!(
        "{{\n  \"format\": \"{PACKAGE_FORMAT}\",\n  \"format_version\": {PACKAGE_FORMAT_VERSION},\n  \"payload_format\": \"{PAYLOAD_FORMAT}\",\n  \"payload_format_version\": {PAYLOAD_FORMAT_VERSION},\n  \"payload_root_sha256\": \"{}\"\n}}\n",
        encode_hex(root)
    )
}

fn build_zip(entries: &[Entry<'_>]) -> Result<Vec<u8>, PortablePackageError> {
    let complete_size = preflight_zip(entries)?;

    let mut archive = Vec::with_capacity(complete_size);
    let mut local_offsets = Vec::with_capacity(entries.len());
    for entry in entries {
        let offset = u32::try_from(archive.len()).expect("preflight proves ZIP32 offset");
        let size = u32::try_from(entry.body.len()).expect("preflight proves ZIP32 body length");
        let name_length =
            u16::try_from(entry.name.len()).expect("fixed package name lengths fit u16");
        local_offsets.push(offset);
        push_u32(&mut archive, ZIP_LOCAL_SIGNATURE);
        push_u16(&mut archive, ZIP_VERSION_NEEDED);
        push_u16(&mut archive, ZIP_UTF8_FLAG);
        push_u16(&mut archive, ZIP_STORE_METHOD);
        push_u16(&mut archive, ZIP_DOS_TIME);
        push_u16(&mut archive, ZIP_DOS_DATE);
        push_u32(&mut archive, crc32(entry.body));
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, name_length);
        push_u16(&mut archive, 0);
        archive.extend_from_slice(entry.name.as_bytes());
        archive.extend_from_slice(entry.body);
    }

    let central_offset = u32::try_from(archive.len()).expect("preflight proves ZIP32 offset");
    for (entry, local_offset) in entries.iter().zip(local_offsets) {
        let size = u32::try_from(entry.body.len()).expect("preflight proves ZIP32 body length");
        let name_length =
            u16::try_from(entry.name.len()).expect("fixed package name lengths fit u16");
        push_u32(&mut archive, ZIP_CENTRAL_SIGNATURE);
        push_u16(&mut archive, ZIP_VERSION_MADE_BY);
        push_u16(&mut archive, ZIP_VERSION_NEEDED);
        push_u16(&mut archive, ZIP_UTF8_FLAG);
        push_u16(&mut archive, ZIP_STORE_METHOD);
        push_u16(&mut archive, ZIP_DOS_TIME);
        push_u16(&mut archive, ZIP_DOS_DATE);
        push_u32(&mut archive, crc32(entry.body));
        push_u32(&mut archive, size);
        push_u32(&mut archive, size);
        push_u16(&mut archive, name_length);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u16(&mut archive, 0);
        push_u32(&mut archive, 0);
        push_u32(&mut archive, local_offset);
        archive.extend_from_slice(entry.name.as_bytes());
    }
    let central_size = u32::try_from(archive.len())
        .expect("preflight proves ZIP32 length")
        .checked_sub(central_offset)
        .expect("central directory follows local records");
    let entry_count = u16::try_from(entries.len()).expect("preflight proves ZIP32 entry count");
    push_u32(&mut archive, ZIP_END_SIGNATURE);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, entry_count);
    push_u16(&mut archive, entry_count);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    debug_assert_eq!(archive.len(), complete_size);
    Ok(archive)
}

fn preflight_zip(entries: &[Entry<'_>]) -> Result<usize, PortablePackageError> {
    if entries.len() >= usize::from(ZIP32_U16_SENTINEL) {
        return Err(PortablePackageError::CapacityExceeded {
            resource: "entry count",
        });
    }

    let mut local_size = 0_u64;
    let mut central_size = 0_u64;
    for entry in entries {
        let name_length = u64::try_from(entry.name.len()).map_err(|_| {
            PortablePackageError::CapacityExceeded {
                resource: "entry name length",
            }
        })?;
        let body_length = u64::try_from(entry.body.len()).map_err(|_| {
            PortablePackageError::CapacityExceeded {
                resource: "entry body length",
            }
        })?;
        require_zip32(body_length, "entry body length")?;
        local_size = checked_sum(
            checked_sum(local_size, checked_sum(LOCAL_HEADER_LENGTH, name_length)?)?,
            body_length,
        )?;
        central_size = checked_sum(
            central_size,
            checked_sum(CENTRAL_HEADER_LENGTH, name_length)?,
        )?;
        require_zip32(local_size, "local record section")?;
    }
    require_zip32(central_size, "central directory")?;
    let complete_size = checked_sum(
        checked_sum(local_size, central_size)?,
        u64::try_from(END_RECORD_LENGTH).expect("fixed end record length fits u64"),
    )?;
    require_zip32(complete_size, "complete archive")?;
    let complete_size =
        usize::try_from(complete_size).map_err(|_| PortablePackageError::CapacityExceeded {
            resource: "complete archive",
        })?;
    if complete_size > PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES {
        return Err(PortablePackageError::ResourceLimit {
            resource: "archive bytes",
            limit: PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES,
            actual: complete_size,
        });
    }
    Ok(complete_size)
}

fn parse_container(source: &[u8]) -> Result<Container<'_>, PortablePackageError> {
    if source.len() < END_RECORD_LENGTH {
        return invalid_container("package is shorter than the ZIP end record");
    }
    if read_u32(source, 0, "initial local signature")? != ZIP_LOCAL_SIGNATURE {
        return invalid_container("package has a prepended stub or lacks local-file framing");
    }
    let end_offset = source.len() - END_RECORD_LENGTH;
    if read_u32(source, end_offset, "end signature")? != ZIP_END_SIGNATURE {
        return invalid_container("end record is absent, commented, or followed by trailing bytes");
    }
    let end = EndRecord {
        disk_number: read_u16(source, end_offset + 4, "end disk number")?,
        central_disk_number: read_u16(source, end_offset + 6, "central disk number")?,
        entries_on_disk: read_u16(source, end_offset + 8, "entries on disk")?,
        total_entries: read_u16(source, end_offset + 10, "total entries")?,
        central_size: read_u32(source, end_offset + 12, "central size")?,
        central_offset: read_u32(source, end_offset + 16, "central offset")?,
    };
    let comment_length = read_u16(source, end_offset + 20, "archive comment length")?;
    if comment_length != 0 {
        return invalid_container("archive comment is not absent");
    }
    if end.disk_number != 0 || end.central_disk_number != 0 {
        return invalid_container("split or spanned disk numbers are not supported");
    }
    if end.entries_on_disk == ZIP32_U16_SENTINEL
        || end.total_entries == ZIP32_U16_SENTINEL
        || end.central_size == ZIP32_U32_SENTINEL
        || end.central_offset == ZIP32_U32_SENTINEL
    {
        return invalid_container("ZIP64 sentinel appears in ordinary ZIP32 framing");
    }
    if end.entries_on_disk != end.total_entries {
        return invalid_container("entry counts do not describe a complete one-disk container");
    }
    let central_offset = usize::try_from(end.central_offset)
        .map_err(|_| invalid_container_error("central offset does not fit this host"))?;
    let central_size = usize::try_from(end.central_size)
        .map_err(|_| invalid_container_error("central size does not fit this host"))?;
    if central_offset.checked_add(central_size) != Some(end_offset) {
        return invalid_container("central directory does not exactly precede the end record");
    }

    let mut local_records = Vec::with_capacity(usize::from(end.total_entries));
    let mut local_offset = 0;
    for _ in 0..end.total_entries {
        let record = parse_local_record(source, local_offset)?;
        if record.end > central_offset {
            return invalid_container("local entry overlaps the central directory");
        }
        local_offset = record.end;
        local_records.push(record);
    }
    if local_offset != central_offset {
        return invalid_container("local records do not end at the central-directory offset");
    }

    let mut central_records = Vec::with_capacity(usize::from(end.total_entries));
    let mut record_offset = central_offset;
    for _ in 0..end.total_entries {
        let record = parse_central_record(source, record_offset)?;
        if record.end > end_offset {
            return invalid_container("central entry overlaps the end record");
        }
        record_offset = record.end;
        central_records.push(record);
    }
    if record_offset != end_offset {
        return invalid_container("central records do not consume the declared directory");
    }
    if central_records
        .iter()
        .any(|record| record.disk_number_start != 0)
    {
        return invalid_container("a central entry selects another start disk");
    }
    Ok(Container {
        end,
        local_records,
        central_records,
    })
}

fn parse_local_record(
    source: &[u8],
    offset: usize,
) -> Result<LocalRecord<'_>, PortablePackageError> {
    require_range(source, offset, 30, "local header")?;
    if read_u32(source, offset, "local signature")? != ZIP_LOCAL_SIGNATURE {
        return invalid_container("missing local header at the declared offset");
    }
    let name_length = usize::from(read_u16(source, offset + 26, "local name length")?);
    let extra_length = read_u16(source, offset + 28, "local extra length")?;
    let compressed_size = read_u32(source, offset + 18, "local compressed size")?;
    let name_start = checked_offset(offset, 30, "local name offset")?;
    let data_start = checked_offset(
        checked_offset(name_start, name_length, "local name end")?,
        usize::from(extra_length),
        "local extra end",
    )?;
    let data_end = checked_offset(
        data_start,
        usize::try_from(compressed_size)
            .map_err(|_| invalid_container_error("local body size does not fit this host"))?,
        "local data end",
    )?;
    require_range(source, name_start, name_length, "local name")?;
    require_range(
        source,
        checked_offset(name_start, name_length, "local extra offset")?,
        usize::from(extra_length),
        "local extra",
    )?;
    require_range(
        source,
        data_start,
        usize::try_from(compressed_size)
            .map_err(|_| invalid_container_error("local body size does not fit this host"))?,
        "local data",
    )?;
    Ok(LocalRecord {
        offset,
        version_needed: read_u16(source, offset + 4, "local version")?,
        flags: read_u16(source, offset + 6, "local flags")?,
        method: read_u16(source, offset + 8, "local method")?,
        dos_time: read_u16(source, offset + 10, "local time")?,
        dos_date: read_u16(source, offset + 12, "local date")?,
        crc32: read_u32(source, offset + 14, "local CRC")?,
        compressed_size,
        uncompressed_size: read_u32(source, offset + 22, "local size")?,
        name: &source[name_start..name_start + name_length],
        extra_length,
        data: &source[data_start..data_end],
        end: data_end,
    })
}

fn parse_central_record(
    source: &[u8],
    offset: usize,
) -> Result<CentralRecord<'_>, PortablePackageError> {
    require_range(source, offset, 46, "central header")?;
    if read_u32(source, offset, "central signature")? != ZIP_CENTRAL_SIGNATURE {
        return invalid_container("missing central header at the declared offset");
    }
    let name_length = usize::from(read_u16(source, offset + 28, "central name length")?);
    let extra_length = read_u16(source, offset + 30, "central extra length")?;
    let comment_length = read_u16(source, offset + 32, "central comment length")?;
    let name_start = checked_offset(offset, 46, "central name offset")?;
    let extra_start = checked_offset(name_start, name_length, "central name end")?;
    let comment_start =
        checked_offset(extra_start, usize::from(extra_length), "central extra end")?;
    let end = checked_offset(
        comment_start,
        usize::from(comment_length),
        "central comment end",
    )?;
    require_range(source, name_start, name_length, "central name")?;
    require_range(
        source,
        extra_start,
        usize::from(extra_length),
        "central extra",
    )?;
    require_range(
        source,
        comment_start,
        usize::from(comment_length),
        "central comment",
    )?;
    Ok(CentralRecord {
        version_made_by: read_u16(source, offset + 4, "central made-by version")?,
        version_needed: read_u16(source, offset + 6, "central needed version")?,
        flags: read_u16(source, offset + 8, "central flags")?,
        method: read_u16(source, offset + 10, "central method")?,
        dos_time: read_u16(source, offset + 12, "central time")?,
        dos_date: read_u16(source, offset + 14, "central date")?,
        crc32: read_u32(source, offset + 16, "central CRC")?,
        compressed_size: read_u32(source, offset + 20, "central compressed size")?,
        uncompressed_size: read_u32(source, offset + 24, "central size")?,
        name: &source[name_start..name_start + name_length],
        extra_length,
        comment_length,
        disk_number_start: read_u16(source, offset + 34, "central start disk")?,
        internal_attributes: read_u16(source, offset + 36, "central internal attributes")?,
        external_attributes: read_u32(source, offset + 38, "central external attributes")?,
        local_offset: read_u32(source, offset + 42, "central local offset")?,
        end,
    })
}

fn package_manifest_record<'a>(
    container: &'a Container<'a>,
) -> Result<&'a LocalRecord<'a>, PortablePackageError> {
    let mut matches = container
        .local_records
        .iter()
        .filter(|record| record.name == PACKAGE_PATHS[0].as_bytes());
    let Some(record) = matches.next() else {
        return invalid_manifest("package.json is missing");
    };
    if matches.next().is_some() {
        return invalid_manifest("package.json is duplicated");
    }
    if record.method != ZIP_STORE_METHOD || record.flags & 1 != 0 {
        return noncanonical_container("package.json is compressed or encrypted");
    }
    Ok(record)
}

struct ParsedManifest {
    payload_root: [u8; 32],
}

fn parse_manifest(source: &[u8]) -> Result<ParsedManifest, PortablePackageError> {
    let text = std::str::from_utf8(source)
        .map_err(|_| invalid_manifest_error("package.json is not valid UTF-8"))?;
    inspect(text).map_err(map_manifest_frontend_error)?;
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| invalid_manifest_error("package.json is not valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid_manifest_error("package.json root is not an object"))?;
    if object.get("format").and_then(serde_json::Value::as_str) != Some(PACKAGE_FORMAT) {
        return invalid_manifest("package.json format is missing or malformed");
    }
    let version = object
        .get("format_version")
        .and_then(serde_json::Value::as_number)
        .map(ToString::to_string)
        .ok_or_else(|| {
            invalid_manifest_error("format_version is not a lexical positive integer")
        })?;
    if version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|byte| byte.is_ascii_digit())
    {
        return invalid_manifest("format_version is not a lexical positive integer");
    }
    if version != PACKAGE_FORMAT_VERSION.to_string() {
        return Err(PortablePackageError::UnsupportedVersion { found: version });
    }
    if object.len() != MANIFEST_KEYS.len()
        || !MANIFEST_KEYS.iter().all(|key| object.contains_key(*key))
    {
        return invalid_manifest("package.json members are missing or unknown");
    }
    let manifest: PackageManifestV1 = serde_json::from_value(value)
        .map_err(|_| invalid_manifest_error("package.json does not match package v1"))?;
    if manifest.format != PACKAGE_FORMAT
        || manifest.format_version != PACKAGE_FORMAT_VERSION
        || manifest.payload_format != PAYLOAD_FORMAT
        || manifest.payload_format_version != PAYLOAD_FORMAT_VERSION
    {
        return invalid_manifest("package.json payload claim is malformed or unsupported");
    }
    let payload_root = decode_root(&manifest.payload_root_sha256)?;
    if source != render_manifest(&payload_root).as_bytes() {
        return noncanonical_container("package.json bytes are not canonical v1");
    }
    Ok(ParsedManifest { payload_root })
}

fn validate_canonical_profile(container: &Container<'_>) -> Result<(), PortablePackageError> {
    let local_names = decode_names(container.local_records.iter().map(|record| record.name))?;
    let central_names = decode_names(container.central_records.iter().map(|record| record.name))?;
    if !same_name_set(&local_names) || !same_name_set(&central_names) {
        return Err(PortablePackageError::EntrySetMismatch {
            message: "package entries are missing, unknown, duplicate, extra, or aliased"
                .to_owned(),
        });
    }
    if local_names.as_slice() != PACKAGE_PATHS || central_names.as_slice() != PACKAGE_PATHS {
        return noncanonical_container("package entry order is not canonical");
    }
    if container.end.entries_on_disk != PACKAGE_ENTRY_COUNT
        || container.end.total_entries != PACKAGE_ENTRY_COUNT
    {
        return noncanonical_container("end record does not describe 19 canonical entries");
    }

    for (index, expected_name) in PACKAGE_PATHS.iter().enumerate() {
        let local = &container.local_records[index];
        let central = &container.central_records[index];
        if local.version_needed != ZIP_VERSION_NEEDED
            || local.flags != ZIP_UTF8_FLAG
            || local.method != ZIP_STORE_METHOD
            || local.dos_time != ZIP_DOS_TIME
            || local.dos_date != ZIP_DOS_DATE
            || local.extra_length != 0
        {
            return noncanonical_container(&format!(
                "'{expected_name}' local header has noncanonical metadata"
            ));
        }
        if central.version_made_by != ZIP_VERSION_MADE_BY
            || central.version_needed != ZIP_VERSION_NEEDED
            || central.flags != ZIP_UTF8_FLAG
            || central.method != ZIP_STORE_METHOD
            || central.dos_time != ZIP_DOS_TIME
            || central.dos_date != ZIP_DOS_DATE
            || central.extra_length != 0
            || central.comment_length != 0
            || central.disk_number_start != 0
            || central.internal_attributes != 0
            || central.external_attributes != 0
        {
            return noncanonical_container(&format!(
                "'{expected_name}' central header has noncanonical metadata"
            ));
        }
        if local.name != central.name
            || local.crc32 != central.crc32
            || local.compressed_size != central.compressed_size
            || local.uncompressed_size != central.uncompressed_size
            || usize::try_from(central.local_offset).ok() != Some(local.offset)
        {
            return noncanonical_container(&format!(
                "'{expected_name}' local and central records disagree"
            ));
        }
    }
    Ok(())
}

fn validate_crc_and_sizes(container: &Container<'_>) -> Result<(), PortablePackageError> {
    for record in &container.local_records {
        let path = std::str::from_utf8(record.name)
            .unwrap_or("<invalid UTF-8 entry name>")
            .to_owned();
        if record.compressed_size != record.uncompressed_size
            || usize::try_from(record.compressed_size).ok() != Some(record.data.len())
            || crc32(record.data) != record.crc32
        {
            return Err(PortablePackageError::CrcMismatch { path });
        }
    }
    Ok(())
}

fn decode_names<'a>(
    names: impl Iterator<Item = &'a [u8]>,
) -> Result<Vec<&'a str>, PortablePackageError> {
    names
        .map(|name| {
            std::str::from_utf8(name).map_err(|_| PortablePackageError::EntrySetMismatch {
                message: "entry name is not valid canonical UTF-8".to_owned(),
            })
        })
        .collect()
}

fn same_name_set(names: &[&str]) -> bool {
    names.len() == PACKAGE_PATHS.len()
        && PACKAGE_PATHS
            .iter()
            .all(|expected| names.iter().filter(|name| *name == expected).count() == 1)
}

fn payload_root_from_files(files: &[(String, Vec<u8>)]) -> [u8; 32] {
    let mut root = Sha256::new();
    root.update(PAYLOAD_DOMAIN);
    for (path, body) in files {
        let mut leaf = Sha256::new();
        leaf.update(path.as_bytes());
        leaf.update([0]);
        leaf.update(body);
        root.update(leaf.finalize());
    }
    root.finalize().into()
}

fn decode_root(source: &str) -> Result<[u8; 32], PortablePackageError> {
    if source.len() != 64
        || !source
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid_manifest("payload_root_sha256 is not 64 lowercase hexadecimal digits");
    }
    let mut root = [0_u8; 32];
    for (output, pair) in root.iter_mut().zip(source.as_bytes().chunks_exact(2)) {
        *output = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(root)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("decode_root validates lowercase hexadecimal first"),
    }
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

fn check_input_capacity(source: &[u8]) -> Result<(), PortablePackageError> {
    let length =
        u64::try_from(source.len()).map_err(|_| PortablePackageError::CapacityExceeded {
            resource: "complete archive",
        })?;
    require_zip32(length, "complete archive")?;
    if source.len() > PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES {
        return Err(PortablePackageError::ResourceLimit {
            resource: "archive bytes",
            limit: PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES,
            actual: source.len(),
        });
    }
    Ok(())
}

fn require_zip32(value: u64, resource: &'static str) -> Result<(), PortablePackageError> {
    if value > ZIP32_MAX_ORDINARY_U32 {
        Err(PortablePackageError::CapacityExceeded { resource })
    } else {
        Ok(())
    }
}

fn checked_sum(left: u64, right: u64) -> Result<u64, PortablePackageError> {
    left.checked_add(right)
        .ok_or(PortablePackageError::CapacityExceeded {
            resource: "complete archive",
        })
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(
    source: &[u8],
    offset: usize,
    label: &'static str,
) -> Result<u16, PortablePackageError> {
    let bytes = require_range(source, offset, 2, label)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(
    source: &[u8],
    offset: usize,
    label: &'static str,
) -> Result<u32, PortablePackageError> {
    let bytes = require_range(source, offset, 4, label)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn require_range<'a>(
    source: &'a [u8],
    offset: usize,
    length: usize,
    label: &'static str,
) -> Result<&'a [u8], PortablePackageError> {
    let Some(end) = offset.checked_add(length) else {
        return invalid_container(&format!("overflowed {label} range"));
    };
    source
        .get(offset..end)
        .ok_or_else(|| invalid_container_error(&format!("truncated or out-of-range {label}")))
}

fn checked_offset(
    offset: usize,
    length: usize,
    label: &'static str,
) -> Result<usize, PortablePackageError> {
    offset
        .checked_add(length)
        .ok_or_else(|| invalid_container_error(&format!("overflowed {label}")))
}

fn map_manifest_frontend_error(error: FrontendError) -> PortablePackageError {
    match error {
        FrontendError::InvalidJson(_) => invalid_manifest_error("package.json is not valid JSON"),
        FrontendError::DuplicateMember(member) => {
            invalid_manifest_error(&format!("duplicate package.json member '{member}'"))
        }
        FrontendError::NestingLimit { .. } => {
            invalid_manifest_error("package.json exceeds its JSON nesting limit")
        }
    }
}

fn map_payload_error(error: FormatError) -> PortablePackageError {
    match error {
        FormatError::InvalidDocument { diagnostics } => {
            PortablePackageError::InvalidSemanticPayload { diagnostics }
        }
        other => PortablePackageError::NonCanonicalPayload {
            message: other.to_string(),
        },
    }
}

fn invalid_container<T>(message: &str) -> Result<T, PortablePackageError> {
    Err(invalid_container_error(message))
}

fn invalid_container_error(message: &str) -> PortablePackageError {
    PortablePackageError::InvalidContainer {
        message: message.to_owned(),
    }
}

fn invalid_manifest<T>(message: &str) -> Result<T, PortablePackageError> {
    Err(invalid_manifest_error(message))
}

fn invalid_manifest_error(message: &str) -> PortablePackageError {
    PortablePackageError::InvalidManifest {
        message: message.to_owned(),
    }
}

fn noncanonical_container<T>(message: &str) -> Result<T, PortablePackageError> {
    Err(PortablePackageError::NonCanonicalContainer {
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{PortablePackageError, crc32, require_zip32};

    #[test]
    fn crc32_matches_the_standard_zip_check_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn zip32_sentinel_is_a_capacity_error() {
        assert!(matches!(
            require_zip32(0xffff_ffff, "test field"),
            Err(PortablePackageError::CapacityExceeded {
                resource: "test field"
            })
        ));
    }
}
