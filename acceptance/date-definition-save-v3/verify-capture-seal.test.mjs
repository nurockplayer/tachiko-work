// Synthetic seal-unit controls only. These files are not native/Worker output
// and cannot supply changed-contract producer or historical-reader evidence.
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import assert from "node:assert/strict";
import { NATIVE_CASES, WORKER_CASES, PROFILES, verifyCaptureSeal } from "./verify-capture-seal.mjs";

const head = "a".repeat(40);
const run = "00000000-0000-4000-8000-000000000001";
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
function syntheticSeal() {
  const root = mkdtempSync(join(tmpdir(), "work374-seal-unit-"));
  const captures = join(root, "capture"); const kit = join(root, "kit");
  mkdirSync(captures); mkdirSync(kit);
  const json = (file, value) => writeFileSync(file, `${JSON.stringify(value)}\n`);
  const artifacts = {};
  for (const profile of PROFILES) {
    const kind = profile.startsWith("ordinary-") ? "ordinary_legacy_export" :
      profile.startsWith("constrained-") ? "valid_storage_v3_constraint_input" :
        profile.startsWith("boundary-fresh-65537") ? "fresh_admission_probe" : "selected_v3_export";
    writeFileSync(join(captures, `${profile}.twd`), `synthetic:${profile}`);
    json(join(captures, `${profile}.source.json`), { producer_head: head, capture_run_id: run, profile, kind });
    for (const extension of ["twd", "source.json"]) {
      const name = `${profile}.${extension}`;
      artifacts[name] = sha(readFileSync(join(captures, name)));
    }
  }
  writeFileSync(join(kit, "client.js"), "synthetic kit");
  json(join(kit, "artifact-manifest.json"), { sourceCommit: head,
    files: [{ path: "client.js", sha256: sha(readFileSync(join(kit, "client.js"))) }] });
  const kitHash = sha(readFileSync(join(kit, "artifact-manifest.json")));
  json(join(captures, "capture-lease.json"), { state: "worker_complete", candidate_head: head, run_id: run, kit_artifact_manifest_sha256: kitHash });
  json(join(captures, "native-result.json"), { status: "native_cases_passed", candidate_head: head, run_id: run,
    required_case_count: NATIVE_CASES.length, required_cases: NATIVE_CASES, artifacts });
  writeFileSync(join(captures, "worker-acceptance.log"), `${WORKER_CASES.join("\n")}\n2 passed\n`);
  json(join(captures, "worker-completion.json"), { status: "native_and_worker_cases_passed", candidate_head: head, capture_run_id: run,
    required_worker_case_count: WORKER_CASES.length, required_worker_cases: WORKER_CASES, kit_artifact_manifest_sha256: kitHash,
    native_result_sha256: sha(readFileSync(join(captures, "native-result.json"))),
    worker_log_sha256: sha(readFileSync(join(captures, "worker-acceptance.log"))) });
  return { root, captures, kit, json, verify: () => verifyCaptureSeal(captures, head, run, kit) };
}

test("seal verifies every artifact and returns the exact verified snapshot", () => {
  const fixture = syntheticSeal();
  try {
    const verified = fixture.verify();
    assert.equal(Object.keys(verified.artifacts).length, PROFILES.length * 2);
    assert.equal(Buffer.from(verified.artifacts["date-only.twd"], "hex").toString(), "synthetic:date-only");
    writeFileSync(join(fixture.captures, "date-only.twd"), "changed after snapshot");
    assert.equal(Buffer.from(verified.artifacts["date-only.twd"], "hex").toString(), "synthetic:date-only");
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});

test("historical stdin preflight returns sealed bytes and rejects subsequent changes", () => {
  const fixture = syntheticSeal();
  try {
    const input = `${readFileSync(new URL("./verify-capture-seal.mjs", import.meta.url), "utf8")}\nprocess.stdout.write(JSON.stringify(verifyCaptureSeal(...process.argv.slice(2))));\n`;
    const invoke = () => spawnSync(process.execPath, ["--input-type=module", "-", fixture.captures, head, run, fixture.kit], { input, encoding: "utf8" });
    const positive = invoke();
    assert.equal(positive.status, 0, positive.stderr);
    assert.equal(Buffer.from(JSON.parse(positive.stdout).artifacts["date-only.twd"], "hex").toString(), "synthetic:date-only");
    writeFileSync(join(fixture.captures, "date-only.twd"), "changed after Worker seal");
    const negative = invoke();
    assert.notEqual(negative.status, 0);
    assert.match(negative.stderr, /ARTIFACT_HASH/);
    assert.equal(negative.stdout, "");
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});

for (const [label, mutate, expected] of [
  ["changed artifact byte", (f) => writeFileSync(join(f.captures, "date-only.twd"), "changed"), /ARTIFACT_HASH/],
  ["changed receipt byte", (f) => writeFileSync(join(f.captures, "neither.source.json"), "{}"), /ARTIFACT_HASH/],
  ["broken completion to native-result link", (f) => {
    const file = join(f.captures, "native-result.json");
    writeFileSync(file, `${readFileSync(file)} `);
  }, /NATIVE_RESULT_LINK/],
  ["changed Worker log", (f) => writeFileSync(join(f.captures, "worker-acceptance.log"), "changed"), /WORKER_LOG_LINK/],
  ["changed kit member", (f) => writeFileSync(join(f.kit, "client.js"), "changed"), /KIT_HASH/],
  ["missing artifact", (f) => rmSync(join(f.captures, "ordinary-neither.twd")), /CAPTURE_INVENTORY/],
  ["extra artifact", (f) => writeFileSync(join(f.captures, "stale.twd"), "stale"), /CAPTURE_INVENTORY/],
  ["mismatched completion kit identity", (f) => {
    const file = join(f.captures, "worker-completion.json"); const value = JSON.parse(readFileSync(file));
    value.kit_artifact_manifest_sha256 = "b".repeat(64); f.json(file, value);
  }, /KIT_IDENTITY/],
]) test(`seal refuses ${label}`, () => {
  const fixture = syntheticSeal();
  try { mutate(fixture); assert.throws(fixture.verify, expected); }
  finally { rmSync(fixture.root, { recursive: true, force: true }); }
});
