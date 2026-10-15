import { readFile, writeFile } from "node:fs/promises";

// Rust storage owns both membership and ordering. Keep the browser adapter's
// generated table in its existing module so the exported kit needs no new API.
const versions = [1, 2, 3];
const tables = await Promise.all(versions.map(async version => {
  const source = await readFile(new URL(`../../../crates/storage/src/roproj/v${version}.rs`, import.meta.url), "utf8");
  const declaration = source.match(new RegExp(`pub const ROPROJ_V${version}_PATHS: \\[&str; (\\d+)\\] = \\[([\\s\\S]*?)\\];`));
  if (!declaration) throw new Error(`Missing Rust v${version} path authority`);
  const paths = Array.from(declaration[2].matchAll(/"([^"]+)"/g), match => match[1]);
  if (paths.length !== Number(declaration[1]) || new Set(paths).size !== paths.length) {
    throw new Error(`Invalid Rust v${version} path declaration`);
  }
  return `  ${version}: ${JSON.stringify(paths)},`;
}));
const start = "// BEGIN generated Rust storage paths";
const end = "// END generated Rust storage paths";
const generated = `${start}\n// Run: node packages/browser-client/scripts/generate-project-paths.mjs\nconst CANONICAL_PROJECT_PATHS: Readonly<Record<number, readonly string[]>> = {\n${tables.join("\n")}\n};\n${end}`;
const target = new URL("../src/host/project-transfer.ts", import.meta.url);
const source = await readFile(target, "utf8");
const region = new RegExp(`${start}[\\s\\S]*?${end}`);
if (!region.test(source)) throw new Error("Missing generated host path region");
const expected = source.replace(region, generated);
if (process.argv.includes("--check")) {
  if (source !== expected) throw new Error("Host paths differ from Rust storage; regenerate project paths");
} else {
  await writeFile(target, expected);
}
