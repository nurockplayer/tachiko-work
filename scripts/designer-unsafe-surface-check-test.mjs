#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const scanner = resolve(import.meta.dirname, "designer-unsafe-surface-check.mjs");

function runFixture(files) {
  const root = mkdtempSync(join(tmpdir(), "tachiko-designer-unsafe-"));
  try {
    for (const [relativePath, source] of Object.entries(files)) {
      const path = join(root, relativePath);
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

assertRejected("no_mangle outside root wasm.rs", { "lib.rs": "#[unsafe(no_mangle)]\nfn f() {}" });
assertRejected("no_mangle in nested wasm.rs", { "nested/wasm.rs": "#[unsafe(no_mangle)]\nfn f() {}" });
assertAccepted("approved root wasm ABI attribute", {
  "wasm.rs": "#[unsafe /* ABI */ ( no_mangle )]\npub extern \"C\" fn f() {}",
  "lib.rs": "fn safe() {}",
});
assertAccepted("comments and literals are not unsafe syntax", {
  "lib.rs": [
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

const missing = spawnSync(process.execPath, [scanner, join(tmpdir(), "tachiko-missing-unsafe-root")], { encoding: "utf8" });
assert.notEqual(missing.status, 0, "missing source root unexpectedly passed");

console.log("designer unsafe-surface scanner fixtures: PASS");
