import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, it } from "vitest";
import inventory from "./fixtures/interop/hostile/inventory.json";

const root = resolve(process.cwd(), "tests/fixtures/interop/hostile");
const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

it("pins the hostile estate to its original reference workbook and exact archive bytes", () => {
  expect(inventory.source_file).toBe("../reference-two-sheet.xlsx");
  const source = readFileSync(resolve(root, inventory.source_file));
  expect(sha256(source)).toBe(inventory.source_sha256);
  expect(inventory.source_sha256).toBe("81cce4bdfecc8e9832a48cfff4b6e83818c1f169d84d339d9793954655800d63");
  expect(sha256(readFileSync(resolve(root, "../ordinary-two-sheet.xlsx")))).not.toBe(inventory.source_sha256);
  expect(inventory.fixtures).toHaveLength(8);
  expect(new Set(inventory.fixtures.map(fixture => fixture.file)).size).toBe(8);
  for (const fixture of inventory.fixtures) {
    const bytes = readFileSync(resolve(root, fixture.file));
    expect(sha256(bytes), fixture.file).toBe(fixture.sha256);
    expect(bytes.length, fixture.file).toBe(fixture.compressed_bytes);
    expect(fixture.reference_tool_opened).toBe(false);
  }
});
