import { mkdir, rm, writeFile } from "node:fs/promises";
import { dashboardHtml } from "../dist/server/shared.js";

// Only the shared UI is an asset. Server modules, bindings and source maps are
// never copied into the browser-served directory.
const directory = new URL("../dist/assets/", import.meta.url);
await rm(directory, { recursive: true, force: true });
await mkdir(directory, { recursive: true });
await writeFile(new URL("index.html", directory), dashboardHtml);
