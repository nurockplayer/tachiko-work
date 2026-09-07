import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

type ExportedType = {
  identifier?: unknown;
  conformsTo?: unknown;
};

type FileAssociation = {
  ext?: unknown;
  exportedType?: ExportedType;
};

type TauriConfig = {
  identifier?: unknown;
  build?: {
    frontendDist?: unknown;
  };
  bundle?: {
    fileAssociations?: unknown;
  };
};

const DESIGNER_ROOT = fileURLToPath(new URL("../", import.meta.url));
const TAURI_CONFIG = resolve(DESIGNER_ROOT, "src-tauri", "tauri.conf.json");
const DESIGNER_DIST = resolve(DESIGNER_ROOT, "dist");

function readTauriConfig(): TauriConfig | null {
  if (!existsSync(TAURI_CONFIG)) return null;
  return JSON.parse(readFileSync(TAURI_CONFIG, "utf8")) as TauriConfig;
}

function associations(config: TauriConfig): FileAssociation[] {
  const value = config.bundle?.fileAssociations;
  expect(Array.isArray(value), "Tauri bundle.fileAssociations must be an array").toBe(true);
  return Array.isArray(value) ? value as FileAssociation[] : [];
}

describe("macOS desktop shell distribution acceptance", () => {
  it("declares one custom .ro association while serving the existing Designer build", () => {
    const config = readTauriConfig();
    expect(
      config,
      "production apps/designer/src-tauri/tauri.conf.json must exist before the desktop shell is delivered",
    ).not.toBeNull();
    if (config === null) return;

    expect(typeof config.identifier).toBe("string");
    expect(String(config.identifier)).not.toHaveLength(0);

    const frontendDist = config.build?.frontendDist;
    expect(typeof frontendDist, "desktop shell must serve the existing Designer build output").toBe(
      "string",
    );
    if (typeof frontendDist === "string") {
      expect(resolve(dirname(TAURI_CONFIG), frontendDist)).toBe(DESIGNER_DIST);
    }

    const roAssociations = associations(config).filter((association) =>
      Array.isArray(association.ext) && association.ext.includes("ro")
    );
    expect(roAssociations, "exactly one desktop association must own the .ro extension").toHaveLength(1);

    const association = roAssociations[0];
    expect(association?.ext).toEqual(["ro"]);
    expect(typeof association?.exportedType?.identifier).toBe("string");
    expect(String(association?.exportedType?.identifier ?? "").split(".").length).toBeGreaterThanOrEqual(3);
    expect(association?.exportedType?.conformsTo).toEqual(expect.arrayContaining(["public.data"]));
  });
});
