import { readFile } from "node:fs/promises";

const [messagesPath, targetName] = process.argv.slice(2);
if (!messagesPath || !targetName) {
  throw new Error("usage: node cargo-artifact-path.mjs <messages.jsonl> <target-name>");
}

const messages = (await readFile(messagesPath, "utf8"))
  .split("\n")
  .filter(Boolean)
  .map((line) => JSON.parse(line));
const candidates = messages
  .filter(
    (message) =>
      message.reason === "compiler-artifact" && message.target?.name === targetName,
  )
  .flatMap((message) => message.filenames ?? [])
  .filter((filename) => filename.endsWith(".rlib"));

if (candidates.length === 0) {
  throw new Error(`Cargo emitted no rlib artifact for ${targetName}`);
}
console.log(candidates.at(-1));
