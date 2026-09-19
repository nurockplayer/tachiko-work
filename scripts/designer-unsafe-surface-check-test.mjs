#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const scanner = resolve(import.meta.dirname, "designer-unsafe-surface-check.mjs");
const packageDirectories = new Set(["src", "tests", "examples", "fixtures", "benches", "target", "custom"]);

function runFixture(files) {
  const root = mkdtempSync(join(tmpdir(), "tachiko-designer-unsafe-"));
  try {
    for (const [relativePath, source] of Object.entries(files)) {
      const [directory] = relativePath.split("/");
      const packagePath = relativePath === "build.rs" || relativePath === "Cargo.toml" || packageDirectories.has(directory)
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
assertRejected("unsafe assembly macro", { "lib.rs": "global_asm!(\"\");" });

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
assertRejected("unsafe in an included non-Rust source", {
  "src/lib.rs": "include!(\"payload.inc\");",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("nonliteral include path", {
  "src/lib.rs": "include!(concat!(\"payload\", \".inc\"));",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a path module", {
  "src/lib.rs": "#[path = \"payload.inc\"] mod payload;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a cfg_attr path module", {
  "src/lib.rs": "#[cfg_attr(all(), path = \"payload.inc\")] mod payload;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in an inline-module path module", {
  "src/lib.rs": "mod outer { #[path = \"payload.inc\"] mod payload; }",
  "src/outer/payload.inc": "pub unsafe fn bypassed() {}",
  "src/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in an inline module from a nested module file", {
  "src/lib.rs": "mod foo;",
  "src/foo.rs": "mod bar { #[path = \"payload.inc\"] mod payload; }",
  "src/foo/bar/payload.inc": "pub unsafe fn bypassed() {}",
  "src/foo/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in a path-renamed module's nested path", {
  "src/lib.rs": "#[path = \"renamed.rs\"] mod foo;",
  "src/renamed.rs": "#[path = \"payload.inc\"] mod payload;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
  "src/foo/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in a direct path inside a child module file", {
  "src/lib.rs": "mod foo;",
  "src/foo.rs": "#[path = \"payload.inc\"] mod payload;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
  "src/foo/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in a raw-identifier inline module path", {
  "src/lib.rs": "mod r#outer { #[path = \"payload.inc\"] mod payload; }",
  "src/outer/payload.inc": "pub unsafe fn bypassed() {}",
  "src/r#outer/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in an inline module directory path", {
  "src/lib.rs": "#[path = \"thread_files\"] mod thread { mod payload; }",
  "src/thread_files/payload.rs": "pub unsafe fn bypassed() {}",
  "src/payload.rs": "pub fn safe_decoy() {}",
});
assertRejected("unsafe in a custom Cargo target", {
  "Cargo.toml": "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"custom\"\npath = \"custom/tool.rs\"\n",
  "custom/tool.rs": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a custom target child module", {
  "Cargo.toml": "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"custom\"\npath = \"custom/tool.rs\"\n",
  "custom/tool.rs": "mod helper;",
  "custom/helper.rs": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in an inline module directory with a child module", {
  "Cargo.toml": "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"custom\"\npath = \"custom/tool.rs\"\n",
  "custom/tool.rs": "#[path = \"../generated/thread_files\"] mod thread { mod payload; }",
  "generated/thread_files/payload.rs": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a build-script path module", {
  "build.rs": "#[path = \"helper.inc\"] mod helper;",
  "helper.inc": "pub unsafe fn bypassed() {}",
  "build/helper.inc": "pub fn safe_decoy() {}",
});
assertRejected("inactive cfg_attr path keeps the conventional module", {
  "src/lib.rs": "#[cfg_attr(any(), path = \"unused.inc\")] mod foo;",
  "src/unused.inc": "pub fn safe() {}",
  "src/foo.rs": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe through an include alias", {
  "src/lib.rs": "use std::include as source; source!(\"payload.inc\");",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe through a grouped include alias", {
  "src/lib.rs": "use std::{include as source, println}; source!(\"payload.inc\");",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe through an assembly macro alias", {
  "src/lib.rs": "use core::arch::global_asm as generated; generated!(\"\");",
});
assertRejected("unsafe through a raw assembly macro alias", {
  "src/lib.rs": "use core::arch::global_asm as r#generated; r#generated!(\"\");",
});
assertRejected("unsafe through a chained assembly macro alias", {
  "src/lib.rs": "use core::arch::global_asm as generated; use self::generated as forwarded; forwarded!(\"\");",
});
assertRejected("unsafe through an included assembly macro alias", {
  "src/lib.rs": "include!(\"aliases.inc\"); generated!(\"\");",
  "src/aliases.inc": "use core::arch::global_asm as generated;",
});
assertRejected("unsafe through an ordinary module assembly macro alias", {
  "src/lib.rs": "mod aliases; mod caller;",
  "src/aliases.rs": "pub use core::arch::global_asm as hidden_asm;",
  "src/caller.rs": "use crate::aliases::hidden_asm; hidden_asm!(\"\");",
});
assertRejected("unsafe through an ordinary module include alias", {
  "src/lib.rs": "mod aliases; mod caller;",
  "src/aliases.rs": "pub use std::include as source;",
  "src/caller.rs": "use crate::aliases::source; source!(\"payload.inc\");",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe through an alias introduced by a later include", {
  "src/lib.rs": "source!(\"payload.inc\"); include!(\"aliases.inc\");",
  "src/aliases.inc": "use std::include as source;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a path module inside an included source", {
  "src/lib.rs": "include!(\"nested/fragment.inc\");",
  "src/nested/fragment.inc": "#[path = \"payload.inc\"] mod payload;",
  "src/nested/payload.inc": "pub unsafe fn bypassed() {}",
  "src/payload.inc": "pub fn safe_decoy() {}",
});
assertRejected("unsafe through a raw assembly macro identifier", {
  "src/lib.rs": "core::arch::r#global_asm!(\"\");",
});
assertRejected("unsafe in a long cfg_attr path module", {
  "src/lib.rs": "#[cfg_attr(all(unix, target_pointer_width = \"64\", not(target_os = \"none\")), path = \"payload.inc\")] mod payload;",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe through a forwarded include macro", {
  "src/lib.rs": "macro_rules! invoke { ($m:ident, $p:literal) => { $m!($p); } }\ninvoke!(include, \"payload.inc\");",
  "src/payload.inc": "pub unsafe fn bypassed() {}",
});
assertRejected("unsafe in a nested target module", {
  "src/lib.rs": "mod target;",
  "src/target/mod.rs": "pub unsafe fn bypassed() {}",
});
assertAccepted("safe included non-Rust source", {
  "src/lib.rs": "include!(\"payload.inc\");",
  "src/payload.inc": "pub fn safe() {}",
});

function runSymlinkFixture() {
  const root = mkdtempSync(join(tmpdir(), "tachiko-designer-unsafe-symlink-"));
  try {
    mkdirSync(join(root, "src"), { recursive: true });
    writeFileSync(join(root, "unsafe.rs"), "pub unsafe fn bypassed() {}\n");
    symlinkSync("../unsafe.rs", join(root, "src", "lib.rs"));
    return spawnSync(process.execPath, [scanner, root], { encoding: "utf8" });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const symlink = runSymlinkFixture();
assert.notEqual(symlink.status, 0, "symlinked source unexpectedly passed");
assert.match(symlink.stderr, /symbolic link/i, "symlinked source did not fail closed");
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
