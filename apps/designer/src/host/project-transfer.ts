const PROJECT_BUNDLE_MAGIC = new TextEncoder().encode("TWDPROJ1");
const MAX_PROJECT_TRANSFER_BYTES = 64 * 1024 * 1024;

type HostProjectFile = {
  path: Uint8Array;
  bytes: Uint8Array;
};

/// Pack user-selected directory entries into the app-private host/WASM arena.
/// This is a generic path/byte transfer DTO, not a `.roproj` parser or codec;
/// Rust storage remains the sole authority for the canonical tree and meaning.
export async function projectTransferFromFiles(files: FileList): Promise<ArrayBuffer> {
  if (files.length === 0) throw new Error("No project directory was selected.");
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
