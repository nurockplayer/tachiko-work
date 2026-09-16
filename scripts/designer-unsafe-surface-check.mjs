#!/usr/bin/env node

import { readdirSync, readFileSync, statSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

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
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(path);
      }
    }
  }
  visit(root);
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
        tokens.push({ value: source.slice(index, lifetimeEnd), line: startLine, column: startColumn });
        advanceTo(lifetimeEnd);
        continue;
      }
    }

    if (source.startsWith("r#", index) && isIdentifierStart(source, index + 2)) {
      let cursor = index + 2;
      while (isIdentifierContinue(source, cursor)) cursor += codePointAt(source, cursor).width;
      tokens.push({ value: source.slice(index, cursor), line: startLine, column: startColumn });
      advanceTo(cursor);
      continue;
    }
    if (isIdentifierStart(source, index)) {
      let cursor = index;
      while (isIdentifierContinue(source, cursor)) cursor += codePointAt(source, cursor).width;
      tokens.push({ value: source.slice(index, cursor), line: startLine, column: startColumn });
      advanceTo(cursor);
      continue;
    }

    tokens.push({ value: current, line: startLine, column: startColumn });
    advanceTo(index + 1);
  }
  return tokens;
}

function approvedNoMangle(tokens, index, path) {
  return relative(resolve(process.argv[2]), path) === "wasm.rs" &&
    tokens[index - 2]?.value === "#" &&
    tokens[index - 1]?.value === "[" &&
    tokens[index + 1]?.value === "(" &&
    tokens[index + 2]?.value === "no_mangle" &&
    tokens[index + 3]?.value === ")" &&
    tokens[index + 4]?.value === "]";
}

function scan(root) {
  const resolvedRoot = resolve(root);
  for (const path of sourceFiles(resolvedRoot)) {
    let source;
    try {
      source = readFileSync(path, "utf8");
    } catch (error) {
      fail(`cannot read ${path}: ${error.message}`);
    }
    const tokens = tokensFor(source, path);
    for (let index = 0; index < tokens.length; index += 1) {
      const token = tokens[index];
      if (token.value === "unsafe" && !approvedNoMangle(tokens, index, path)) {
        fail(`${path}:${token.line}:${token.column}: unsafe is outside the approved #[unsafe(no_mangle)] boundary in root wasm.rs`);
      }
    }
  }
}

if (process.argv.length !== 3) {
  console.error("usage: designer-unsafe-surface-check.mjs <runtime-source-root>");
  process.exit(2);
}

try {
  scan(process.argv[2]);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
