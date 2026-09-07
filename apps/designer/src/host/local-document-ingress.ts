/** Opaque browser file-handle boundary for one host-launched local document. */
export type LocalDocumentHandle = {
  kind: string;
  name: string;
  getFile(): Promise<File>;
};

export type LocalDocument = {
  name: string;
  bytes: ArrayBuffer;
};

/**
 * Read exactly one `.ro` file without interpreting its contents.
 *
 * Storage admission remains in the Rust runtime. This host boundary only
 * validates the launch shape and transfers the exact raw bytes.
 */
export async function readSingleLocalRoDocument(
  handles: readonly LocalDocumentHandle[],
): Promise<LocalDocument> {
  if (handles.length === 0) {
    throw new Error("No local .ro file was provided by the operating system.");
  }
  if (handles.length !== 1) {
    throw new Error("Open exactly one local .ro file at a time.");
  }

  const handle = handles[0];
  if (handle === undefined || handle.kind !== "file") {
    throw new Error("The local launch did not provide a readable file.");
  }
  if (!handle.name.toLowerCase().endsWith(".ro")) {
    throw new Error("Only local .ro files can be opened by this launch.");
  }

  const file = await handle.getFile();
  return { name: handle.name, bytes: await file.arrayBuffer() };
}
