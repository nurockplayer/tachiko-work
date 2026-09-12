import {
  createExperimentalDesignerClient,
  preflightCanonicalProjectEntries,
  projectTransferFromEntries,
  type CanonicalProjectTransferEntry,
  type CanonicalTreeExport,
  type OccurrenceProjection,
  type OpenedProjection,
  type ProjectExport,
} from "../vendor/tachiko/experimental-client.js";

/**
 * Compile-only consumer contract for the packaged public entry.
 *
 * Keep each call direct: optional capability guards here would hide a
 * regression in the factory's consumer-facing return type.
 */
export async function exercisePackagedClientContract(): Promise<{
  canonical: CanonicalTreeExport;
  portable: ProjectExport;
  canonicalOpened: OpenedProjection;
  portableOpened: OpenedProjection;
  occurrence: OccurrenceProjection;
}> {
  const client = createExperimentalDesignerClient();
  const canonical = await client.exportCanonicalTree("resident/0");
  const portable = await client.exportPortableRo(canonical.revision);
  await client.verifyPortableRo(portable.bytes);
  const canonicalOpened = await client.openCanonicalTree(canonical.files);
  const portableOpened = await client.openPortableRo(portable.bytes);
  const occurrence = await client.observeOccurrence();
  await client.closeProject();
  client.close();
  return {canonical, portable, canonicalOpened, portableOpened, occurrence};
}

/** Compile-only external consumer check for the public canonical preflight. */
export async function exercisePackagedCanonicalPreflight(
  entries: readonly CanonicalProjectTransferEntry[],
): Promise<OpenedProjection> {
  const client = createExperimentalDesignerClient();
  try {
    preflightCanonicalProjectEntries(entries);
    const transfer = projectTransferFromEntries(entries);
    return await client.openProject(transfer);
  } finally {
    await client.closeProject().catch(() => undefined);
    client.close();
  }
}
