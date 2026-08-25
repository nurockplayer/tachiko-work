#!/usr/bin/env node
// DISPOSABLE, NON-PRODUCTION architecture probe for Issue #43.
// Node standard library only. This executable evidence deliberately does not
// provide a production .roproj codec, packaged .ro codec, or CLI command.

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  chmod,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  utimes,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TextDecoder } from 'node:util';

const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));
const FIXTURE_DIRECTORY = resolve(
  SCRIPT_DIRECTORY,
  '../fixtures/issue-43-portable-package-v1',
);
const EMPTY_ROPROJ_DIRECTORY = join(FIXTURE_DIRECTORY, 'empty.roproj');
const GOLDEN_HEX_PATH = join(FIXTURE_DIRECTORY, 'empty-package-v1.hex');

const ENTITY_SHARDS = [...'0123456789abcdef'].map(
  (bucket) => `entities/${bucket}.jsonl`,
);
const ROPROJ_PATHS = ['manifest.json', 'schemas.json', ...ENTITY_SHARDS];
const PACKAGE_PATHS = [
  'package.json',
  ...ROPROJ_PATHS.map((path) => `payload/${path}`),
];
const PACKAGE_PROFILE = 'tachiko.portable-package/v1';
const PAYLOAD_PROFILE = 'tachiko.roproj/v1';
const PAYLOAD_DOMAIN = Buffer.from(`${PACKAGE_PROFILE}\0${PAYLOAD_PROFILE}\0`, 'ascii');

const ZIP_LOCAL_SIGNATURE = 0x04034b50;
const ZIP_CENTRAL_SIGNATURE = 0x02014b50;
const ZIP_END_SIGNATURE = 0x06054b50;
const ZIP_VERSION_MADE_BY = 20;
const ZIP_VERSION_NEEDED = 10;
const ZIP_UTF8_FLAG = 0x0800;
const ZIP_STORE_METHOD = 0;
const ZIP_DOS_TIME = 0x0000;
const ZIP_DOS_DATE = 0x0021;
const ZIP32_U16_SENTINEL = 0xffff;
const ZIP32_U32_SENTINEL = 0xffffffff;
const ZIP32_MAX_ORDINARY_U16 = ZIP32_U16_SENTINEL - 1;
const ZIP32_MAX_ORDINARY_U32 = ZIP32_U32_SENTINEL - 1;

const CODES = Object.freeze({
  capacityExceeded: 'portable_package.capacity_exceeded',
  crcMismatch: 'portable_package.crc_mismatch',
  destinationExists: 'portable_package.destination_exists',
  entrySetMismatch: 'portable_package.entry_set_mismatch',
  integrityMismatch: 'portable_package.integrity_mismatch',
  invalidContainer: 'portable_package.invalid_container',
  invalidManifest: 'portable_package.invalid_manifest',
  invalidSemanticPayload: 'portable_package.invalid_semantic_payload',
  noncanonicalContainer: 'portable_package.noncanonical_container',
  noncanonicalPayload: 'portable_package.noncanonical_payload',
  payloadManifestMismatch: 'portable_package.payload_manifest_mismatch',
  publicationFailed: 'portable_package.publication_failed',
  sourceMismatch: 'portable_package.source_mismatch',
  sourceNotCanonical: 'portable_package.source_not_canonical',
  unsupportedPackageVersion: 'portable_package.unsupported_version',
});

class ProbeFailure extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'ProbeFailure';
    this.code = code;
  }
}

function fail(code, message) {
  throw new ProbeFailure(code, message);
}

function ensure(condition, code, message) {
  if (!condition) fail(code, message);
}

function compareAscii(left, right) {
  return Buffer.compare(Buffer.from(left, 'ascii'), Buffer.from(right, 'ascii'));
}

function sameArray(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function sameMultiset(left, right) {
  return sameArray([...left].sort(compareAscii), [...right].sort(compareAscii));
}

function sha256Buffer(...chunks) {
  const digest = createHash('sha256');
  for (const chunk of chunks) digest.update(chunk);
  return digest.digest();
}

function sha256Hex(bytes) {
  return sha256Buffer(bytes).toString('hex');
}

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ ((crc & 1) === 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function roprojManifestBytes(document) {
  return Buffer.from(
    `{
  "format": "tachiko.roproj",
  "format_version": 1,
  "document": {
    "id": ${JSON.stringify(document.id)},
    "title": ${JSON.stringify(document.title)}
  }
}
`,
    'utf8',
  );
}

function portableManifestObject(payloadRootSha256) {
  return {
    format: 'tachiko.portable-package',
    format_version: 1,
    payload_format: 'tachiko.roproj',
    payload_format_version: 1,
    payload_root_sha256: payloadRootSha256,
  };
}

function portableManifestBytes(payloadRootSha256) {
  ensure(
    /^[0-9a-f]{64}$/.test(payloadRootSha256),
    CODES.invalidManifest,
    'payload_root_sha256 must be 64 lowercase hexadecimal digits',
  );
  return Buffer.from(
    `{
  "format": "tachiko.portable-package",
  "format_version": 1,
  "payload_format": "tachiko.roproj",
  "payload_format_version": 1,
  "payload_root_sha256": "${payloadRootSha256}"
}
`,
    'utf8',
  );
}

function prettyJsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function payloadRootSha256(tree, paths = ROPROJ_PATHS) {
  const leaves = paths.map((path) => {
    const body = tree.get(path);
    ensure(body !== undefined, CODES.noncanonicalPayload, `missing payload path ${path}`);
    return sha256Buffer(Buffer.from(path, 'utf8'), Buffer.from([0]), body);
  });
  return sha256Buffer(PAYLOAD_DOMAIN, ...leaves).toString('hex');
}

function canonicalPackageEntries(tree) {
  const payloadRoot = payloadRootSha256(tree);
  return [
    { name: 'package.json', body: portableManifestBytes(payloadRoot) },
    ...ROPROJ_PATHS.map((path) => ({
      name: `payload/${path}`,
      body: tree.get(path),
    })),
  ];
}

function u16(value) {
  const bytes = Buffer.alloc(2);
  bytes.writeUInt16LE(value);
  return bytes;
}

function u32(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(value);
  return bytes;
}

function ensureZip32U16(value, label) {
  ensure(
    Number.isInteger(value) && value >= 0 && value <= ZIP32_MAX_ORDINARY_U16,
    CODES.capacityExceeded,
    `${label} does not fit an ordinary ZIP32 16-bit field`,
  );
}

function ensureZip32U32(value, label) {
  ensure(
    Number.isInteger(value) && value >= 0 && value <= ZIP32_MAX_ORDINARY_U32,
    CODES.capacityExceeded,
    `${label} does not fit an ordinary ZIP32 32-bit field`,
  );
}

function localHeader(entry, options) {
  const name = Buffer.from(entry.name, 'ascii');
  const size = entry.body.length;
  ensureZip32U16(name.length, 'entry-name length');
  ensureZip32U32(size, `entry size for ${entry.name}`);
  return Buffer.concat([
    u32(ZIP_LOCAL_SIGNATURE),
    u16(ZIP_VERSION_NEEDED),
    u16(options.flags),
    u16(ZIP_STORE_METHOD),
    u16(options.dosTime),
    u16(options.dosDate),
    u32(crc32(entry.body)),
    u32(size),
    u32(size),
    u16(name.length),
    u16(0),
    name,
  ]);
}

function centralHeader(entry, localOffset, options) {
  const name = Buffer.from(entry.name, 'ascii');
  const size = entry.body.length;
  ensureZip32U32(localOffset, `local-header offset for ${entry.name}`);
  return Buffer.concat([
    u32(ZIP_CENTRAL_SIGNATURE),
    u16(ZIP_VERSION_MADE_BY),
    u16(ZIP_VERSION_NEEDED),
    u16(options.flags),
    u16(ZIP_STORE_METHOD),
    u16(options.dosTime),
    u16(options.dosDate),
    u32(crc32(entry.body)),
    u32(size),
    u32(size),
    u16(name.length),
    u16(0),
    u16(0),
    u16(0),
    u16(0),
    u32(0),
    u32(localOffset),
    name,
  ]);
}

function buildCanonicalZip32(entries, overrides = {}) {
  const options = {
    flags: overrides.flags ?? ZIP_UTF8_FLAG,
    dosTime: overrides.dosTime ?? ZIP_DOS_TIME,
    dosDate: overrides.dosDate ?? ZIP_DOS_DATE,
  };
  ensureZip32U16(entries.length, 'entry count');

  const localParts = [];
  const localOffsets = [];
  let offset = 0;
  for (const entry of entries) {
    ensure(Buffer.isBuffer(entry.body), CODES.invalidContainer, `${entry.name} body is not bytes`);
    const header = localHeader(entry, options);
    localOffsets.push(offset);
    localParts.push(header, entry.body);
    offset += header.length + entry.body.length;
    ensureZip32U32(offset, 'local-record section size');
  }

  const centralOffset = offset;
  const centralParts = entries.map((entry, index) =>
    centralHeader(entry, localOffsets[index], options),
  );
  const centralSize = centralParts.reduce((total, part) => total + part.length, 0);
  ensureZip32U32(centralOffset, 'central-directory offset');
  ensureZip32U32(centralSize, 'central-directory size');

  const end = Buffer.concat([
    u32(ZIP_END_SIGNATURE),
    u16(0),
    u16(0),
    u16(entries.length),
    u16(entries.length),
    u32(centralSize),
    u32(centralOffset),
    u16(0),
  ]);
  const archive = Buffer.concat([...localParts, ...centralParts, end]);
  ensureZip32U32(archive.length, 'complete archive length');
  return archive;
}

function requireBytes(bytes, offset, length, label) {
  ensure(
    Number.isSafeInteger(offset) && Number.isSafeInteger(length) &&
      offset >= 0 && length >= 0 && offset + length <= bytes.length,
    CODES.invalidContainer,
    `truncated or out-of-range ${label}`,
  );
}

function readU16(bytes, offset, label) {
  requireBytes(bytes, offset, 2, label);
  return bytes.readUInt16LE(offset);
}

function readU32(bytes, offset, label) {
  requireBytes(bytes, offset, 4, label);
  return bytes.readUInt32LE(offset);
}

function parseLocalRecord(bytes, offset) {
  requireBytes(bytes, offset, 30, 'local file header');
  ensure(
    readU32(bytes, offset, 'local signature') === ZIP_LOCAL_SIGNATURE,
    CODES.invalidContainer,
    `missing local file header at byte ${offset}`,
  );
  const nameLength = readU16(bytes, offset + 26, 'local name length');
  const extraLength = readU16(bytes, offset + 28, 'local extra length');
  const compressedSize = readU32(bytes, offset + 18, 'local compressed size');
  const nameStart = offset + 30;
  const dataStart = nameStart + nameLength + extraLength;
  const dataEnd = dataStart + compressedSize;
  requireBytes(bytes, nameStart, nameLength + extraLength, 'local name and extra field');
  requireBytes(bytes, dataStart, compressedSize, 'stored entry data');
  return {
    offset,
    versionNeeded: readU16(bytes, offset + 4, 'local version needed'),
    flags: readU16(bytes, offset + 6, 'local flags'),
    method: readU16(bytes, offset + 8, 'local method'),
    dosTime: readU16(bytes, offset + 10, 'local time'),
    dosDate: readU16(bytes, offset + 12, 'local date'),
    crc32: readU32(bytes, offset + 14, 'local CRC-32'),
    compressedSize,
    uncompressedSize: readU32(bytes, offset + 22, 'local uncompressed size'),
    nameLength,
    extraLength,
    nameBytes: bytes.subarray(nameStart, nameStart + nameLength),
    extraBytes: bytes.subarray(nameStart + nameLength, dataStart),
    data: bytes.subarray(dataStart, dataEnd),
    dataStart,
    end: dataEnd,
  };
}

function parseCentralRecord(bytes, offset) {
  requireBytes(bytes, offset, 46, 'central directory header');
  ensure(
    readU32(bytes, offset, 'central signature') === ZIP_CENTRAL_SIGNATURE,
    CODES.invalidContainer,
    `missing central directory header at byte ${offset}`,
  );
  const nameLength = readU16(bytes, offset + 28, 'central name length');
  const extraLength = readU16(bytes, offset + 30, 'central extra length');
  const commentLength = readU16(bytes, offset + 32, 'central comment length');
  const variableLength = nameLength + extraLength + commentLength;
  requireBytes(bytes, offset + 46, variableLength, 'central variable fields');
  return {
    offset,
    versionMadeBy: readU16(bytes, offset + 4, 'central version made by'),
    versionNeeded: readU16(bytes, offset + 6, 'central version needed'),
    flags: readU16(bytes, offset + 8, 'central flags'),
    method: readU16(bytes, offset + 10, 'central method'),
    dosTime: readU16(bytes, offset + 12, 'central time'),
    dosDate: readU16(bytes, offset + 14, 'central date'),
    crc32: readU32(bytes, offset + 16, 'central CRC-32'),
    compressedSize: readU32(bytes, offset + 20, 'central compressed size'),
    uncompressedSize: readU32(bytes, offset + 24, 'central uncompressed size'),
    nameLength,
    extraLength,
    commentLength,
    diskNumberStart: readU16(bytes, offset + 34, 'central disk number'),
    internalAttributes: readU16(bytes, offset + 36, 'central internal attributes'),
    externalAttributes: readU32(bytes, offset + 38, 'central external attributes'),
    localOffset: readU32(bytes, offset + 42, 'central local offset'),
    nameBytes: bytes.subarray(offset + 46, offset + 46 + nameLength),
    extraBytes: bytes.subarray(
      offset + 46 + nameLength,
      offset + 46 + nameLength + extraLength,
    ),
    commentBytes: bytes.subarray(
      offset + 46 + nameLength + extraLength,
      offset + 46 + variableLength,
    ),
    end: offset + 46 + variableLength,
  };
}

function parseZip32Container(bytes) {
  ensure(Buffer.isBuffer(bytes), CODES.invalidContainer, 'package is not a byte buffer');
  ensure(bytes.length >= 22, CODES.invalidContainer, 'package is shorter than ZIP end record');
  ensure(
    readU32(bytes, 0, 'first record signature') === ZIP_LOCAL_SIGNATURE,
    CODES.invalidContainer,
    'package has a prepended stub or lacks initial local-file framing',
  );

  const endOffset = bytes.length - 22;
  ensure(
    readU32(bytes, endOffset, 'end signature') === ZIP_END_SIGNATURE,
    CODES.invalidContainer,
    'end-of-central-directory record is absent, commented, or followed by trailing data',
  );
  const end = {
    offset: endOffset,
    diskNumber: readU16(bytes, endOffset + 4, 'end disk number'),
    centralDiskNumber: readU16(bytes, endOffset + 6, 'end central disk number'),
    entriesOnDisk: readU16(bytes, endOffset + 8, 'end entries on disk'),
    totalEntries: readU16(bytes, endOffset + 10, 'end total entries'),
    centralSize: readU32(bytes, endOffset + 12, 'end central size'),
    centralOffset: readU32(bytes, endOffset + 16, 'end central offset'),
    commentLength: readU16(bytes, endOffset + 20, 'end comment length'),
  };
  ensure(end.commentLength === 0, CODES.invalidContainer, 'archive comment is not absent');
  ensure(
    end.entriesOnDisk !== ZIP32_U16_SENTINEL &&
      end.totalEntries !== ZIP32_U16_SENTINEL &&
      end.centralSize !== ZIP32_U32_SENTINEL &&
      end.centralOffset !== ZIP32_U32_SENTINEL,
    CODES.invalidContainer,
    'ZIP64 sentinel appears without an ordinary ZIP32 structure',
  );
  ensure(
    end.entriesOnDisk === end.totalEntries,
    CODES.invalidContainer,
    'split or inconsistent entry counts are not a complete one-disk container',
  );
  ensure(
    end.centralOffset + end.centralSize === endOffset,
    CODES.invalidContainer,
    'central directory does not exactly precede the end record',
  );

  const localRecords = [];
  let localOffset = 0;
  for (let index = 0; index < end.totalEntries; index += 1) {
    const record = parseLocalRecord(bytes, localOffset);
    ensure(record.end <= end.centralOffset, CODES.invalidContainer, 'local data overlaps central directory');
    localRecords.push(record);
    localOffset = record.end;
  }
  ensure(
    localOffset === end.centralOffset,
    CODES.invalidContainer,
    'local records do not end at the declared central-directory offset',
  );

  const centralRecords = [];
  let centralOffset = end.centralOffset;
  for (let index = 0; index < end.totalEntries; index += 1) {
    const record = parseCentralRecord(bytes, centralOffset);
    ensure(record.end <= endOffset, CODES.invalidContainer, 'central record overlaps end record');
    centralRecords.push(record);
    centralOffset = record.end;
  }
  ensure(
    centralOffset === endOffset,
    CODES.invalidContainer,
    'central records do not consume the declared central directory exactly',
  );
  return { bytes, end, localRecords, centralRecords };
}

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

function decodeEntryName(nameBytes) {
  let name;
  try {
    name = utf8Decoder.decode(nameBytes);
  } catch {
    fail(CODES.entrySetMismatch, 'entry name is not valid UTF-8');
  }
  ensure(
    Buffer.from(name, 'utf8').equals(nameBytes),
    CODES.entrySetMismatch,
    'entry name has a noncanonical UTF-8 spelling',
  );
  return name;
}

function decodeUtf8(bytes, code, label) {
  try {
    return utf8Decoder.decode(bytes);
  } catch {
    fail(code, `${label} is not valid UTF-8`);
  }
}

function detectDuplicateJsonKeys(text) {
  const seen = new Set();
  const keyPattern = /"((?:\\.|[^"\\])*)"\s*:/gu;
  for (const match of text.matchAll(keyPattern)) {
    let key;
    try {
      key = JSON.parse(`"${match[1]}"`);
    } catch {
      continue;
    }
    if (seen.has(key)) return key;
    seen.add(key);
  }
  return null;
}

function parsePortableManifest(bytes) {
  const text = decodeUtf8(bytes, CODES.invalidManifest, 'package.json');
  const duplicate = detectDuplicateJsonKeys(text);
  ensure(duplicate === null, CODES.invalidManifest, `duplicate package.json member ${duplicate}`);
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    fail(CODES.invalidManifest, 'package.json is not valid JSON');
  }
  ensure(
    value !== null && typeof value === 'object' && !Array.isArray(value),
    CODES.invalidManifest,
    'package.json root is not an object',
  );
  ensure(
    value.format === 'tachiko.portable-package',
    CODES.invalidManifest,
    'package.json format is missing or malformed',
  );
  ensure(
    Number.isInteger(value.format_version) && value.format_version > 0,
    CODES.invalidManifest,
    'package.json format_version is missing or malformed',
  );
  if (value.format_version !== 1) {
    fail(
      CODES.unsupportedPackageVersion,
      `unsupported ${PACKAGE_PROFILE.split('/')[0]} version ${value.format_version}`,
    );
  }

  const expectedKeys = [
    'format',
    'format_version',
    'payload_format',
    'payload_format_version',
    'payload_root_sha256',
  ];
  ensure(
    sameMultiset(Object.keys(value), expectedKeys),
    CODES.invalidManifest,
    'package.json members are missing or unknown',
  );
  ensure(
    value.payload_format === 'tachiko.roproj' && value.payload_format_version === 1,
    CODES.invalidManifest,
    'package.json payload claim is missing or malformed',
  );
  ensure(
    typeof value.payload_root_sha256 === 'string' &&
      /^[0-9a-f]{64}$/.test(value.payload_root_sha256),
    CODES.invalidManifest,
    'package.json payload root is malformed',
  );
  ensure(
    bytes.equals(portableManifestBytes(value.payload_root_sha256)),
    CODES.noncanonicalContainer,
    'package.json bytes are not the canonical v1 spelling',
  );
  return value;
}

function packageManifestRecord(container) {
  const matches = container.localRecords.filter(
    (record) => decodeEntryName(record.nameBytes) === 'package.json',
  );
  ensure(matches.length === 1, CODES.invalidManifest, 'package.json is missing or duplicated');
  const record = matches[0];
  ensure(
    record.method === ZIP_STORE_METHOD && (record.flags & 1) === 0,
    CODES.noncanonicalContainer,
    'package.json is compressed or encrypted and cannot dispatch package v1',
  );
  return record;
}

function validateCanonicalZipProfile(container) {
  const localNames = container.localRecords.map((record) => decodeEntryName(record.nameBytes));
  const centralNames = container.centralRecords.map((record) => decodeEntryName(record.nameBytes));
  ensure(
    sameMultiset(localNames, PACKAGE_PATHS) && sameMultiset(centralNames, PACKAGE_PATHS),
    CODES.entrySetMismatch,
    'package entries are missing, unknown, duplicate, or aliased',
  );
  ensure(
    sameArray(localNames, PACKAGE_PATHS) && sameArray(centralNames, PACKAGE_PATHS),
    CODES.noncanonicalContainer,
    'package entry order is not canonical',
  );

  ensure(
    container.end.diskNumber === 0 &&
      container.end.centralDiskNumber === 0 &&
      container.end.entriesOnDisk === PACKAGE_PATHS.length &&
      container.end.totalEntries === PACKAGE_PATHS.length,
    CODES.noncanonicalContainer,
    'end record does not describe one canonical 19-entry disk',
  );

  for (let index = 0; index < PACKAGE_PATHS.length; index += 1) {
    const local = container.localRecords[index];
    const central = container.centralRecords[index];
    const name = PACKAGE_PATHS[index];
    ensure(
      local.versionNeeded === ZIP_VERSION_NEEDED &&
        local.flags === ZIP_UTF8_FLAG &&
        local.method === ZIP_STORE_METHOD &&
        local.dosTime === ZIP_DOS_TIME &&
        local.dosDate === ZIP_DOS_DATE &&
        local.extraLength === 0,
      CODES.noncanonicalContainer,
      `${name} local header has noncanonical ZIP metadata`,
    );
    ensure(
      central.versionMadeBy === ZIP_VERSION_MADE_BY &&
        central.versionNeeded === ZIP_VERSION_NEEDED &&
        central.flags === ZIP_UTF8_FLAG &&
        central.method === ZIP_STORE_METHOD &&
        central.dosTime === ZIP_DOS_TIME &&
        central.dosDate === ZIP_DOS_DATE &&
        central.extraLength === 0 &&
        central.commentLength === 0 &&
        central.diskNumberStart === 0 &&
        central.internalAttributes === 0 &&
        central.externalAttributes === 0,
      CODES.noncanonicalContainer,
      `${name} central header has noncanonical ZIP metadata`,
    );
    ensure(
      local.nameBytes.equals(central.nameBytes) &&
        local.crc32 === central.crc32 &&
        local.compressedSize === central.compressedSize &&
        local.uncompressedSize === central.uncompressedSize &&
        central.localOffset === local.offset,
      CODES.noncanonicalContainer,
      `${name} local and central records disagree`,
    );
  }
}

function validateCrcAndSizes(container) {
  for (const record of container.localRecords) {
    const name = decodeEntryName(record.nameBytes);
    ensure(
      record.compressedSize === record.uncompressedSize &&
        record.compressedSize === record.data.length,
      CODES.crcMismatch,
      `${name} stored sizes disagree`,
    );
    ensure(
      crc32(record.data) === record.crc32,
      CODES.crcMismatch,
      `${name} CRC-32 does not match exact entry bytes`,
    );
  }
}

function payloadTreeFromContainer(container) {
  return new Map(
    container.localRecords.slice(1).map((record) => {
      const name = decodeEntryName(record.nameBytes);
      return [name.slice('payload/'.length), Buffer.from(record.data)];
    }),
  );
}

function parseInnerManifest(tree, packageManifest) {
  const bytes = tree.get('manifest.json');
  const text = decodeUtf8(bytes, CODES.payloadManifestMismatch, 'payload/manifest.json');
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    fail(CODES.payloadManifestMismatch, 'payload manifest is not valid JSON');
  }
  ensure(
    value?.format === packageManifest.payload_format &&
      value?.format_version === packageManifest.payload_format_version,
    CODES.payloadManifestMismatch,
    'inner .roproj manifest disagrees with package payload claims',
  );
  return value;
}

function validateProbeRoprojV1(
  tree,
  canonicalFailureCode = CODES.noncanonicalPayload,
  semanticFailureCode = CODES.invalidSemanticPayload,
) {
  ensure(
    sameArray([...tree.keys()], ROPROJ_PATHS),
    canonicalFailureCode,
    'payload does not have the exact canonical .roproj/v1 path order',
  );
  const manifestText = decodeUtf8(tree.get('manifest.json'), canonicalFailureCode, 'manifest.json');
  let manifest;
  try {
    manifest = JSON.parse(manifestText);
  } catch {
    fail(canonicalFailureCode, 'manifest.json is not valid JSON');
  }
  ensure(
    manifest !== null && typeof manifest === 'object' && !Array.isArray(manifest) &&
      sameArray(Object.keys(manifest), ['format', 'format_version', 'document']) &&
      manifest.document !== null && typeof manifest.document === 'object' &&
      sameArray(Object.keys(manifest.document), ['id', 'title']),
    canonicalFailureCode,
    'manifest.json is not the closed-world .roproj/v1 envelope',
  );
  ensure(
    manifest.format === 'tachiko.roproj' && manifest.format_version === 1,
    canonicalFailureCode,
    'manifest.json does not select .roproj/v1',
  );
  ensure(
    typeof manifest.document.id === 'string' && manifest.document.id.length > 0 &&
      typeof manifest.document.title === 'string',
    semanticFailureCode,
    'probe document identity or title is semantically invalid',
  );
  ensure(
    tree.get('manifest.json').equals(roprojManifestBytes(manifest.document)),
    canonicalFailureCode,
    'manifest.json bytes are noncanonical',
  );
  ensure(
    tree.get('schemas.json').equals(Buffer.from('[]\n', 'utf8')),
    canonicalFailureCode,
    'the focused empty-document fixture requires canonical empty schemas.json',
  );
  for (const path of ENTITY_SHARDS) {
    ensure(tree.get(path).length === 0, canonicalFailureCode, `${path} is not zero bytes`);
  }
  return manifest;
}

function validatePackageBytes(bytes) {
  const container = parseZip32Container(bytes);
  const packageRecord = packageManifestRecord(container);
  const packageManifest = parsePortableManifest(packageRecord.data);
  validateCanonicalZipProfile(container);
  validateCrcAndSizes(container);

  const tree = payloadTreeFromContainer(container);
  const foundRoot = payloadRootSha256(tree);
  ensure(
    foundRoot === packageManifest.payload_root_sha256,
    CODES.integrityMismatch,
    'payload root does not match package.json',
  );
  parseInnerManifest(tree, packageManifest);
  validateProbeRoprojV1(tree);
  return { container, packageManifest, tree, payloadRootSha256: foundRoot };
}

async function pathExists(path) {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

async function exactDirectoryNames(directory) {
  return (await readdir(directory, { withFileTypes: true }))
    .map((entry) => entry.name)
    .sort(compareAscii);
}

async function readCanonicalRoprojV1(root) {
  let rootNames;
  let entityNames;
  try {
    rootNames = await exactDirectoryNames(root);
    entityNames = await exactDirectoryNames(join(root, 'entities'));
  } catch (error) {
    fail(CODES.sourceNotCanonical, `cannot enumerate canonical source tree: ${error.message}`);
  }
  ensure(
    sameArray(rootNames, ['entities', 'manifest.json', 'schemas.json']),
    CODES.sourceNotCanonical,
    'source root does not contain exactly manifest.json, schemas.json, and entities/',
  );
  ensure(
    sameArray(entityNames, [...'0123456789abcdef'].map((bucket) => `${bucket}.jsonl`)),
    CODES.sourceNotCanonical,
    'source entities/ does not contain exactly the 16 canonical shards',
  );

  const tree = new Map();
  for (const path of ROPROJ_PATHS) {
    const file = join(root, path);
    let metadata;
    try {
      metadata = await lstat(file);
    } catch (error) {
      fail(CODES.sourceNotCanonical, `cannot inspect ${path}: ${error.message}`);
    }
    ensure(
      metadata.isFile() && !metadata.isSymbolicLink(),
      CODES.sourceNotCanonical,
      `${path} is not an ordinary regular file`,
    );
    tree.set(path, await readFile(file));
  }
  validateProbeRoprojV1(tree, CODES.sourceNotCanonical, CODES.sourceNotCanonical);
  return tree;
}

function packCanonicalRoprojV1(tree) {
  validateProbeRoprojV1(tree, CODES.sourceNotCanonical, CODES.sourceNotCanonical);
  return buildCanonicalZip32(canonicalPackageEntries(tree));
}

async function writeTree(root, tree) {
  await mkdir(join(root, 'entities'), { recursive: true });
  for (const path of ROPROJ_PATHS) await writeFile(join(root, path), tree.get(path));
}

async function ensureDestinationAbsent(destination) {
  ensure(
    !(await pathExists(destination)),
    CODES.destinationExists,
    `destination already exists: ${destination}`,
  );
}

async function unpackAtomically(bytes, destination) {
  await ensureDestinationAbsent(destination);
  const validated = validatePackageBytes(bytes);
  await mkdir(dirname(destination), { recursive: true });
  let stage;
  try {
    stage = await mkdtemp(join(dirname(destination), '.portable-package-v1-unpack-'));
    await writeTree(stage, validated.tree);
    await ensureDestinationAbsent(destination);
    await rename(stage, destination);
    stage = undefined;
  } catch (error) {
    if (error instanceof ProbeFailure) throw error;
    fail(CODES.publicationFailed, `unpack publication failed: ${error.message}`);
  } finally {
    if (stage !== undefined) await rm(stage, { recursive: true, force: true });
  }
  return validated;
}

async function packAtomically(source, destination) {
  await ensureDestinationAbsent(destination);
  const tree = await readCanonicalRoprojV1(source);
  const bytes = packCanonicalRoprojV1(tree);
  await mkdir(dirname(destination), { recursive: true });
  let stage;
  try {
    stage = await mkdtemp(join(dirname(destination), '.portable-package-v1-pack-'));
    const partial = join(stage, 'artifact.partial');
    const handle = await open(partial, 'wx');
    try {
      await handle.writeFile(bytes);
    } finally {
      await handle.close();
    }
    await ensureDestinationAbsent(destination);
    await rename(partial, destination);
    await rm(stage, { recursive: true });
    stage = undefined;
  } catch (error) {
    if (error instanceof ProbeFailure) throw error;
    fail(CODES.publicationFailed, `pack publication failed: ${error.message}`);
  } finally {
    if (stage !== undefined) await rm(stage, { recursive: true, force: true });
  }
  return bytes;
}

async function comparePackageWithTrackedRoproj(bytes, trackedRoot) {
  const verified = validatePackageBytes(bytes);
  const tracked = await readCanonicalRoprojV1(trackedRoot);
  const trackedRootSha256 = payloadRootSha256(tracked);
  if (trackedRootSha256 !== verified.payloadRootSha256) {
    fail(CODES.sourceMismatch, 'verified package and tracked .roproj roots disagree');
  }
  return { status: 'consistent', payloadRootSha256: trackedRootSha256 };
}

function classifyRepresentation(bytes) {
  if (
    bytes.length >= 4 &&
    bytes[0] === 0x50 && bytes[1] === 0x4b && bytes[2] === 0x03 && bytes[3] === 0x04
  ) {
    return 'portable_package';
  }
  const first = bytes.find((byte) => ![0x09, 0x0a, 0x0d, 0x20].includes(byte));
  return first === 0x7b ? 'direct_ro_json' : 'unknown';
}

function replacePackageManifest(entries, manifestBytes) {
  return entries.map((entry) =>
    entry.name === 'package.json' ? { ...entry, body: manifestBytes } : entry,
  );
}

function replaceEntryBody(entries, name, body) {
  return entries.map((entry) => (entry.name === name ? { ...entry, body } : entry));
}

function corruptStoredByte(bytes, entryName) {
  const container = parseZip32Container(bytes);
  const record = container.localRecords.find(
    (candidate) => decodeEntryName(candidate.nameBytes) === entryName,
  );
  assert(record, `missing mutation target ${entryName}`);
  assert(record.data.length > 0, `${entryName} must be nonempty for corruption`);
  const corrupted = Buffer.from(bytes);
  corrupted[record.dataStart] ^= 1;
  return corrupted;
}

async function expectUnpackRejection(bytes, destination, expectedCode) {
  await assert.rejects(
    () => unpackAtomically(bytes, destination),
    (error) => error instanceof ProbeFailure && error.code === expectedCode,
    `expected ${expectedCode}`,
  );
  assert.equal(await pathExists(destination), false, `${destination} must remain absent`);
  return expectedCode;
}

async function expectFailure(operation, expectedCode) {
  await assert.rejects(
    operation,
    (error) => error instanceof ProbeFailure && error.code === expectedCode,
    `expected ${expectedCode}`,
  );
  return expectedCode;
}

async function assertTreesEqual(leftRoot, rightRoot) {
  const left = await readCanonicalRoprojV1(leftRoot);
  const right = await readCanonicalRoprojV1(rightRoot);
  assert.deepEqual([...left.keys()], [...right.keys()]);
  for (const path of ROPROJ_PATHS) assert(left.get(path).equals(right.get(path)), path);
}

async function loadGoldenBytes() {
  const encoded = await readFile(GOLDEN_HEX_PATH, 'utf8');
  assert.match(encoded, /^[0-9a-f]+\n$/u, 'golden vector must be one lowercase hex line');
  const hex = encoded.trimEnd();
  assert.equal(hex.length % 2, 0, 'golden hex must contain whole bytes');
  return Buffer.from(hex, 'hex');
}

async function createCanonicalCopy(source, destination, seconds, mode) {
  await cp(source, destination, { recursive: true, errorOnExist: true });
  for (const path of ROPROJ_PATHS) {
    const file = join(destination, path);
    await chmod(file, mode);
    await utimes(file, seconds, seconds);
  }
}

async function runPressureTests(work, fixtureTree, canonicalBytes) {
  const entries = canonicalPackageEntries(fixtureTree);
  const packageManifest = portableManifestObject(payloadRootSha256(fixtureTree));
  const rejections = {};

  const first = packCanonicalRoprojV1(fixtureTree);
  const second = packCanonicalRoprojV1(fixtureTree);
  assert(first.equals(second), 'repeated in-memory pack differs');

  const hostA = join(work, 'alpha.roproj');
  const hostB = join(work, 'beta.roproj');
  await createCanonicalCopy(EMPTY_ROPROJ_DIRECTORY, hostA, 946684800, 0o600);
  await createCanonicalCopy(EMPTY_ROPROJ_DIRECTORY, hostB, 1893456000, 0o644);
  const hostBytesA = packCanonicalRoprojV1(await readCanonicalRoprojV1(hostA));
  const hostBytesB = packCanonicalRoprojV1(await readCanonicalRoprojV1(hostB));
  assert(hostBytesA.equals(hostBytesB), 'host basename, mode, or mtime affected package bytes');

  const packedAPath = join(work, 'packed-a.ro');
  const packedBPath = join(work, 'packed-b.ro');
  const packedA = await packAtomically(hostA, packedAPath);
  const packedB = await packAtomically(hostB, packedBPath);
  assert(packedA.equals(packedB));
  assert((await readFile(packedAPath)).equals(canonicalBytes));

  const unpacked = join(work, 'unpacked.roproj');
  await unpackAtomically(canonicalBytes, unpacked);
  await assertTreesEqual(EMPTY_ROPROJ_DIRECTORY, unpacked);

  rejections.corruptPayload = await expectUnpackRejection(
    corruptStoredByte(canonicalBytes, 'payload/schemas.json'),
    join(work, 'reject-corrupt.roproj'),
    CODES.crcMismatch,
  );

  const staleRootBytes = buildCanonicalZip32(
    replaceEntryBody(entries, 'payload/schemas.json', Buffer.from('{}\n', 'utf8')),
  );
  rejections.staleIntegrityRoot = await expectUnpackRejection(
    staleRootBytes,
    join(work, 'reject-stale-root.roproj'),
    CODES.integrityMismatch,
  );

  const missingRoot = { ...packageManifest };
  delete missingRoot.payload_root_sha256;
  rejections.missingMetadata = await expectUnpackRejection(
    buildCanonicalZip32(replacePackageManifest(entries, prettyJsonBytes(missingRoot))),
    join(work, 'reject-missing-metadata.roproj'),
    CODES.invalidManifest,
  );

  const malformedRoot = { ...packageManifest, payload_root_sha256: 'ABC' };
  rejections.malformedMetadata = await expectUnpackRejection(
    buildCanonicalZip32(replacePackageManifest(entries, prettyJsonBytes(malformedRoot))),
    join(work, 'reject-malformed-metadata.roproj'),
    CODES.invalidManifest,
  );

  const unsupportedManifest = { ...packageManifest, format_version: 2 };
  const unsupportedEntries = replaceEntryBody(
    replacePackageManifest(entries, prettyJsonBytes(unsupportedManifest)),
    'payload/schemas.json',
    Buffer.from('{}\n', 'utf8'),
  );
  rejections.unsupportedVersion = await expectUnpackRejection(
    buildCanonicalZip32(unsupportedEntries),
    join(work, 'reject-unsupported.roproj'),
    CODES.unsupportedPackageVersion,
  );

  const unknownMetadata = { ...packageManifest, unknown: true };
  rejections.unknownMetadata = await expectUnpackRejection(
    buildCanonicalZip32(replacePackageManifest(entries, prettyJsonBytes(unknownMetadata))),
    join(work, 'reject-unknown-metadata.roproj'),
    CODES.invalidManifest,
  );
  const duplicateMetadata = Buffer.from(
    portableManifestBytes(packageManifest.payload_root_sha256)
      .toString('utf8')
      .replace(
        '  "payload_root_sha256":',
        `  "payload_root_sha256": "${packageManifest.payload_root_sha256}",\n` +
          '  "payload_root_sha256":',
      ),
    'utf8',
  );
  rejections.duplicateMetadata = await expectUnpackRejection(
    buildCanonicalZip32(replacePackageManifest(entries, duplicateMetadata)),
    join(work, 'reject-duplicate-metadata.roproj'),
    CODES.invalidManifest,
  );

  rejections.missingEntry = await expectUnpackRejection(
    buildCanonicalZip32(entries.filter(({ name }) => name !== 'payload/entities/f.jsonl')),
    join(work, 'reject-missing-entry.roproj'),
    CODES.entrySetMismatch,
  );
  rejections.unknownEntry = await expectUnpackRejection(
    buildCanonicalZip32([...entries, { name: 'payload/extra.json', body: Buffer.from('{}\n') }]),
    join(work, 'reject-unknown-entry.roproj'),
    CODES.entrySetMismatch,
  );
  rejections.duplicateEntry = await expectUnpackRejection(
    buildCanonicalZip32([
      ...entries,
      entries.find(({ name }) => name === 'payload/schemas.json'),
    ]),
    join(work, 'reject-duplicate-entry.roproj'),
    CODES.entrySetMismatch,
  );
  rejections.aliasedEntry = await expectUnpackRejection(
    buildCanonicalZip32(
      entries.map((entry) =>
        entry.name === 'payload/schemas.json'
          ? { ...entry, name: 'payload\\schemas.json' }
          : entry,
      ),
    ),
    join(work, 'reject-aliased-entry.roproj'),
    CODES.entrySetMismatch,
  );

  const consistent = await comparePackageWithTrackedRoproj(
    canonicalBytes,
    EMPTY_ROPROJ_DIRECTORY,
  );
  assert.equal(consistent.status, 'consistent');
  const changedTree = new Map(fixtureTree);
  changedTree.set(
    'manifest.json',
    roprojManifestBytes({ id: 'doc-empty', title: 'Changed' }),
  );
  const changedTracked = join(work, 'changed.roproj');
  await mkdir(changedTracked);
  await writeTree(changedTracked, changedTree);
  const beforeMismatch = await readCanonicalRoprojV1(changedTracked);
  rejections.sourceMismatch = await expectFailure(
    () => comparePackageWithTrackedRoproj(canonicalBytes, changedTracked),
    CODES.sourceMismatch,
  );
  const afterMismatch = await readCanonicalRoprojV1(changedTracked);
  for (const path of ROPROJ_PATHS) {
    assert(beforeMismatch.get(path).equals(afterMismatch.get(path)));
  }

  rejections.noncanonicalMetadata = await expectUnpackRejection(
    buildCanonicalZip32(entries, { dosTime: 1 }),
    join(work, 'reject-noncanonical-metadata.roproj'),
    CODES.noncanonicalContainer,
  );
  const reordered = [...entries];
  [reordered[1], reordered[2]] = [reordered[2], reordered[1]];
  rejections.noncanonicalOrder = await expectUnpackRejection(
    buildCanonicalZip32(reordered),
    join(work, 'reject-noncanonical-order.roproj'),
    CODES.noncanonicalContainer,
  );
  rejections.trailingData = await expectUnpackRejection(
    Buffer.concat([canonicalBytes, Buffer.from([0])]),
    join(work, 'reject-trailing.roproj'),
    CODES.invalidContainer,
  );

  const repacked = packCanonicalRoprojV1(await readCanonicalRoprojV1(unpacked));
  assert(repacked.equals(canonicalBytes), 'pack -> unpack -> pack bytes differ');

  const zeroShardEntries = validatePackageBytes(canonicalBytes).container.localRecords.filter(
    (record) => /^payload\/entities\/[0-9a-f]\.jsonl$/u.test(
      decodeEntryName(record.nameBytes),
    ),
  );
  assert.equal(zeroShardEntries.length, 16);
  assert(zeroShardEntries.every((record) => record.data.length === 0));
  const rootWithoutZeroShards = payloadRootSha256(fixtureTree, [
    'manifest.json',
    'schemas.json',
  ]);
  assert.notEqual(rootWithoutZeroShards, payloadRootSha256(fixtureTree));

  const occupiedDestination = join(work, 'occupied.roproj');
  await mkdir(occupiedDestination);
  await writeFile(join(occupiedDestination, 'sentinel'), 'unchanged\n');
  rejections.unpackDestinationExists = await expectFailure(
    () => unpackAtomically(canonicalBytes, occupiedDestination),
    CODES.destinationExists,
  );
  assert.equal(
    await readFile(join(occupiedDestination, 'sentinel'), 'utf8'),
    'unchanged\n',
  );

  const occupiedPackDestination = join(work, 'occupied.ro');
  await writeFile(occupiedPackDestination, 'unchanged\n');
  rejections.packDestinationExists = await expectFailure(
    () => packAtomically(hostA, occupiedPackDestination),
    CODES.destinationExists,
  );
  assert.equal(await readFile(occupiedPackDestination, 'utf8'), 'unchanged\n');

  const invalidSource = join(work, 'invalid-source.roproj');
  await cp(EMPTY_ROPROJ_DIRECTORY, invalidSource, {
    recursive: true,
    errorOnExist: true,
  });
  await writeFile(join(invalidSource, 'unknown.txt'), 'unknown\n');
  const invalidPackDestination = join(work, 'invalid-source.ro');
  rejections.noncanonicalSource = await expectFailure(
    () => packAtomically(invalidSource, invalidPackDestination),
    CODES.sourceNotCanonical,
  );
  assert.equal(await pathExists(invalidPackDestination), false);

  assert.equal(classifyRepresentation(canonicalBytes), 'portable_package');
  assert.equal(
    classifyRepresentation(Buffer.from(' {"format_version":2}\n')),
    'direct_ro_json',
  );
  assert.equal(classifyRepresentation(Buffer.from('not-a-format')), 'unknown');

  return {
    repeatedPackByteIdentical: true,
    hostMetadataIgnored: true,
    validUnpackExact: true,
    rejections,
    consistentTrackedSource: consistent,
    packUnpackPackByteIdentical: true,
    zeroByteShards: {
      entryCount: zeroShardEntries.length,
      allZeroBytes: true,
      includedInPayloadRoot: true,
    },
    contentFraming: {
      package: 'portable_package',
      directJson: 'direct_ro_json',
      malformedPackageFallback: false,
    },
  };
}

async function buildCanonicalFixturePackage() {
  const tree = await readCanonicalRoprojV1(EMPTY_ROPROJ_DIRECTORY);
  return { tree, bytes: packCanonicalRoprojV1(tree) };
}

function parseArguments(arguments_) {
  if (arguments_.length === 0) return { keepDirectory: null, printGolden: false };
  if (arguments_.length === 1 && arguments_[0] === '--print-golden') {
    return { keepDirectory: null, printGolden: true };
  }
  if (arguments_.length === 2 && arguments_[0] === '--keep-dir') {
    return { keepDirectory: resolve(arguments_[1]), printGolden: false };
  }
  throw new Error(
    'usage: node issue-43-portable-package-v1.mjs [--print-golden | --keep-dir <new-path>]',
  );
}

async function main() {
  const arguments_ = parseArguments(process.argv.slice(2));
  const canonical = await buildCanonicalFixturePackage();
  if (arguments_.printGolden) {
    process.stdout.write(`${canonical.bytes.toString('hex')}\n`);
    return;
  }

  const golden = await loadGoldenBytes();
  assert(
    canonical.bytes.equals(golden),
    'manual ZIP32 encoder differs from checked-in golden bytes',
  );
  const work = arguments_.keepDirectory ??
    await mkdtemp(join(tmpdir(), 'issue-43-portable-package-v1-'));
  if (arguments_.keepDirectory !== null) await mkdir(work);
  try {
    const pressureTests = await runPressureTests(
      work,
      canonical.tree,
      canonical.bytes,
    );
    const validated = validatePackageBytes(canonical.bytes);
    const packagePath = join(work, 'empty-package-v1.ro');
    if (!(await pathExists(packagePath))) await writeFile(packagePath, canonical.bytes);
    const packageMetadata = await stat(packagePath);
    process.stdout.write(
      `${JSON.stringify({
        probe: 'issue-43-portable-package-v1',
        profile: PACKAGE_PROFILE,
        payloadProfile: PAYLOAD_PROFILE,
        canonicalPaths: ROPROJ_PATHS,
        payloadRootSha256: validated.payloadRootSha256,
        packageGolden: {
          byteLength: canonical.bytes.length,
          sha256: sha256Hex(canonical.bytes),
          packageManifestByteLength: validated.container.localRecords[0].data.length,
          centralDirectoryOffset: validated.container.end.centralOffset,
          centralDirectoryByteLength: validated.container.end.centralSize,
          retainedByteLength: packageMetadata.size,
        },
        pressureTests,
      })}\n`,
    );
  } finally {
    if (arguments_.keepDirectory === null) {
      await rm(work, { recursive: true, force: true });
    }
  }
}

await main();
