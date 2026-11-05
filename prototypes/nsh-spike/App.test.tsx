// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
//
// Both tests go through app-level test ids, not presses on table cells: the
// DataTable's hit layer is its own element, so the app offers deterministic
// entry points (the selection's editor on Screen A, the "jump to the last
// cell" button on Screen B) and the tests use those.
import { expect, open, test } from "nsh/test";

test("work table: open fixture, edit a cell, save, reopen", async () => {
  const app = await open(".");
  const contains = (part: string) =>
    app.texts().some((text) => text.includes(part));

  await app.press("id:open-fixture");
  expect(contains("opened Moonfall: starter balance")).toBe(true);

  // The open filled the editor with the first editable text cell; commit a
  // change through the Work runtime's edit_scalar.
  await app.clear("id:cell-editor");
  await app.type("id:cell-editor", "Alric spike");
  await app.press("id:commit");
  expect(contains("edited ")).toBe(true);
  expect(contains("Alric spike")).toBe(true);

  // Export the resident revision, write it, and open the file again.
  await app.press("id:save");
  expect(contains("saved ")).toBe(true);
  await app.press("id:reopen");
  expect(app.texts()).toContain("reopened: Alric spike");
});

test("grid stress: jump to row 99,999 column 49 and edit it", async () => {
  const app = await open(".");
  const contains = (part: string) =>
    app.texts().some((text) => text.includes(part));

  // Both screens stay mounted (one hidden by a style binding), so the grid's
  // controls exist before the switch; pressing them proves the switch made
  // them visible and that the deep cell is reachable.
  await app.press("id:tab-grid");
  await app.press("id:jump-last");
  await app.clear("id:grid-editor");
  await app.type("id:grid-editor", "EDITED-DEEP");
  await app.press("id:grid-commit");
  expect(app.texts()).toContain("edited R99999C49 = EDITED-DEEP");
});
