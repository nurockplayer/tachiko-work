const PROJECT_BUNDLE_MAGIC = new TextEncoder().encode("TWDPROJ1");
const MAX_PROJECT_TRANSFER_BYTES = 64 * 1024 * 1024;
const CANONICAL_PROJECT_FILE_COUNT = 18;
const MAX_PROJECT_TRANSFER_FILES = 1_024;

type HostProjectFile = {
  path: Uint8Array;
  bytes: Uint8Array;
};

export type CanonicalProjectTransferEntry = {
  path: string;
  bytes: ArrayBuffer;
};

/// Pack user-selected directory entries into the app-private host/WASM arena.
/// This is a generic path/byte transfer DTO, not a `.roproj` parser or codec;
/// Rust storage remains the sole authority for the canonical tree and meaning.
export async function projectTransferFromFiles(files: FileList): Promise<ArrayBuffer> {
  if (files.length === 0) throw new Error("No project directory was selected.");
  if (files.length !== CANONICAL_PROJECT_FILE_COUNT) {
    throw new Error("A canonical .roproj/v1 directory must contain exactly 18 files.");
  }
  const first = files.item(0);
  if (first === null) throw new Error("No project directory was selected.");
  const root = rootDirectory(first);
  const encoder = new TextEncoder();
  const transferred: HostProjectFile[] = [];
  let total = PROJECT_BUNDLE_MAGIC.byteLength + 4;

  for (const file of files) {
    const relativePath = relativeFilePath(file, root);
    const path = encoder.encode(relativePath);
    if (path.byteLength > 65_535) {
      throw new Error("A selected project path exceeds the private host transfer profile.");
    }
    const nextTotal = total + 2 + 4 + path.byteLength + file.size;
    if (nextTotal > MAX_PROJECT_TRANSFER_BYTES) {
      throw new Error("The selected project exceeds the 64 MiB host transfer boundary.");
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    total = nextTotal;
    transferred.push({ path, bytes });
  }

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
  if (count !== CANONICAL_PROJECT_FILE_COUNT) {
    throw new Error("The canonical project transfer must contain exactly 18 files.");
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
    components.length === 0 ||
    components.some((component) =>
      component.length === 0 || component === "." || component === ".."
    )
  ) {
    throw new Error("The selected files do not form one safe project directory.");
  }
  return components.join("/");
}

function isSafeRelativePath(path: string): boolean {
  return path.length > 0 && !path.startsWith("/") && !path.includes("\\") &&
    path.split("/").every(component => component.length > 0 && component !== "." && component !== "..");
}
