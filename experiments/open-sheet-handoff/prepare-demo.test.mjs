import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { repositoryRoot } from "./prepare-demo-paths.mjs";

const script = fileURLToPath(new URL("./prepare-demo.mjs", import.meta.url));

test("prepare-demo resolves its export script root from encoded checkout paths", () => {
  const modulePath = join("/tmp", "Tachiko 台灣 demo", "experiments", "open-sheet-handoff", "prepare-demo.mjs");
  assert.equal(
    repositoryRoot(pathToFileURL(modulePath).href),
    join("/tmp", "Tachiko 台灣 demo"),
  );
});

test("prepare-demo refuses an existing output without touching it", () => {
  const parent = mkdtempSync(join(tmpdir(), "tachiko-handoff-prepare-"));
  const output = join(parent, "existing-demo");
  mkdirSync(output);
  try {
    const result = spawnSync(process.execPath, [script, "--output", output], {
      encoding: "utf8", env: { ...process.env, TACHIKO_BIN: "/definitely/not/tachiko" },
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /output already exists/u);
    assert.deepEqual(readdirSync(output), []);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});

test("prepare-demo cleans failed staging before publishing an output", () => {
  const parent = mkdtempSync(join(tmpdir(), "tachiko-handoff-prepare-"));
  const output = join(parent, "new-demo");
  try {
    const result = spawnSync(process.execPath, [script, "--output", output], {
      encoding: "utf8", env: { ...process.env, TACHIKO_BIN: "/definitely/not/tachiko" },
    });
    assert.notEqual(result.status, 0);
    assert.equal(readdirSync(parent).includes("new-demo"), false);
    assert.deepEqual(readdirSync(parent).filter(name => name.startsWith(".issue341-stage-")), []);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});
