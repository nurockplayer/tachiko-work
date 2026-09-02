/// <reference lib="webworker" />

import { startDesignerWorker } from "./worker-runtime.ts";

startDesignerWorker("/designer_runtime.wasm");
