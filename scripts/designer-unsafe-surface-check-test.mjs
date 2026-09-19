#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const scanner = resolve(import.meta.dirname, "designer-unsafe-surface-check.mjs");
const packageDirectories = new Set(["src", "tests", "examples", "fixtures", "benches", "target"]);

function runFixture(files) {
  const root = mkdtempSync(join(tmpdir(), "tachiko-designer-unsafe-"));
  try {
    for (const [relativePath, source] of Object.entries(files)) {
      const [directory] = relativePath.split("/");
      const packagePath = relativePath === "build.rs" || packageDirectories.has(directory)
        ? relativePath
        : join("src", relativePath);
      const path = join(root, packagePath);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, source);
    }
    return spawnSync(process.execPath, [scanner, root], { encoding: "utf8" });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function assertRejected(name, files) {
  const result = runFixture(files);
  assert.notEqual(result.status, 0, `${name} unexpectedly passed`);
  assert.match(result.stderr, /unsafe|unterminated|cannot/i, `${name} did not fail through the scanner`);
}

function assertAccepted(name, files) {
  const result = runFixture(files);
  assert.equal(result.status, 0, `${name} failed: ${result.stderr}`);
}

for (const [name, source] of [
  ["unsafe block", "fn f() { unsafe { call(); } }"],
  ["multiline unsafe block", "fn f() { unsafe\n{ call(); } }"],
  ["multiline unsafe function", "unsafe\nfn f() {}"],
  ["unsafe extern function", "unsafe extern \"C\" fn f() {}"],
  ["multiline unsafe extern function", "unsafe\nextern \"C\"\nfn f() {}"],
  ["unsafe extern block", "unsafe extern \"C\" { fn f(); }"],
  ["unsafe impl", "unsafe impl Trait for Type {}"],
  ["unsafe trait", "unsafe trait Trait {}"],
  ["comments between unsafe tokens", "unsafe /* boundary */ fn f() {}"],
  ["other unsafe attribute", "#[unsafe(export_name = \"f\")]\nfn f() {}"],
]) {
  assertRejected(name, { "lib.rs": source });
}

assertRejected("unsafe in tests", { "tests/unsafe.rs": "fn f() { unsafe { call(); } }" });
assertRejected("unsafe in examples", { "examples/unsafe.rs": "unsafe fn f() {}" });
assertRejected("unsafe in build.rs", { "build.rs": "unsafe fn f() {}" });
assertRejected("unsafe in fixtures", { "fixtures/unsafe.rs": "unsafe impl Trait for Type {}" });
assertRejected("no_mangle outside root wasm.rs", { "lib.rs": "#[unsafe(no_mangle)]\npub extern \"C\" fn tachiko_designer_request_run() {}" });
assertRejected("no_mangle in nested wasm.rs", { "src/nested/wasm.rs": "#[unsafe(no_mangle)]\npub extern \"C\" fn tachiko_designer_request_run() {}" });
assertRejected("normal Rust ABI function", { "wasm.rs": "#[unsafe(no_mangle)]\npub fn tachiko_designer_request_run() {}" });
assertRejected("static instead of function", { "wasm.rs": "#[unsafe(no_mangle)]\npub static tachiko_designer_request_run: u8 = 0;" });
assertRejected("non-C ABI function", { "wasm.rs": "#[unsafe(no_mangle)]\npub extern \"Rust\" fn tachiko_designer_request_run() {}" });
assertRejected("omitted ABI function", { "wasm.rs": "#[unsafe(no_mangle)]\npub extern fn tachiko_designer_request_run() {}" });
assertRejected("unapproved export name", { "wasm.rs": "#[unsafe(no_mangle)]\npub extern \"C\" fn tachiko_designer_not_approved() {}" });
assertAccepted("approved root wasm ABI attribute", {
  "src/wasm.rs": "#[unsafe /* ABI */ ( no_mangle )]\npub extern \"C\" fn tachiko_designer_request_run() {}",
  "src/lib.rs": "fn safe() {}",
});
assertAccepted("comments and literals are not unsafe syntax", {
  "src/lib.rs": [
    "// unsafe extern fn ignored() {}",
    "/* unsafe { ignored(); } */",
    "const TEXT: &str = \"unsafe fn ignored\";",
    "const RAW: &str = r#\"unsafe { ignored(); }\"#;",
    "const BYTE: &[u8] = b\"unsafe impl\";",
    "const C_TEXT: &str = c\"unsafe trait\";",
    "const LETTER: char = 'u';",
    "fn r#unsafe() {}",
    "fn with_lifetime<'unsafe>(value: &'unsafe str) -> &'unsafe str { value }",
  ].join("\n"),
});
assertAccepted("target is ignored", {
  "target/generated.rs": "unsafe fn ignored() {}",
  "src/lib.rs": "fn safe() {}",
});

const missing = spawnSync(process.execPath, [scanner, join(tmpdir(), "tachiko-missing-unsafe-root")], { encoding: "utf8" });
assert.notEqual(missing.status, 0, "missing source root unexpectedly passed");

console.log("designer unsafe-surface scanner fixtures: PASS");
