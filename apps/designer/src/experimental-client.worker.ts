/// <reference lib="webworker" />

import { startDesignerWorker } from "./runtime/worker-runtime.ts";

startDesignerWorker(new URL("./designer_runtime.wasm", import.meta.url).href);
