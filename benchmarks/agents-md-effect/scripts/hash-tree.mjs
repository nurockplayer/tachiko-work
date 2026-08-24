#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readdir, readlink, realpath } from "node:fs/promises";
import { relative, resolve } from "node:path";

if (process.argv.length < 3 || process.argv.length > 4) {
  console.error("usage: node hash-tree.mjs /absolute/tree [--content-only]");
  process.exit(2);
}

const contentOnly = process.argv[3] === "--content-only";
if (process.argv[3] !== undefined && !contentOnly) {
  console.error("the only supported option is --content-only");
  process.exit(2);
}

const root = await realpath(resolve(process.argv[2]));
const entries = [];

async function fileSha256(filePath) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest("hex");
}

async function walk(directory) {
  const children = await readdir(directory);
  children.sort();
  for (const child of children) {
    const absolute = resolve(directory, child);
    const info = await lstat(absolute);
    const pathFromRoot = relative(root, absolute);
    const mode = info.mode & 0o777;
    if (info.isDirectory()) {
      entries.push({
        path: pathFromRoot,
        type: "directory",
        ...(contentOnly ? {} : {mode}),
      });
      await walk(absolute);
    } else if (info.isSymbolicLink()) {
      entries.push({
        path: pathFromRoot,
        type: "symlink",
        ...(contentOnly ? {} : {mode}),
        target: await readlink(absolute),
      });
    } else if (info.isFile()) {
      entries.push({
        path: pathFromRoot,
        type: "file",
        ...(contentOnly ? {} : {mode}),
        bytes: info.size,
        sha256: await fileSha256(absolute),
      });
    } else {
      throw new Error(`unsupported tree entry: ${absolute}`);
    }
  }
}

await walk(root);
const manifestBytes = Buffer.from(`${JSON.stringify(entries)}\n`, "utf8");
const totalFileBytes = entries.reduce(
  (sum, entry) => sum + (entry.type === "file" ? entry.bytes : 0),
  0,
);
console.log(
  JSON.stringify({
    root,
    entries: entries.length,
    file_bytes: totalFileBytes,
    digest_kind: contentOnly ? "paths-types-content-v1" : "paths-types-modes-content-v1",
    manifest_sha256: createHash("sha256").update(manifestBytes).digest("hex"),
  }),
);
