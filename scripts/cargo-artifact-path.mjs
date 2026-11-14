import { readFile } from "node:fs/promises";
import { dirname } from "node:path";

const [messagesPath, targetName] = process.argv.slice(2);
if (!messagesPath || !targetName) {
  throw new Error("usage: node cargo-artifact-path.mjs <messages.jsonl> <target-name|--dependency-dirs>");
}

const messages = (await readFile(messagesPath, "utf8"))
  .split("\n")
  .filter(Boolean)
  .map((line) => JSON.parse(line));
const candidates = messages
  .filter(
    (message) =>
      message.reason === "compiler-artifact" &&
      (targetName === "--dependency-dirs" || message.target?.name === targetName),
  )
  .flatMap((message) => message.filenames ?? [])
  .filter(
    (filename) => targetName === "--dependency-dirs" || filename.endsWith(".rlib"),
  );

if (candidates.length === 0) {
  throw new Error(`Cargo emitted no rlib artifact for ${targetName}`);
}
console.log(
  targetName === "--dependency-dirs"
    ? [...new Set(candidates.map((filename) => dirname(filename)))].join("\n")
    : candidates.at(-1),
);
