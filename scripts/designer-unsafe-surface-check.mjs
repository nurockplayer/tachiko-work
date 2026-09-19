#!/usr/bin/env node

import { existsSync, lstatSync, readdirSync, readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, relative, resolve, sep } from "node:path";

const APPROVED_EXPORTS = new Set([
  "tachiko_designer_request_reserve",
  "tachiko_designer_request_run",
  "tachiko_designer_response_ptr",
  "tachiko_designer_response_len",
  "tachiko_designer_project_reserve",
  "tachiko_designer_project_open",
  "tachiko_designer_project_open_local_document",
  "tachiko_designer_portable_ro_open",
  "tachiko_designer_portable_ro_verify",
  "tachiko_designer_project_inspect",
  "tachiko_designer_spreadsheet_run",
  "tachiko_designer_project_export",
  "tachiko_designer_canonical_tree_export",
  "tachiko_designer_portable_ro_export",
  "tachiko_designer_occurrence_observe",
  "tachiko_designer_project_release",
  "tachiko_designer_project_close",
  "tachiko_designer_project_ptr",
  "tachiko_designer_project_len",
]);
const UNSAFE_MACROS = new Set(["asm", "global_asm", "llvm_asm", "naked_asm"]);

function fail(message) {
  throw new Error(`designer-unsafe-surface-check: ${message}`);
}

function codePointAt(source, index) {
  const point = source.codePointAt(index);
  const value = String.fromCodePoint(point);
  return { value, width: value.length };
}

function isIdentifierStart(source, index) {
  if (index >= source.length) return false;
  return /[\p{XID_Start}_]/u.test(codePointAt(source, index).value);
}

function isIdentifierContinue(source, index) {
  if (index >= source.length) return false;
  return /[\p{XID_Continue}_]/u.test(codePointAt(source, index).value);
}

function sourceFiles(root) {
  let rootStats;
  try {
    rootStats = statSync(root);
  } catch (error) {
    fail(`cannot read source root ${root}: ${error.message}`);
  }
  if (!rootStats.isDirectory()) fail(`source root is not a directory: ${root}`);

  const files = [];
  function visit(directory) {
    let entries;
    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch (error) {
      fail(`cannot list ${directory}: ${error.message}`);
    }
    for (const entry of entries) {
      const path = `${directory}${sep}${entry.name}`;
      if (entry.isSymbolicLink()) {
        fail(`symbolic links are not supported in the runtime source surface: ${path}`);
      }
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(path);
      }
    }
  }
  for (const directory of ["src", "tests", "examples", "fixtures", "benches"]) {
    const path = `${root}${sep}${directory}`;
    if (existsSync(path)) visit(path);
  }
  const manifest = `${root}${sep}Cargo.toml`;
  if (existsSync(manifest)) {
    let metadata;
    try {
      metadata = JSON.parse(execFileSync("cargo", ["metadata", "--manifest-path", manifest, "--no-deps", "--format-version", "1"], { encoding: "utf8" }));
    } catch (error) {
      fail(`cannot read Cargo target metadata: ${error.message}`);
    }
    for (const target of metadata.packages?.flatMap((packageInfo) => packageInfo.targets) ?? []) {
      const targetPath = resolve(target.src_path);
      const relativeTarget = relative(resolve(root), targetPath);
      if (relativeTarget === ".." || relativeTarget.startsWith(`..${sep}`)) {
        fail(`Cargo target escapes the runtime package: ${target.src_path}`);
      }
      files.push(targetPath);
    }
  }
  const buildScript = `${root}${sep}build.rs`;
  if (existsSync(buildScript)) {
    if (lstatSync(buildScript).isSymbolicLink()) {
      fail(`symbolic links are not supported in the runtime source surface: ${buildScript}`);
    }
    files.push(buildScript);
  }
  return files.sort();
}

function tokensFor(source, path) {
  const tokens = [];
  let index = 0;
  let line = 1;
  let column = 1;

  function advanceTo(next) {
    while (index < next) {
      if (source[index] === "\n") {
        line += 1;
        column = 1;
      } else {
        column += 1;
      }
      index += 1;
    }
  }

  function unterminated(kind, startLine, startColumn) {
    fail(`${path}:${startLine}:${startColumn}: unterminated ${kind}`);
  }

  while (index < source.length) {
    const start = index;
    const startLine = line;
    const startColumn = column;
    const current = source[index];

    if (/\s/.test(current)) {
      advanceTo(index + 1);
      continue;
    }
    if (source.startsWith("//", index)) {
      const end = source.indexOf("\n", index + 2);
      advanceTo(end === -1 ? source.length : end);
      continue;
    }
    if (source.startsWith("/*", index)) {
      let depth = 1;
      let cursor = index + 2;
      while (cursor < source.length && depth > 0) {
        if (source.startsWith("/*", cursor)) {
          depth += 1;
          cursor += 2;
        } else if (source.startsWith("*/", cursor)) {
          depth -= 1;
          cursor += 2;
        } else {
          cursor += 1;
        }
      }
      if (depth !== 0) unterminated("block comment", startLine, startColumn);
      advanceTo(cursor);
      continue;
    }

    let rawPrefixLength = 0;
    if (current === "r") rawPrefixLength = 1;
    if ((current === "b" || current === "c") && source[index + 1] === "r") rawPrefixLength = 2;
    if (rawPrefixLength > 0) {
      let cursor = index + rawPrefixLength;
      while (source[cursor] === "#") cursor += 1;
      if (source[cursor] === '"') {
        const hashes = source.slice(index + rawPrefixLength, cursor);
        const terminator = `"${hashes}`;
        const end = source.indexOf(terminator, cursor + 1);
        if (end === -1) unterminated("raw string", startLine, startColumn);
        tokens.push({ kind: "string", value: source.slice(index, end + terminator.length), line: startLine, column: startColumn });
        advanceTo(end + terminator.length);
        continue;
      }
    }

    if (current === '"') {
      let cursor = index + 1;
      let closed = false;
      while (cursor < source.length) {
        if (source[cursor] === "\\") {
          cursor += 2;
        } else if (source[cursor] === '"') {
          cursor += 1;
          closed = true;
          break;
        } else {
          cursor += 1;
        }
      }
      if (!closed) unterminated("string", startLine, startColumn);
      tokens.push({ kind: "string", value: source.slice(index, cursor), line: startLine, column: startColumn });
      advanceTo(cursor);
      continue;
    }
    if (current === "'") {
      let cursor = index + 1;
      let closed = false;
      if (source[cursor] === "\\") {
        cursor += 1;
        if (source[cursor] === "u" && source[cursor + 1] === "{") {
          const escapeEnd = source.indexOf("}", cursor + 2);
          if (escapeEnd !== -1) cursor = escapeEnd + 1;
        } else if (cursor < source.length) {
          cursor += codePointAt(source, cursor).width;
        }
      } else if (cursor < source.length && source[cursor] !== "\n") {
        cursor += codePointAt(source, cursor).width;
      }
      if (source[cursor] === "'") {
        cursor += 1;
        closed = true;
      }
      if (closed) {
        advanceTo(cursor);
        continue;
      }
      if (isIdentifierStart(source, index + 1)) {
        let lifetimeEnd = index + 1;
        while (isIdentifierContinue(source, lifetimeEnd)) {
          lifetimeEnd += codePointAt(source, lifetimeEnd).width;
        }
        tokens.push({ kind: "identifier", value: source.slice(index, lifetimeEnd), line: startLine, column: startColumn });
        advanceTo(lifetimeEnd);
        continue;
      }
    }

    if (source.startsWith("r#", index) && isIdentifierStart(source, index + 2)) {
      let cursor = index + 2;
      while (isIdentifierContinue(source, cursor)) cursor += codePointAt(source, cursor).width;
      tokens.push({ kind: "identifier", value: source.slice(index, cursor), line: startLine, column: startColumn });
      advanceTo(cursor);
      continue;
    }
    if (isIdentifierStart(source, index)) {
      let cursor = index;
      while (isIdentifierContinue(source, cursor)) cursor += codePointAt(source, cursor).width;
      tokens.push({ kind: "identifier", value: source.slice(index, cursor), line: startLine, column: startColumn });
      advanceTo(cursor);
      continue;
    }

    tokens.push({ kind: "punctuation", value: current, line: startLine, column: startColumn });
    advanceTo(index + 1);
  }
  return tokens;
}

function approvedNoMangle(tokens, index, path, packageRoot) {
  return relative(resolve(packageRoot), path).split(sep).join("/") === "src/wasm.rs" &&
    tokens[index - 2]?.value === "#" &&
    tokens[index - 1]?.value === "[" &&
    tokens[index + 1]?.value === "(" &&
    tokens[index + 2]?.value === "no_mangle" &&
    tokens[index + 3]?.value === ")" &&
    tokens[index + 4]?.value === "]" &&
    tokens[index + 5]?.value === "pub" &&
    tokens[index + 6]?.value === "extern" &&
    tokens[index + 7]?.kind === "string" &&
    tokens[index + 7]?.value === '"C"' &&
    tokens[index + 8]?.value === "fn" &&
    tokens[index + 9]?.kind === "identifier" &&
    APPROVED_EXPORTS.has(tokens[index + 9]?.value);
}

function scan(root) {
  const resolvedRoot = resolve(root);
  const visited = new Set();
  function scanFile(path) {
    if (visited.has(path)) return;
    visited.add(path);
    let source;
    try {
      source = readFileSync(path, "utf8");
    } catch (error) {
      fail(`cannot read ${path}: ${error.message}`);
    }
    const tokens = tokensFor(source, path);
    const includeNames = new Set(["include"]);
    for (let index = 0; index + 6 < tokens.length; index += 1) {
      if (tokens[index].value === "use" && tokens[index + 1]?.value === "std" &&
        tokens[index + 2]?.value === ":" && tokens[index + 3]?.value === ":" &&
        tokens[index + 4]?.value === "include" && tokens[index + 5]?.value === "as" &&
        tokens[index + 6]?.kind === "identifier") {
        includeNames.add(tokens[index + 6].value);
      }
    }
    if (tokens.some((token) => token.value === "macro_rules")) {
      fail(`${path}: macro_rules! source expansion is not supported by the unsafe-surface scanner`);
    }
    const inlineModules = [];
    const braceModules = [];
    function scanReferencedSource(reference, token, baseDirectory = dirname(path)) {
      if (!reference.startsWith('"') || !reference.endsWith('"')) {
        fail(`${path}:${token.line}:${token.column}: source path must be a normal string literal`);
      }
      let referencedPath;
      try {
        referencedPath = resolve(baseDirectory, JSON.parse(reference));
      } catch (error) {
        fail(`${path}:${token.line}:${token.column}: invalid source path: ${error.message}`);
      }
      const relativeReference = relative(resolvedRoot, referencedPath);
      if (relativeReference === "" || relativeReference === ".." || relativeReference.startsWith(`..${sep}`)) {
        fail(`${path}:${token.line}:${token.column}: source path escapes the runtime package: ${reference}`);
      }
      scanFile(referencedPath);
    }
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (token.value === "unsafe" && !approvedNoMangle(tokens, index, path, resolvedRoot)) {
        fail(`${path}:${token.line}:${token.column}: unsafe is outside the approved #[unsafe(no_mangle)] boundary in src/wasm.rs`);
      }
      if (UNSAFE_MACROS.has(token.value) && tokens[index + 1]?.value === "!") {
        fail(`${path}:${token.line}:${token.column}: unsafe macro ${token.value}! is outside the approved boundary`);
      }
      if (includeNames.has(token.value) && tokens[index + 1]?.value === "!") {
        const includeStringIndex = tokens[index + 2]?.value === "(" ? index + 3 : index + 2;
        if (tokens[includeStringIndex]?.kind !== "string" || !tokens[includeStringIndex].value.startsWith('"')) {
          fail(`${path}:${token.line}:${token.column}: include! path must be a normal string literal`);
        }
        scanReferencedSource(tokens[includeStringIndex].value, token);
      }
      if (token.value === "path" && tokens[index - 2]?.value === "#" && tokens[index - 1]?.value === "[" &&
        tokens[index + 1]?.value === "=" && tokens[index + 2]?.kind === "string") {
        scanReferencedSource(tokens[index + 2].value, token, join(dirname(path), ...inlineModules));
      }
      if (token.value === "path" && tokens[index + 1]?.value === "=" && tokens[index + 2]?.kind === "string" &&
        tokens.slice(Math.max(0, index - 12), index).some((candidate) => candidate.value === "cfg_attr")) {
        scanReferencedSource(tokens[index + 2].value, token, join(dirname(path), ...inlineModules));
      }
      if (token.value === "{") {
        let moduleName;
        for (let previous = index - 1; previous >= 0 && previous >= index - 6; previous -= 1) {
          if (["{", "}", ";"].includes(tokens[previous].value)) break;
          if (tokens[previous].value === "mod" && tokens[previous + 1]?.kind === "identifier") {
            moduleName = tokens[previous + 1].value;
            break;
          }
        }
        braceModules.push(moduleName);
        if (moduleName) inlineModules.push(moduleName);
      } else if (token.value === "}") {
        const moduleName = braceModules.pop();
        if (moduleName) inlineModules.pop();
      }
    }
  }
  for (const path of sourceFiles(resolvedRoot)) scanFile(path);
}

if (process.argv.length !== 3) {
  console.error("usage: designer-unsafe-surface-check.mjs <runtime-package-root>");
  process.exit(2);
}

try {
  scan(process.argv[2]);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
