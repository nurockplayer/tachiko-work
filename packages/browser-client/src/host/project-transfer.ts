const PROJECT_BUNDLE_MAGIC = new TextEncoder().encode("TWDPROJ1");
const MAX_PROJECT_TRANSFER_BYTES = 64 * 1024 * 1024;
const CANONICAL_PROJECT_FILE_COUNTS = [18, 19];
const MAX_PROJECT_TRANSFER_FILES = 1_024;

// BEGIN generated Rust storage paths
// Run: node packages/browser-client/scripts/generate-project-paths.mjs
const CANONICAL_PROJECT_PATHS: Readonly<Record<number, readonly string[]>> = {
  1: ["manifest.json","schemas.json","entities/0.jsonl","entities/1.jsonl","entities/2.jsonl","entities/3.jsonl","entities/4.jsonl","entities/5.jsonl","entities/6.jsonl","entities/7.jsonl","entities/8.jsonl","entities/9.jsonl","entities/a.jsonl","entities/b.jsonl","entities/c.jsonl","entities/d.jsonl","entities/e.jsonl","entities/f.jsonl"],
  2: ["manifest.json","schemas.json","entities/0.jsonl","entities/1.jsonl","entities/2.jsonl","entities/3.jsonl","entities/4.jsonl","entities/5.jsonl","entities/6.jsonl","entities/7.jsonl","entities/8.jsonl","entities/9.jsonl","entities/a.jsonl","entities/b.jsonl","entities/c.jsonl","entities/d.jsonl","entities/e.jsonl","entities/f.jsonl","definitions.json"],
  3: ["manifest.json","schemas.json","definitions.json","entities/0.jsonl","entities/1.jsonl","entities/2.jsonl","entities/3.jsonl","entities/4.jsonl","entities/5.jsonl","entities/6.jsonl","entities/7.jsonl","entities/8.jsonl","entities/9.jsonl","entities/a.jsonl","entities/b.jsonl","entities/c.jsonl","entities/d.jsonl","entities/e.jsonl","entities/f.jsonl"],
};
// END generated Rust storage paths

type HostProjectFile = {
  path: Uint8Array;
  bytes: Uint8Array;
};

export type CanonicalProjectTransferEntry = {
  path: string;
  bytes: ArrayBuffer;
};

/// Pack user-selected directory entries into the app-private host/WASM arena.
/// Validate exact directory membership and package it in Rust storage's order.
/// Only manifest version dispatch happens here; Rust still admits all content
/// and canonical bytes. Opaque transfer framing remains available separately.
export async function projectTransferFromFiles(files: FileList): Promise<ArrayBuffer> {
  if (files.length === 0) throw new Error("No project directory was selected.");
  if (!CANONICAL_PROJECT_FILE_COUNTS.includes(files.length)) {
    throw new Error("A canonical .roproj directory must contain exactly 18 or 19 files.");
  }
  const first = files.item(0);
  if (first === null) throw new Error("No project directory was selected.");
  const root = rootDirectory(first);
  const encoder = new TextEncoder();
  const filesByPath = new Map<string, File>();
  let total = PROJECT_BUNDLE_MAGIC.byteLength + 4;

  for (const file of files) {
    const relativePath = relativeFilePath(file, root);
    if (filesByPath.has(relativePath)) {
      throw new Error(`The selected project contains duplicate path '${relativePath}'.`);
    }
    const path = encoder.encode(relativePath);
    if (path.byteLength > 65_535) {
      throw new Error("A selected project path exceeds the private host transfer profile.");
    }
    const nextTotal = total + 2 + 4 + path.byteLength + file.size;
    if (nextTotal > MAX_PROJECT_TRANSFER_BYTES) {
      throw new Error("The selected project exceeds the 64 MiB host transfer boundary.");
    }
    total = nextTotal;
    filesByPath.set(relativePath, file);
  }

  // Check membership before reading payloads, then dispatch from the manifest.
  // v2 and v3 have the same set but different storage ordering.
  const possiblePaths = CANONICAL_PROJECT_PATHS[files.length === 18 ? 1 : 3]!;
  assertExactPaths(filesByPath, possiblePaths);
  const manifestBytes = new Uint8Array(await filesByPath.get("manifest.json")!.arrayBuffer());
  let version: unknown;
  try {
    const manifest: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(manifestBytes));
    if (typeof manifest === "object" && manifest !== null && "format_version" in manifest) {
      version = manifest.format_version;
    }
  } catch {
    throw new Error("The selected project manifest is not valid UTF-8 JSON.");
  }
  const paths = typeof version === "number" ? CANONICAL_PROJECT_PATHS[version] : undefined;
  if (paths === undefined) throw new Error("The selected project manifest has no supported format version (1, 2 or 3).");
  assertExactPaths(filesByPath, paths);
  const transferred: HostProjectFile[] = [];
  for (const path of paths) {
    const bytes = path === "manifest.json" ? manifestBytes
      : new Uint8Array(await filesByPath.get(path)!.arrayBuffer());
    transferred.push({ path: encoder.encode(path), bytes });
  }
  return encodeTransfer(transferred, total);
}

function assertExactPaths(files: ReadonlyMap<string, File>, paths: readonly string[]): void {
  for (const path of files.keys()) {
    if (!paths.includes(path)) throw new Error(`The selected project contains unexpected path '${path}'.`);
  }
  for (const path of paths) {
    if (!files.has(path)) throw new Error(`The selected project is missing '${path}'.`);
  }
}

/**
 * Encode opaque canonical path/byte entries for the private host/WASM arena.
 * This validates only transport shape; Rust remains the `.roproj` authority.
 */
export function projectTransferFromEntries(
  entries: readonly CanonicalProjectTransferEntry[],
): ArrayBuffer {
  if (entries.length > MAX_PROJECT_TRANSFER_FILES) {
    throw new Error("The selected project exceeds the private host transfer file limit.");
  }
  const encoder = new TextEncoder();
  const transferred: HostProjectFile[] = [];
  const paths = new Set<string>();
  let total = PROJECT_BUNDLE_MAGIC.byteLength + 4;
  for (const entry of entries) {
    if (!isSafeRelativePath(entry.path) || paths.has(entry.path)) {
      throw new Error("The canonical project entries do not form one safe project directory.");
    }
    paths.add(entry.path);
    const path = encoder.encode(entry.path);
    if (path.byteLength > 65_535) {
      throw new Error("A selected project path exceeds the private host transfer profile.");
    }
    const bytes = new Uint8Array(entry.bytes);
    const nextTotal = total + 2 + 4 + path.byteLength + bytes.byteLength;
    if (nextTotal > MAX_PROJECT_TRANSFER_BYTES) {
      throw new Error("The selected project exceeds the 64 MiB host transfer boundary.");
    }
    total = nextTotal;
    transferred.push({ path, bytes });
  }
  return encodeTransfer(transferred, total);
}

/** Decode an app-private host transfer into opaque path/byte entries. */
export function projectTransferToEntries(input: ArrayBuffer): CanonicalProjectTransferEntry[] {
  const bytes = new Uint8Array(input);
  if (bytes.byteLength < PROJECT_BUNDLE_MAGIC.byteLength + 4 ||
    !PROJECT_BUNDLE_MAGIC.every((value, index) => bytes[index] === value)) {
    throw new Error("The canonical project transfer is invalid.");
  }
  const view = new DataView(input);
  const count = view.getUint32(PROJECT_BUNDLE_MAGIC.byteLength, true);
  if (!CANONICAL_PROJECT_FILE_COUNTS.includes(count)) {
    throw new Error("The canonical project transfer must contain exactly 18 or 19 files.");
  }
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const entries: CanonicalProjectTransferEntry[] = [];
  const paths = new Set<string>();
  let offset = PROJECT_BUNDLE_MAGIC.byteLength + 4;
  for (let index = 0; index < count; index += 1) {
    if (offset + 6 > bytes.byteLength) throw new Error("The canonical project transfer is truncated.");
    const pathLength = view.getUint16(offset, true); offset += 2;
    const byteLength = view.getUint32(offset, true); offset += 4;
    const end = offset + pathLength + byteLength;
    if (!Number.isSafeInteger(end) || end > bytes.byteLength) throw new Error("The canonical project transfer is truncated.");
    const path = decoder.decode(bytes.slice(offset, offset + pathLength)); offset += pathLength;
    if (!isSafeRelativePath(path) || paths.has(path)) throw new Error("The canonical project transfer has invalid paths.");
    paths.add(path);
    entries.push({ path, bytes: bytes.slice(offset, end).buffer }); offset = end;
  }
  if (offset !== bytes.byteLength) throw new Error("The canonical project transfer has trailing bytes.");
  return entries;
}

function encodeTransfer(transferred: readonly HostProjectFile[], total: number): ArrayBuffer {
  const output = new Uint8Array(total);
  const view = new DataView(output.buffer);
  let offset = 0;
  output.set(PROJECT_BUNDLE_MAGIC, offset);
  offset += PROJECT_BUNDLE_MAGIC.byteLength;
  view.setUint32(offset, transferred.length, true);
  offset += 4;
  for (const file of transferred) {
    view.setUint16(offset, file.path.byteLength, true);
    offset += 2;
    view.setUint32(offset, file.bytes.byteLength, true);
    offset += 4;
    output.set(file.path, offset);
    offset += file.path.byteLength;
    output.set(file.bytes, offset);
    offset += file.bytes.byteLength;
  }
  return output.buffer;
}

function rootDirectory(file: File): string {
  const [root, ...relative] = file.webkitRelativePath.split("/");
  if (root === undefined || root.length === 0 || relative.length === 0) {
    throw new Error("Select the complete .roproj directory, not individual files.");
  }
  return root;
}

function relativeFilePath(file: File, expectedRoot: string): string {
  const [root, ...components] = file.webkitRelativePath.split("/");
  if (
    root !== expectedRoot ||
    !isSafeRelativePath(components.join("/"))
  ) {
    throw new Error("The selected files do not form one safe project directory.");
  }
  return components.join("/");
}

function isSafeRelativePath(path: string): boolean {
  return path.length > 0 && !path.startsWith("/") && !path.includes("\\") &&
    path.split("/").every(component => component.length > 0 && component !== "." && component !== "..");
}
