import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function repositoryRoot(moduleUrl) {
  return resolve(fileURLToPath(new URL("../..", moduleUrl)));
}
