// Independent ZIP32/root oracle over hand-authored canonical files. No Work codec.
// Run with `node acceptance/native-display-names/reference.mjs --check`.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import assert from "node:assert/strict";

const root = fileURLToPath(new URL(".", import.meta.url));
const paths = ["manifest.json", "schemas.json", "definitions.json", ...Array.from("0123456789abcdef", n => `entities/${n}.jsonl`)];
const hash = bytes => createHash("sha256").update(bytes).digest();
const files = paths.map(path => [path, readFileSync(join(root, "tree", path))]);
const payloadRoot = hash(Buffer.concat([Buffer.from("tachiko.portable-package/v1\0tachiko.roproj/v3\0"), ...files.map(([path, body]) => hash(Buffer.concat([Buffer.from(path), Buffer.from([0]), body])))]));
const manifest = Buffer.from(`{\n  "format": "tachiko.portable-package",\n  "format_version": 1,\n  "payload_format": "tachiko.roproj",\n  "payload_format_version": 3,\n  "payload_root_sha256": "${payloadRoot.toString("hex")}"\n}\n`);
const entries = [["package.json", manifest], ...files.map(([path, body]) => [`payload/${path}`, body])];
const crc32 = body => {
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
};
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n); return b; };
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const local = [], central = [], crcs = {};
let offset = 0;
for (const [path, body] of entries) {
  const name = Buffer.from(path), crc = crc32(body);
  crcs[path] = crc.toString(16).padStart(8, "0");
  const common = [u16(10), u16(0x0800), u16(0), u16(0), u16(0x21), u32(crc), u32(body.length), u32(body.length), u16(name.length), u16(0)];
  const record = Buffer.concat([u32(0x04034b50), ...common, name, body]);
  local.push(record);
  central.push(Buffer.concat([u32(0x02014b50), u16(20), ...common, u16(0), u16(0), u16(0), u32(0), u32(offset), name]));
  offset += record.length;
}
const directory = Buffer.concat(central);
const archive = Buffer.concat([...local, directory, u32(0x06054b50), u16(0), u16(0), u16(entries.length), u16(entries.length), u32(directory.length), u32(offset), u16(0)]);
const receipt = Buffer.from(JSON.stringify({ length: archive.length, sha256: hash(archive).toString("hex"), payload_root_sha256: payloadRoot.toString("hex"), crcs }, null, 2) + "\n");
if (process.argv[2] === "--check") {
  assert.deepEqual(readFileSync(join(root, "named.ro")), archive);
  assert.deepEqual(readFileSync(join(root, "expected.json")), receipt);
} else if (process.argv[2] === "--write") {
  writeFileSync(join(root, "named.ro"), archive);
  writeFileSync(join(root, "expected.json"), receipt);
} else throw new Error("Use --check or explicit --write");
